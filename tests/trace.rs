//! Trazado de contornos: bucles cerrados y regiones conexas.

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

#[test]
fn un_pixel_da_un_cuadrado() {
    let loops = traced(&[".....", ".....", "..#..", ".....", "....."]);
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].len(), 4);
    let mut points = loops[0].clone();
    points.sort();
    assert_eq!(points, vec![(2, 2), (2, 3), (3, 2), (3, 3)]);
}

#[test]
fn un_rectangulo_se_une_en_un_solo_contorno() {
    let loops = traced(&["....", ".###", ".###", "...."]);
    assert_eq!(loops.len(), 1);
    // Cuatro esquinas: los píxeles intermedios se funden en tramos rectos.
    assert_eq!(loops[0].len(), 4);
}

#[test]
fn un_hueco_genera_su_propio_bucle() {
    let loops = traced(&["#####", "#...#", "#...#", "#...#", "#####"]);
    assert_eq!(loops.len(), 2);
    let mut sizes: Vec<usize> = loops.iter().map(|l| l.len()).collect();
    sizes.sort();
    assert_eq!(sizes, vec![4, 4]);
}

#[test]
fn las_diagonales_no_comparten_contorno() {
    let loops = traced(&["#..", ".#.", "..#"]);
    assert_eq!(loops.len(), 3);
    assert!(loops.iter().all(|l| l.len() == 4));
}

#[test]
fn una_forma_en_l_conserva_sus_seis_esquinas() {
    let loops = traced(&["##.", "##.", "###"]);
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].len(), 6);
}

#[test]
fn mascara_vacia_no_da_contornos() {
    assert!(traced(&["..", ".."]).is_empty());
}
