//! Ajuste: del contorno de una región a los datos de un `<path>`.
//!
//! Es el otro eje de la conversión, ortogonal a la segmentación: cualquier
//! ajustador sirve para cualquier segmentación. Hoy existen [`Fit::Pixel`], la
//! escalera literal, y [`Fit::Polygon`], que la simplifica a segmentos rectos;
//! `Spline` entra con la fase 4.
//!
//! # Se ajusta por media arista, y sólo después se ensamblan los anillos
//!
//! El orden importa y no es el evidente. La representación intermedia extrae
//! **cada frontera una sola vez** ([`crate::region`]) precisamente para que las
//! dos caras que la comparten reciban la misma geometría; si el ajuste
//! trabajara sobre el anillo ya ensamblado, esa garantía se perdería en el
//! último paso: un nodo donde se juntan tres regiones no es más que un vértice
//! interior para el simplificador, que lo descartaría por recto, y la misma
//! frontera se simplificaría dos veces —dentro de dos anillos distintos, con
//! vecinos distintos a cada lado del nodo— saliendo distinta. Entre las dos
//! asomaría el pelo de fondo que toda esta estructura existe para evitar.
//!
//! Así que [`Fitted`] ajusta cada `EdgeId` una vez, y [`Fitted::ring_data`]
//! ensambla el anillo a partir de tramos ya ajustados. Con `Pixel` sobre
//! coordenadas enteras la diferencia no se ve —colapsar los colineales a un
//! lado y a otro del nodo da la misma escalera—, que es justo por lo que esto
//! podía llevar tanto tiempo mal sin que nada lo notara.
//!
//! El único paso que sí mira el anillo entero es el de después: quitar los
//! vértices que quedan **exactamente** sobre la recta entre sus vecinos, que es
//! lo que pasa en un nodo por el que la frontera pasa de largo. Eso no es
//! ajustar, es no escribir un punto que no dibuja nada: la curva resultante es
//! idéntica, así que las dos caras siguen coincidiendo aunque una lo quite y la
//! otra no.

use crate::region::{self, Regions, Ring};
use crate::trace::Point;

/// Cómo se convierte un contorno en datos de path.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Fit {
    /// La escalera literal del contorno, con comandos `h`/`v`.
    #[default]
    Pixel,
    /// Segmentos rectos, quedándose con los vértices que dibujan algo
    /// (Ramer–Douglas–Peucker). `tolerance` es la desviación máxima admitida,
    /// en píxeles del lienzo.
    Polygon { tolerance: f64 },
}

impl Fit {
    /// Desviación por defecto del ajuste de polígono.
    ///
    /// El escalón de una diagonal a 45° se aparta `1/√2 ≈ 0.707` de su cuerda,
    /// que es el número que manda aquí: por debajo no se endereza ni una
    /// diagonal y el ajuste no hace nada, y justo por encima se enderezan todas.
    /// Es el valor más pequeño que sirve para algo, y por eso es el de partida.
    ///
    /// Lo que **no** hace es garantizar que un detalle más alto que la
    /// tolerancia sobreviva. RDP mide contra la cuerda que tenga en cada paso de
    /// la recursión, no contra los vecinos del vértice, así que una cuerda que
    /// venga de lejos puede tragarse un píxel que sobresale aunque suelto se
    /// apartara 1.0. Lo único que promete la tolerancia es el techo: ningún
    /// punto del contorno acaba a más de esa distancia de lo que se dibuja.
    ///
    /// Subirla comprime bastante más —de 0.75 a 1.5 son otro 20% de fichero en
    /// el corpus—, a cambio de ir redondeando las esquinas pequeñas.
    pub const POLYGON_TOLERANCE: f64 = 0.75;

    /// El ajuste de polígono con la desviación por defecto.
    pub fn polygon() -> Self {
        Fit::Polygon {
            tolerance: Self::POLYGON_TOLERANCE,
        }
    }
}

/// Los tramos ya ajustados, indexados por `EdgeId` igual que `Regions::edges`.
///
/// Existe como tipo, y no como una función que devuelva el `d` de un anillo,
/// para que el orden sea el que impone la firma: primero se ajusta todo, y sólo
/// con eso en la mano se puede pedir un anillo.
pub struct Fitted(Vec<Vec<Point>>);

impl Fitted {
    /// Ajusta cada media arista de la segmentación, una sola vez.
    pub fn new(regions: &Regions, fit: Fit) -> Self {
        Fitted(
            regions
                .edges
                .iter()
                .map(|edge| chain_fit(&edge.points, fit))
                .collect(),
        )
    }

    /// Datos `d` de un anillo cerrado, ensamblado a partir de sus tramos.
    pub fn ring_data(&self, ring: &Ring) -> String {
        let points = region::chain(ring, |edge| self.0[edge].as_slice());
        subpath(&simplify(&points))
    }
}

/// Ajusta un tramo. Los dos extremos se quedan donde están: son los nodos en
/// los que se encuentran las cadenas vecinas, y moverlos las abriría.
fn chain_fit(points: &[Point], fit: Fit) -> Vec<Point> {
    // Un tramo cerrado repite el primer punto al final; esa repetición es su
    // marca, y hay que devolverla puesta.
    if points.len() > 1 && points[0] == points[points.len() - 1] {
        let mut out = closed_fit(&points[..points.len() - 1], fit);
        if let Some(&first) = out.first() {
            out.push(first);
        }
        return out;
    }
    let straight = simplify_open(points);
    match fit {
        Fit::Pixel => straight,
        Fit::Polygon { tolerance } => rdp(&straight, tolerance),
    }
}

/// Ajusta una cadena cerrada, ya sin el punto repetido del final.
fn closed_fit(points: &[Point], fit: Fit) -> Vec<Point> {
    let straight = simplify(points);
    match fit {
        Fit::Pixel => straight,
        Fit::Polygon { tolerance } => rdp_closed(&straight, tolerance),
    }
}

/// Elimina los vértices intermedios de los tramos rectos, dejando las esquinas.
///
/// Cíclica: el primer punto también se descarta si el contorno pasa recto por
/// él. Vive aquí y no en el trazado porque los ajustadores de curvas quieren la
/// polilínea densa —necesitan los puntos intermedios para estimar tangentes y
/// detectar esquinas—, y porque quitar un punto colineal no cambia la curva:
/// pasa por donde pasaba.
pub fn simplify(points: &[Point]) -> Vec<Point> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .filter(|&i| turns(points[(i + n - 1) % n], points[i], points[(i + 1) % n]))
        .map(|i| points[i])
        .collect()
}

/// Lo mismo sobre una polilínea abierta, conservando los dos extremos.
fn simplify_open(points: &[Point]) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(n);
    out.push(points[0]);
    out.extend(
        (1..n - 1)
            .filter(|&i| turns(points[i - 1], points[i], points[i + 1]))
            .map(|i| points[i]),
    );
    out.push(points[n - 1]);
    out
}

/// Si el camino cambia de dirección al pasar por `cur`.
///
/// Seguir recto es producto vectorial cero **y** avanzar en el mismo sentido:
/// una vuelta de 180° también tiene el vectorial a cero, y ese punto no se
/// puede quitar sin cambiar el dibujo. Se compara así y no por igualdad de
/// deltas porque después del primer colapso los pasos ya no son unitarios: dos
/// tramos rectos seguidos de 3 y 2 unidades son la misma recta con deltas
/// distintos.
fn turns(prev: Point, cur: Point, next: Point) -> bool {
    let a = delta(prev, cur);
    let b = delta(cur, next);
    cross(a, b) != 0 || dot(a, b) <= 0
}

/// Ramer–Douglas–Peucker sobre una polilínea **abierta**.
///
/// Conserva los dos extremos —que es justo lo que pide un tramo compartido: los
/// nodos no se mueven— y descarta los vértices que se apartan menos de
/// `tolerance` de la cuerda. Los puntos que quedan son los que ya estaban: RDP
/// sólo elige, no inventa, así que la salida sigue en coordenadas enteras y
/// `Point` no cambia.
fn rdp(points: &[Point], tolerance: f64) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let limit = tolerance * tolerance;
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;

    // Pila explícita en vez de recursión: el reparto puede salir tan
    // desequilibrado como apartar un punto por nivel, y entonces la profundidad
    // sería la longitud del tramo, que en el contorno de una región grande son
    // miles de puntos.
    let mut stack = vec![(0, n - 1)];
    while let Some((a, b)) = stack.pop() {
        let mut worst = a;
        let mut worst_d = 0.0;
        for i in a + 1..b {
            let d = deviation2(points[i], points[a], points[b]);
            if d > worst_d {
                worst_d = d;
                worst = i;
            }
        }
        if worst_d <= limit {
            continue;
        }
        keep[worst] = true;
        stack.push((a, worst));
        stack.push((worst, b));
    }

    (0..n).filter(|&i| keep[i]).map(|i| points[i]).collect()
}

/// RDP sobre una cadena **cerrada**, ya sin el punto repetido del final.
///
/// Un anillo no tiene extremos que fijar y RDP necesita una cuerda de la que
/// medir, así que hay que elegir dos puntos y dejarlos quietos. Se toman el
/// primero y el más lejano a él, que es aproximadamente el diámetro del anillo:
/// las dos mitades salen parecidas y ninguno de los dos puntos cae en medio de
/// una recta larga, que es donde más se notaría clavarlo.
fn rdp_closed(points: &[Point], tolerance: f64) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let far = (1..n).max_by_key(|&i| dist2(points[0], points[i])).unwrap();

    let mut out = rdp(&points[..=far], tolerance);
    // El punto de corte lo vuelve a traer la segunda mitad.
    out.pop();

    let mut back: Vec<Point> = points[far..].to_vec();
    back.push(points[0]);
    let mut back = rdp(&back, tolerance);
    // Y el cierre del anillo es implícito.
    back.pop();

    out.extend(back);
    out
}

/// Distancia al cuadrado de `p` a la recta que pasa por `a` y `b`.
///
/// Al cuadrado para poder compararla con la tolerancia sin una raíz por punto,
/// y en `f64` porque el producto vectorial de dos puntos de una imagen grande,
/// elevado al cuadrado, se sale de `i64`.
fn deviation2(p: Point, a: Point, b: Point) -> f64 {
    let ab = delta(a, b);
    let ap = delta(a, p);
    let len2 = dot(ab, ab);
    if len2 == 0 {
        // Cuerda degenerada: la distancia es al propio punto.
        return dot(ap, ap) as f64;
    }
    let area = cross(ab, ap) as f64;
    area * area / len2 as f64
}

fn delta(a: Point, b: Point) -> (i64, i64) {
    ((b.0 - a.0) as i64, (b.1 - a.1) as i64)
}

fn cross(a: (i64, i64), b: (i64, i64)) -> i64 {
    a.0 * b.1 - a.1 * b.0
}

fn dot(a: (i64, i64), b: (i64, i64)) -> i64 {
    a.0 * b.0 + a.1 * b.1
}

fn dist2(a: Point, b: Point) -> i64 {
    let d = delta(a, b);
    dot(d, d)
}

/// Un subtrazado cerrado, en comandos relativos.
///
/// Se usan `h`/`v` siempre que se pueda, que ocupan la mitad que `l`; con el
/// ajuste de píxel es siempre, porque todos sus tramos son de un eje. El `l`
/// aparece con el polígono, donde ya hay oblicuas.
fn subpath(points: &[Point]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut d = format!("M{} {}", points[0].0, points[0].1);
    for i in 1..points.len() {
        let (dx, dy) = delta(points[i - 1], points[i]);
        if dy == 0 {
            d.push_str(&format!("h{dx}"));
        } else if dx == 0 {
            d.push_str(&format!("v{dy}"));
        } else if dy < 0 {
            // El signo del segundo número ya hace de separador.
            d.push_str(&format!("l{dx}{dy}"));
        } else {
            d.push_str(&format!("l{dx} {dy}"));
        }
    }
    d.push('z');
    d
}
