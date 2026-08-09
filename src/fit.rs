//! Ajuste: del contorno de una región a los datos de un `<path>`.
//!
//! Es el otro eje de la conversión, ortogonal a la segmentación: cualquier
//! ajustador sirve para cualquier segmentación. Hoy sólo existe [`Fit::Pixel`];
//! `Polygon` y `Spline` entran con las fases 3 y 4.

use crate::region::{Regions, Ring};
use crate::trace::Point;

/// Cómo se convierte un contorno en datos de path.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Fit {
    /// La escalera literal del contorno, con comandos `h`/`v`.
    #[default]
    Pixel,
}

/// Datos `d` de un anillo cerrado.
pub fn ring_data(regions: &Regions, ring: &Ring, fit: Fit) -> String {
    let points = regions.ring_points(ring);
    match fit {
        Fit::Pixel => pixel(&simplify(&points)),
    }
}

/// Elimina los vértices intermedios de los tramos rectos, dejando las esquinas.
///
/// Vive aquí y no en el trazado porque los ajustadores de curvas quieren la
/// polilínea densa: necesitan los puntos intermedios para estimar tangentes y
/// detectar esquinas. Sólo el ajustador de píxel la colapsa.
pub fn simplify(points: &[Point]) -> Vec<Point> {
    let n = points.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = points[(i + n - 1) % n];
        let cur = points[i];
        let next = points[(i + 1) % n];
        if delta(prev, cur) != delta(cur, next) {
            out.push(cur);
        }
    }
    out
}

fn delta(a: Point, b: Point) -> Point {
    (b.0 - a.0, b.1 - a.1)
}

/// Un subtrazado cerrado de tramos horizontales y verticales. Se usan los
/// comandos relativos `h`/`v`, que ocupan la mitad que `L`.
fn pixel(points: &[Point]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut d = format!("M{} {}", points[0].0, points[0].1);
    for i in 1..points.len() {
        let (px, py) = points[i - 1];
        let (x, y) = points[i];
        if y == py {
            d.push_str(&format!("h{}", x - px));
        } else {
            d.push_str(&format!("v{}", y - py));
        }
    }
    d.push('z');
    d
}
