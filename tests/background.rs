//! Retirada del fondo plano y recorte al contenido.

use img2svg::background;
use img2svg::color::Rgba;
use img2svg::grid::PixelMap;

const FONDO: Rgba = Rgba {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};
const TINTA: Rgba = Rgba {
    r: 20,
    g: 30,
    b: 40,
    a: 255,
};

/// Construye un mapa a partir de un dibujo: `.` fondo, `#` tinta, ` ` vacío.
fn map(rows: &[&str]) -> PixelMap {
    PixelMap {
        width: rows[0].len(),
        height: rows.len(),
        pixels: rows
            .iter()
            .flat_map(|r| {
                r.chars().map(|c| match c {
                    '.' => Some(FONDO),
                    '#' => Some(TINTA),
                    _ => None,
                })
            })
            .collect(),
    }
}

fn visibles(map: &PixelMap) -> usize {
    map.pixels.iter().flatten().count()
}

#[test]
fn quita_el_fondo_que_rodea_al_dibujo() {
    let mut m = map(&["......", ".####.", ".#..#.", ".####.", "......"]);
    assert_eq!(background::remove(&mut m), Some(FONDO));

    // El marco se va; el hueco interior del mismo color se queda.
    assert_eq!(m.pixels[0], None);
    assert_eq!(visibles(&m), 10 + 2, "10 de tinta y los 2 encerrados");
}

#[test]
fn recorta_el_lienzo_a_lo_que_queda() {
    let mut m = map(&["......", ".####.", ".#..#.", ".####.", "......"]);
    background::remove(&mut m);
    let m = background::trim(m);
    assert_eq!((m.width, m.height), (4, 3));
    assert_eq!(m.pixels[0], Some(TINTA));
}

#[test]
fn no_quita_nada_si_ningun_color_domina_el_borde() {
    let mut m = map(&[" # # ", "#...#", " # # "]);
    assert_eq!(background::remove(&mut m), None);
    assert_eq!(visibles(&m), 9);
}

#[test]
fn un_borde_ya_transparente_no_deja_nada_que_quitar() {
    let mut m = map(&["     ", " ### ", "     "]);
    assert_eq!(background::remove(&mut m), None);
    let m = background::trim(m);
    assert_eq!((m.width, m.height), (3, 1));
}

#[test]
fn el_recorte_respeta_un_mapa_que_ya_va_justo() {
    let m = map(&["###", "###"]);
    let m = background::trim(m);
    assert_eq!((m.width, m.height), (3, 2));
}

#[test]
fn el_fondo_que_asoma_por_una_ranura_se_va_entero() {
    // La bahía comunica con el exterior, así que se vacía; el bolsillo cerrado no.
    let mut m = map(&[
        "........", ".######.", ".#....#.", ".#.##.#.", ".#....#.", ".######.", "........",
    ]);
    background::remove(&mut m);
    let dentro = m.pixels[2 * 8 + 3];
    assert_eq!(dentro, Some(FONDO), "el fondo encerrado se conserva");
    assert_eq!(m.pixels[0], None, "el de fuera se va");
}
