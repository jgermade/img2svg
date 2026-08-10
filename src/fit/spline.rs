//! Ajuste por Béziers cúbicas: esquinas, tangentes y mínimos cuadrados.
//!
//! El método es el de Schneider: parametrizar por longitud de cuerda, resolver
//! la cúbica que menos se aparta con las **tangentes de los extremos fijas**, y
//! si aun así se aparta demasiado, partir por el punto peor y repetir. Las
//! tangentes fijas no son un detalle de implementación: son lo que hace que dos
//! tramos consecutivos se encuentren sin pico.
//!
//! # Dónde puede aparecer un pico, y dónde no
//!
//! Una cadena abierta va de nodo a nodo, y un nodo es una esquina de la retícula
//! donde se juntan **tres o cuatro regiones** ([`crate::boundary`]). Eso es una
//! esquina de verdad casi siempre, así que sus extremos se clavan y no se
//! comparte tangente con la cadena vecina. El caso que quedaría —una tercera
//! región que toca en un solo punto una frontera por lo demás lisa— existe, es
//! raro, y lo que cuesta es un pico pequeño: las dos caras de cada cadena
//! siguen coincidiendo, así que no asoma fondo por ningún lado.
//!
//! Una cadena **cerrada** es harina de otro costal, y es la que importa. Un
//! bucle que no pasa por ningún nodo —una región suelta sobre un fondo
//! uniforme— se parte por donde caiga, y eso es un punto cualquiera del
//! contorno más liso que suele haber en la imagen. Por eso aquí se trata como
//! periódica: las esquinas se buscan dando la vuelta entera, y si la costura no
//! es una de ellas, su tangente se estima **con los puntos de los dos lados**,
//! de modo que la curva se va por donde llegó.

use super::{rdp_keep, Pt, Vertex};
use crate::trace::Point;

/// Ventana, en puntos del contorno denso, con la que se mide un giro.
///
/// Los puntos van de uno en uno por la retícula, así que son píxeles. Cuatro es
/// suficiente para que una escalera a 45° se lea como lo que es —recta— y corto
/// como para no cruzarse dos esquinas seguidas de una figura pequeña.
const WINDOW: usize = 4;

/// Coseno del giro a partir del cual un vértice es esquina; 60°.
///
/// El número que hay que separar es 90° —la esquina de un rectángulo, que tiene
/// que salir en pico— de 0°, que es lo que mide una diagonal en escalera con
/// esta ventana. Entre los dos hay sitio de sobra, y 60° deja pasar como lisas
/// las curvas cerradas de verdad sin redondear ninguna esquina recta.
const CORNER_COS: f64 = 0.5;

/// Cuántas veces se reajustan los parámetros antes de partir el tramo.
///
/// Reparametrizar es barato y evita un corte; partir siempre sale más caro en
/// bytes. Dos pasadas es donde deja de mejorar.
const REPARAM: usize = 2;

/// Ajusta una cadena abierta. Los dos extremos son nodos: se clavan.
pub fn open(points: &[Point], tolerance: f64) -> Vec<Vertex> {
    let pts = real(points);
    if pts.len() < 3 {
        return pts.iter().map(|&p| plain(p)).collect();
    }
    let mut cuts = vec![0];
    cuts.extend(inner_corners(&pts, false));
    cuts.push(pts.len() - 1);
    assemble(&pts, &cuts, None, tolerance)
}

/// Ajusta una cadena cerrada, ya sin el punto repetido del final.
pub fn closed(points: &[Point], tolerance: f64) -> Vec<Vertex> {
    let pts = real(points);
    if pts.len() < 4 {
        return pts.iter().map(|&p| plain(p)).collect();
    }
    let corners = inner_corners(&pts, true);

    // Sin esquinas, el contorno es liso entero y la costura cae donde cayó al
    // trazarlo. Se deja ahí y se le estima la tangente dando la vuelta, que es
    // lo que hace que no se note; rotar a un punto «mejor» no arreglaría nada,
    // porque no hay ninguno peor que otro.
    let start = corners.first().copied().unwrap_or(0);
    let n = pts.len();

    // La tangente de la costura se mide **antes** de rotar, sobre el contorno
    // con su período de verdad. Sobre el rotado, que repite el primer punto al
    // final para poder recorrerlo de una pasada, el módulo daría un paso de
    // menos: n+1 posiciones para un ciclo de n.
    let seam = corners.is_empty().then(|| across(&pts, start));

    let rotated: Vec<Pt> = (0..=n).map(|i| pts[(start + i) % n]).collect();
    let mut cuts = vec![0];
    cuts.extend(
        corners
            .iter()
            .map(|&c| (c + n - start) % n)
            .filter(|&c| c > 0),
    );
    cuts.sort_unstable();
    cuts.dedup();
    cuts.push(n);

    let mut out = assemble(&rotated, &cuts, seam, tolerance);
    // El último vértice es otra vez el primero: lo que llega a la costura se
    // guarda en el primero, que es el que se queda.
    if out.len() > 1 {
        let last = out.pop().expect("acabamos de mirar que hay más de uno");
        out[0].cin = last.cin;
    }
    out
}

/// Ajusta cada tramo entre cortes y encadena los vértices.
///
/// `seam` es la tangente compartida de los dos extremos, y sólo la trae un bucle
/// cuya costura no es esquina. Es toda la diferencia entre las dos formas: con
/// ella el contorno se cierra liso, y sin ella los extremos son esquinas y se
/// encuentran en pico, que es lo que quiere un nodo.
fn assemble(pts: &[Pt], cuts: &[usize], seam: Option<Pt>, tolerance: f64) -> Vec<Vertex> {
    let mut out = vec![plain(pts[0])];
    let last = pts.len() - 1;
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let t0 = match seam {
            Some(t) if a == 0 => t,
            _ => forward(pts, a, b),
        };
        let t1 = match seam {
            Some(t) if b == last => neg(t),
            _ => backward(pts, b, a),
        };
        let piece = &pts[a..=b];
        for seg in fit(piece, t0, t1, tolerance) {
            if let Some((c1, _)) = seg.controls {
                out.last_mut().expect("siempre hay uno").cout = Some(c1);
            }
            out.push(Vertex {
                p: piece[seg.to],
                cin: seg.controls.map(|(_, c2)| c2),
                cout: None,
            });
        }
    }
    out
}

/// Un tramo ajustado: hasta qué punto del contorno llega y con qué controles,
/// si es curvo.
///
/// Guarda el **índice** y no el punto porque el remate de rectas necesita saber
/// qué trozo del contorno original queda debajo de cada tramo.
#[derive(Clone, Copy)]
struct Seg {
    controls: Option<(Pt, Pt)>,
    to: usize,
}

/// Ajusta una polilínea con las dos tangentes dadas, partiéndola por el peor
/// punto mientras se aparte más de la cuenta.
///
/// Con pila explícita y no con recursión, por lo mismo que RDP: el reparto puede
/// salir tan desequilibrado como un corte por nivel, y en el contorno de una
/// región grande eso son miles de niveles. Se apila la mitad derecha antes que
/// la izquierda para que salgan en orden.
fn fit(pts: &[Pt], t0: Pt, t1: Pt, tolerance: f64) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut stack = vec![(0, pts.len() - 1, t0, t1)];

    while let Some((a, b, ta, tb)) = stack.pop() {
        // Dos puntos no dan curvatura, y un tramo que no se aparta de su cuerda
        // es una recta: sale más corta que una cúbica y dice la verdad.
        if b - a < 2 || straight(&pts[a..=b], tolerance) {
            out.push(Seg {
                controls: None,
                to: b,
            });
            continue;
        }

        let sub = &pts[a..=b];
        let mut u = chord_params(sub);
        let mut curve = bezier(sub, &u, ta, tb);
        let (mut err, mut worst) = deviation(sub, &u, &curve);

        // Reajustar los parámetros mueve cada punto al sitio de la curva que le
        // pilla más cerca, que es de donde salía el error de casi todos.
        for _ in 0..REPARAM {
            if err <= tolerance * tolerance {
                break;
            }
            u = reparam(sub, &u, &curve);
            curve = bezier(sub, &u, ta, tb);
            (err, worst) = deviation(sub, &u, &curve);
        }

        if err <= tolerance * tolerance {
            out.push(Seg {
                controls: Some((curve.1, curve.2)),
                to: b,
            });
            continue;
        }

        // Partir por el punto peor. El corte tiene que quedar por dentro o esto
        // no termina.
        let k = a + worst.clamp(1, b - a - 1);
        // La misma tangente para los dos lados: el corte es del ajuste, no del
        // dibujo, y ahí no debe verse nada.
        let t = center(pts, k);
        stack.push((k, b, t, tb));
        stack.push((a, k, ta, neg(t)));
    }
    merge_lines(pts, out, tolerance)
}

/// Vuelve a simplificar con RDP las rachas de rectas seguidas.
///
/// Hace falta porque los cortes de arriba los decide el error de la **curva**, y
/// donde no hay curva que valga eso deja las rectas partidas por donde no toca:
/// medido en una imagen del corpus, un 40% más de tramos que el ajuste de
/// polígono sobre el mismo contorno. RDP los coloca donde van, y como sólo elige
/// vértices que ya estaban, el techo de la tolerancia sigue en pie.
fn merge_lines(pts: &[Pt], segs: Vec<Seg>, tolerance: f64) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::with_capacity(segs.len());
    let mut from = 0;
    let mut run: Option<usize> = None;

    for seg in segs {
        match (seg.controls, run) {
            // Empieza una racha, o sigue la que ya había.
            (None, None) => run = Some(from),
            (None, Some(_)) => {}
            // Una curva la corta: se remata lo acumulado y se deja pasar.
            (Some(_), start) => {
                if let Some(start) = start {
                    push_simplified(&mut out, pts, start, from, tolerance);
                    run = None;
                }
                out.push(seg);
            }
        }
        from = seg.to;
    }
    if let Some(start) = run {
        push_simplified(&mut out, pts, start, from, tolerance);
    }
    out
}

fn push_simplified(out: &mut Vec<Seg>, pts: &[Pt], a: usize, b: usize, tolerance: f64) {
    for i in rdp_keep(&pts[a..=b], tolerance).into_iter().skip(1) {
        out.push(Seg {
            controls: None,
            to: a + i,
        });
    }
}

/* ------------------------------------------------------------- esquinas --- */

/// Los índices interiores en los que el contorno hace esquina.
fn inner_corners(pts: &[Pt], closed: bool) -> Vec<usize> {
    let n = pts.len();
    let range = if closed { 0..n } else { 1..n - 1 };
    range
        .filter(|&i| {
            let (back, fwd) = window(i, n, closed);
            match (back, fwd) {
                (Some(back), Some(fwd)) => {
                    let a = sub(pts[i], pts[back]);
                    let b = sub(pts[fwd], pts[i]);
                    cos(a, b) < CORNER_COS
                }
                _ => false,
            }
        })
        .collect()
}

/// Los dos extremos de la ventana alrededor de `i`, recortada en las puntas de
/// una cadena abierta y dando la vuelta en una cerrada.
fn window(i: usize, n: usize, closed: bool) -> (Option<usize>, Option<usize>) {
    if closed {
        return (Some((i + n - WINDOW % n) % n), Some((i + WINDOW) % n));
    }
    let back = i.saturating_sub(WINDOW);
    let fwd = (i + WINDOW).min(n - 1);
    ((back < i).then_some(back), (fwd > i).then_some(fwd))
}

/* ------------------------------------------------------------ tangentes --- */

/// Tangente unitaria que sale de `at` hacia `limit`.
///
/// Se mide contra un punto a varios pasos y no contra el vecino inmediato: en la
/// retícula el vecino siempre está en un eje, así que esa tangente sólo podría
/// apuntar a cuatro sitios.
fn forward(pts: &[Pt], at: usize, limit: usize) -> Pt {
    unit(sub(pts[(at + WINDOW).min(limit)], pts[at]))
}

/// La misma, mirando hacia atrás desde `at`. `limit` es el principio del tramo:
/// la ventana no se sale de él, porque lo de más allá es otra curva.
fn backward(pts: &[Pt], at: usize, limit: usize) -> Pt {
    unit(sub(pts[at.saturating_sub(WINDOW).max(limit)], pts[at]))
}

/// Tangente centrada en `at`, mirando a los dos lados **dentro** del tramo.
///
/// Es la de un corte por error: al salir del mismo número por los dos lados, los
/// dos trozos que se encuentran ahí lo hacen en la misma dirección y el corte no
/// se ve.
fn center(pts: &[Pt], at: usize) -> Pt {
    let back = at.saturating_sub(WINDOW);
    let fwd = (at + WINDOW).min(pts.len() - 1);
    unit(sub(pts[fwd], pts[back]))
}

/// La centrada de un contorno cerrado, que sí da la vuelta. `pts` tiene que ser
/// el ciclo **sin** repetir el primer punto, o el módulo cuenta un paso de menos.
fn across(pts: &[Pt], at: usize) -> Pt {
    let n = pts.len();
    let back = (at + n - WINDOW % n) % n;
    let fwd = (at + WINDOW) % n;
    unit(sub(pts[fwd], pts[back]))
}

/* -------------------------------------------------------------- Bézier --- */

/// Una cúbica por sus cuatro puntos de control.
type Cubic = (Pt, Pt, Pt, Pt);

/// Parámetros por longitud de cuerda, normalizados a `[0, 1]`.
fn chord_params(pts: &[Pt]) -> Vec<f64> {
    let mut u = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    u.push(0.0);
    for i in 1..pts.len() {
        acc += dist(pts[i - 1], pts[i]);
        u.push(acc);
    }
    if acc > 0.0 {
        for v in &mut u {
            *v /= acc;
        }
    }
    u
}

/// La cúbica de mínimos cuadrados que pasa por los extremos con las tangentes
/// dadas. Sólo quedan libres las dos distancias a las que se ponen los
/// controles, y salen de un sistema de dos por dos.
fn bezier(pts: &[Pt], u: &[f64], t0: Pt, t1: Pt) -> Cubic {
    let (p0, p3) = (pts[0], pts[pts.len() - 1]);
    let (mut c00, mut c01, mut c11) = (0.0, 0.0, 0.0);
    let (mut x0, mut x1) = (0.0, 0.0);

    for (i, &ui) in u.iter().enumerate() {
        let (b0, b1, b2, b3) = bernstein(ui);
        let a0 = scale(t0, b1);
        let a1 = scale(t1, b2);
        c00 += dot(a0, a0);
        c01 += dot(a0, a1);
        c11 += dot(a1, a1);
        let tmp = sub(pts[i], add(scale(p0, b0 + b1), scale(p3, b2 + b3)));
        x0 += dot(a0, tmp);
        x1 += dot(a1, tmp);
    }

    let det = c00 * c11 - c01 * c01;
    // Cuerda de reserva cuando el sistema es degenerado o pide poner un control
    // detrás del extremo, que dibujaría un lazo.
    let fallback = dist(p0, p3) / 3.0;
    let (mut alpha0, mut alpha1) = if det.abs() > 1e-12 {
        ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
    } else {
        (fallback, fallback)
    };
    if alpha0 <= 0.0 || alpha1 <= 0.0 {
        alpha0 = fallback;
        alpha1 = fallback;
    }

    (
        p0,
        add(p0, scale(t0, alpha0)),
        add(p3, scale(t1, alpha1)),
        p3,
    )
}

fn bernstein(u: f64) -> (f64, f64, f64, f64) {
    let v = 1.0 - u;
    (v * v * v, 3.0 * u * v * v, 3.0 * u * u * v, u * u * u)
}

fn at(c: &Cubic, u: f64) -> Pt {
    let (b0, b1, b2, b3) = bernstein(u);
    add(
        add(scale(c.0, b0), scale(c.1, b1)),
        add(scale(c.2, b2), scale(c.3, b3)),
    )
}

/// El error al cuadrado más grande, y en qué punto.
fn deviation(pts: &[Pt], u: &[f64], c: &Cubic) -> (f64, usize) {
    let mut worst = (0.0, 0);
    for (i, &ui) in u.iter().enumerate() {
        let d = sub(at(c, ui), pts[i]);
        let d2 = dot(d, d);
        if d2 > worst.0 {
            worst = (d2, i);
        }
    }
    worst
}

/// Mueve cada parámetro al punto de la curva que le queda más cerca, con un paso
/// de Newton sobre la derivada de la distancia.
fn reparam(pts: &[Pt], u: &[f64], c: &Cubic) -> Vec<f64> {
    u.iter()
        .enumerate()
        .map(|(i, &ui)| {
            let d = sub(at(c, ui), pts[i]);
            let d1 = derivative(c, ui);
            let d2 = derivative2(c, ui);
            let den = dot(d1, d1) + dot(d, d2);
            if den.abs() < 1e-12 {
                ui
            } else {
                (ui - dot(d, d1) / den).clamp(0.0, 1.0)
            }
        })
        .collect()
}

fn derivative(c: &Cubic, u: f64) -> Pt {
    let v = 1.0 - u;
    let a = scale(sub(c.1, c.0), 3.0 * v * v);
    let b = scale(sub(c.2, c.1), 6.0 * v * u);
    let d = scale(sub(c.3, c.2), 3.0 * u * u);
    add(add(a, b), d)
}

fn derivative2(c: &Cubic, u: f64) -> Pt {
    let v = 1.0 - u;
    let a = scale(sub(add(c.2, c.0), scale(c.1, 2.0)), 6.0 * v);
    let b = scale(sub(add(c.3, c.1), scale(c.2, 2.0)), 6.0 * u);
    add(a, b)
}

/// Si toda la polilínea cabe dentro de la tolerancia alrededor de su cuerda.
fn straight(pts: &[Pt], tolerance: f64) -> bool {
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let limit = tolerance * tolerance;
    pts.iter().all(|&p| {
        let ap = sub(p, a);
        if len2 == 0.0 {
            return dot(ap, ap) <= limit;
        }
        let cross = ab.0 * ap.1 - ab.1 * ap.0;
        cross * cross / len2 <= limit
    })
}

/* ------------------------------------------------------------ vectores --- */

fn real(points: &[Point]) -> Vec<Pt> {
    points.iter().map(|p| (p.0 as f64, p.1 as f64)).collect()
}

fn plain(p: Pt) -> Vertex {
    Vertex {
        p,
        cin: None,
        cout: None,
    }
}

fn add(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

fn scale(a: Pt, k: f64) -> Pt {
    (a.0 * k, a.1 * k)
}

fn neg(a: Pt) -> Pt {
    (-a.0, -a.1)
}

fn dot(a: Pt, b: Pt) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn dist(a: Pt, b: Pt) -> f64 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

fn unit(a: Pt) -> Pt {
    let len = dot(a, a).sqrt();
    if len == 0.0 {
        (0.0, 0.0)
    } else {
        scale(a, 1.0 / len)
    }
}

/// Coseno del ángulo entre dos vectores. Cero si alguno es nulo, que en la
/// detección de esquinas se lee como esquina y es lo prudente.
fn cos(a: Pt, b: Pt) -> f64 {
    let len = (dot(a, a) * dot(b, b)).sqrt();
    if len == 0.0 {
        0.0
    } else {
        dot(a, b) / len
    }
}
