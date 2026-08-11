//! Trazado de contornos: bucles cerrados y regiones conexas.
//!
//! `trace` devuelve la polilínea **densa**, con un punto por esquina de píxel
//! recorrida; colapsar los tramos rectos es cosa de `fit::simplify`. Cada caso
//! comprueba las dos cifras, porque el reparto entre ambas etapas es justo lo
//! que se puede perder sin que el dibujo cambie: un `simplify` olvidado da el
//! mismo SVG con los paths llenos de vértices colineales.

use img2svg::fit::simplify;
use img2svg::trace::{self, Point};

/// Construye una máscara a partir de un dibujo: `#` marca píxel presente.
fn mask(rows: &[&str]) -> (Vec<bool>, usize, usize) {
    let w = rows[0].len();
    let bits = rows
        .iter()
        .flat_map(|r| r.chars().map(|c| c == '#'))
        .collect();
    (bits, w, rows.len())
}

fn traced(rows: &[&str]) -> Vec<Vec<Point>> {
    let (bits, w, h) = mask(rows);
    trace::trace(&bits, w, h)
}

/// `simplify` trabaja en reales desde que los contornos pueden salirse de la
/// retícula ([`img2svg::subpixel`]); el trazado sigue dando enteros, así que aquí
/// se convierten para llamarla y se vuelve a enteros para poder afirmar sobre
/// ellos. Los valores son exactos en `f64`, así que la ida y vuelta no pierde
/// nada.
fn simplified(points: &[Point]) -> Vec<Point> {
    let real: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (x.into(), y.into())).collect();
    simplify(&real)
        .into_iter()
        .map(|(x, y)| (x as i32, y as i32))
        .collect()
}

/// Puntos de cada bucle, densos y tras simplificar.
fn sizes(loops: &[Vec<Point>]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = loops
        .iter()
        .map(|l| (l.len(), simplified(l).len()))
        .collect();
    out.sort();
    out
}

#[test]
fn un_pixel_da_un_cuadrado() {
    let loops = traced(&[".....", ".....", "..#..", ".....", "....."]);
    assert_eq!(loops.len(), 1);
    // Cuatro lados de una unidad: no hay nada que colapsar.
    assert_eq!(sizes(&loops), vec![(4, 4)]);
    let mut points = simplified(&loops[0]);
    points.sort();
    assert_eq!(points, vec![(2, 2), (2, 3), (3, 2), (3, 3)]);
}

#[test]
fn un_rectangulo_se_une_en_un_solo_contorno() {
    let loops = traced(&["....", ".###", ".###", "...."]);
    assert_eq!(loops.len(), 1);
    // Perímetro de 3x2 = 10 lados unitarios, que se colapsan a cuatro esquinas.
    assert_eq!(sizes(&loops), vec![(10, 4)]);
}

#[test]
fn un_hueco_genera_su_propio_bucle() {
    let loops = traced(&["#####", "#...#", "#...#", "#...#", "#####"]);
    assert_eq!(loops.len(), 2);
    // El agujero de 3x3 da 12 lados; el contorno de 5x5, 20. Ambos, 4 esquinas.
    assert_eq!(sizes(&loops), vec![(12, 4), (20, 4)]);
}

#[test]
fn las_diagonales_no_comparten_contorno() {
    let loops = traced(&["#..", ".#.", "..#"]);
    assert_eq!(loops.len(), 3);
    assert_eq!(sizes(&loops), vec![(4, 4); 3]);
}

#[test]
fn una_forma_en_l_conserva_sus_seis_esquinas() {
    let loops = traced(&["##.", "##.", "###"]);
    assert_eq!(loops.len(), 1);
    assert_eq!(sizes(&loops), vec![(12, 6)]);
}

#[test]
fn mascara_vacia_no_da_contornos() {
    assert!(traced(&["..", ".."]).is_empty());
}

/// Simplificar es idempotente: sobre un contorno ya sin colineales no toca nada.
#[test]
fn simplificar_dos_veces_no_cambia_nada() {
    let loops = traced(&["##.", "##.", "###"]);
    let once = simplified(&loops[0]);
    assert_eq!(simplified(&once), once);
}
