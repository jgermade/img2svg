//! Quitarle a un contorno el temblor de la escalera, sin quitarle las esquinas.
//!
//! # Qué temblor
//!
//! El contorno de una región sale de recorrer grietas entre píxeles, así que un
//! canto oblicuo sale a peldaños. El ajuste sabe deshacer los peldaños
//! *regulares* —una escalera de 45 grados colapsa en su diagonal en cuanto la
//! tolerancia pasa de `raíz(2)/2`—, pero los de un dibujo real no son regulares:
//! un tramo casi recto alterna peldaños de dos y de tres píxeles siguiendo el
//! ruido del original, y esa alternancia se aparta del acorde más de la
//! tolerancia. El simplificador no puede tirarla, porque tirarla sería salirse de
//! lo que promete, así que la escribe. Ampliado a 6x, el resultado es un contorno
//! que tiembla píxel a píxel, y eso es lo que se ve como «pixelado».
//!
//! # Por qué no lo arregla la tolerancia
//!
//! Porque el temblor y el dibujo miden lo mismo. Subir la tolerancia hasta que el
//! temblor entre dentro mete también el borde de cualquier curva pequeña: una
//! lente de gafas sale octogonal. Está medido en
//! `SESSIONS/2026-08-11_12h45.off-the-pixel-lattice.md`, y la conclusión de allí
//! —que el simplificador es un recortador de acordes y no distingue un arco de
//! una escalera— sigue valiendo. Lo que hace falta es mover los vértices, no
//! elegir cuáles se quedan.
//!
//! # Y por qué no estropea las esquinas
//!
//! Porque una esquina se reconoce **a escala**: el giro de un arco está repartido
//! entre muchos vértices y el de una esquina está concentrado en uno. Comparando
//! la dirección del acorde que llega con la del que sale, medidas a [`WINDOW`]
//! puntos de distancia, una escalera de 45 grados no gira nada —los dos acordes
//! van a 45 grados—, un arco de radio 30 px gira unos 8 grados, y una esquina en
//! pico gira 90. Con el corte en [`CORNER_TURN`] no hay confusión posible.
//!
//! # Dónde vive
//!
//! En los desplazamientos de los vértices, el mismo sitio donde
//! [`crate::subpixel`] escribe los suyos, y por el mismo motivo: los dos dicen
//! *dónde está de verdad* un vértice del contorno, y `Fit::Pixel` —que es la
//! escalera literal por definición— los ignora a los dos sin tener que saber que
//! existen. Fundir un tramo se hace una sola vez y sus dos caras lo leen, así que
//! la costura entre regiones vecinas sigue siendo exacta por construcción.

use crate::fit::Pt;
use crate::region::Regions;

/// Cuántos puntos atrás y adelante se mira para juzgar si un vértice es esquina.
///
/// Cuatro: bastante como para que un peldaño de uno o dos píxeles quede dentro
/// del acorde y no lo tuerza, y poco como para que un arco pequeño —una lente de
/// unos 30 px de radio— no parezca una esquina.
const WINDOW: usize = 4;

/// Giro, en radianes, a partir del cual un vértice es esquina y no se toca.
///
/// 50 grados. Un arco de radio `r` gira `WINDOW/r` radianes en la ventana, que
/// para `r = 10 px` son 23 grados: por debajo del corte, así que hasta un arco
/// bastante cerrado se sigue relajando. Una esquina de verdad gira 90.
const CORNER_TURN: f64 = 50.0 * std::f64::consts::PI / 180.0;

/// Pasadas del promediado. Cada una acerca un vértice a la mitad de la distancia
/// entre él y la recta de sus vecinos, así que cuatro dejan un peldaño de un
/// píxel por debajo de una décima; más no cambian nada porque el tope de
/// desplazamiento ya está saturado.
const PASSES: usize = 4;

/// Relaja los contornos de todas las regiones, moviendo cada vértice como mucho
/// `budget` píxeles de su sitio.
///
/// El tope es lo que hace que esto no sea un suavizado: un vértice no puede
/// acabar lejos de donde la imagen dice que está el borde, pase lo que pase con
/// sus vecinos. Con `budget` a cero no hace nada.
pub fn relax(regions: &mut Regions, budget: f64) {
    if budget <= 0.0 {
        return;
    }
    for edge in &mut regions.edges {
        let points = edge.placed();
        let n = points.len();
        if n < 3 {
            continue;
        }
        // Un tramo cerrado repite el primer punto al final: no tiene extremos que
        // clavar y la ventana da la vuelta.
        let closed = points[0] == points[n - 1];
        let relaxed = if closed {
            let mut out = smooth(&points[..n - 1], budget, true);
            out.push(out[0]);
            out
        } else {
            smooth(&points, budget, false)
        };

        edge.offsets = edge
            .points
            .iter()
            .zip(&relaxed)
            .map(|(&(x, y), &(rx, ry))| ((rx - f64::from(x)) as f32, (ry - f64::from(y)) as f32))
            .collect();
    }
}

/// El promediado con tope, sobre una cadena abierta —extremos clavados— o
/// cerrada.
fn smooth(points: &[Pt], budget: f64, closed: bool) -> Vec<Pt> {
    let n = points.len();
    let fixed = corners(points, closed);
    let mut cur = points.to_vec();

    for _ in 0..PASSES {
        let prev = cur.clone();
        for i in 0..n {
            if fixed[i] {
                continue;
            }
            let (before, after) = if closed {
                ((i + n - 1) % n, (i + 1) % n)
            } else {
                if i == 0 || i + 1 == n {
                    continue;
                }
                (i - 1, i + 1)
            };
            // Binomial [1,2,1]/4: la media entre el punto y la recta de sus dos
            // vecinos, que es lo que aplana un peldaño sin arrastrar el tramo.
            let target = (
                (prev[before].0 + 2.0 * prev[i].0 + prev[after].0) / 4.0,
                (prev[before].1 + 2.0 * prev[i].1 + prev[after].1) / 4.0,
            );
            cur[i] = clamped(points[i], target, budget);
        }
    }
    cur
}

/// `target`, o lo más cerca de él que se puede estar sin pasar de `budget` desde
/// `origin`.
fn clamped(origin: Pt, target: Pt, budget: f64) -> Pt {
    let (dx, dy) = (target.0 - origin.0, target.1 - origin.1);
    let d = (dx * dx + dy * dy).sqrt();
    if d <= budget {
        return target;
    }
    let k = budget / d;
    (origin.0 + dx * k, origin.1 + dy * k)
}

/// Qué vértices no se mueven: las esquinas y, en una cadena abierta, los dos
/// extremos.
fn corners(points: &[Pt], closed: bool) -> Vec<bool> {
    let n = points.len();
    (0..n)
        .map(|i| {
            if !closed && (i == 0 || i + 1 == n) {
                return true;
            }
            let (a, b) = if closed {
                // El módulo en el paso atrás es para una cadena cerrada más corta
                // que la ventana, donde restar `WINDOW` se saldría por abajo.
                let atras = WINDOW % n;
                (points[(i + n - atras) % n], points[(i + WINDOW) % n])
            } else {
                (
                    points[i.saturating_sub(WINDOW)],
                    points[(i + WINDOW).min(n - 1)],
                )
            };
            turn(a, points[i], b) > CORNER_TURN
        })
        .collect()
}

/// El giro en `cur`, en radianes: el ángulo entre el acorde que llega y el que
/// sale. Con un acorde de longitud nula no hay giro que medir.
fn turn(a: Pt, cur: Pt, b: Pt) -> f64 {
    let (ux, uy) = (cur.0 - a.0, cur.1 - a.1);
    let (vx, vy) = (b.0 - cur.0, b.1 - cur.1);
    let (nu, nv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
    if nu == 0.0 || nv == 0.0 {
        return 0.0;
    }
    // Con el producto vectorial y el escalar, que es estable en los dos extremos
    // —un `acos` cerca de 1 pierde casi todos los dígitos.
    (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs()
}
