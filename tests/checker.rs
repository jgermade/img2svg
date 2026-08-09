//! Detección de la cuadrícula de transparencia.

use image::{Rgba as ImgRgba, RgbaImage};
use px2svg::checker;

/// Damero de 8 px con un cuadrado opaco encima.
fn sample(cell: u32, art: bool) -> RgbaImage {
    let mut img = RgbaImage::new(160, 160);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let dark = ((x / cell) + (y / cell)) % 2 == 1;
        *p = if dark {
            ImgRgba([204, 204, 204, 255])
        } else {
            ImgRgba([255, 255, 255, 255])
        };
    }
    if art {
        for y in 40..90 {
            for x in 40..90 {
                img.put_pixel(x, y, ImgRgba([200, 20, 30, 255]));
            }
        }
    }
    img
}

#[test]
fn reconoce_el_damero_y_lo_deja_transparente() {
    let mut img = sample(8, true);
    let found = checker::remove(&mut img).expect("debería detectarse");
    assert_eq!(found.cell, (8.0, 8.0));
    assert!(found.coverage > 0.6, "cobertura {}", found.coverage);

    // El fondo desaparece y el dibujo se queda.
    assert_eq!(img.get_pixel(4, 4).0[3], 0);
    assert_eq!(img.get_pixel(12, 4).0[3], 0);
    assert_eq!(img.get_pixel(60, 60).0, [200, 20, 30, 255]);
}

#[test]
fn respeta_un_pixel_del_dibujo_que_coincide_con_el_damero() {
    let mut img = sample(8, true);
    // Blanco suelto dentro del cuadrado rojo, en una casilla que ya no cuadra.
    img.put_pixel(60, 60, ImgRgba([255, 255, 255, 255]));
    checker::remove(&mut img).expect("debería detectarse");
    assert_eq!(img.get_pixel(60, 60).0[3], 255);
}

/// Un plano blanco del dibujo cuadra a la perfección con las casillas claras
/// del damero. Lo que lo delata es que sus vecinas no alternan.
#[test]
fn respeta_un_plano_blanco_del_dibujo() {
    let mut img = sample(8, false);
    // Cinco casillas por lado de blanco puro, alineadas con la rejilla.
    for y in 40..80 {
        for x in 40..80 {
            img.put_pixel(x, y, ImgRgba([255, 255, 255, 255]));
        }
    }
    checker::remove(&mut img).expect("el fondo sigue siendo damero");

    // El interior del plano se queda; el borde toca fondo y es indistinguible.
    for y in 50..70 {
        for x in 50..70 {
            assert_eq!(
                img.get_pixel(x, y).0[3],
                255,
                "se ha borrado el plano blanco en ({x}, {y})"
            );
        }
    }
    assert_eq!(img.get_pixel(4, 4).0[3], 0, "el fondo sí debe irse");
}

#[test]
fn casillas_de_otro_tamano() {
    let mut img = sample(16, false);
    assert_eq!(
        checker::remove(&mut img).map(|c| c.cell),
        Some((16.0, 16.0))
    );
}

#[test]
fn no_ve_dameros_donde_no_los_hay() {
    let mut img = RgbaImage::from_pixel(64, 64, ImgRgba([255, 255, 255, 255]));
    for (x, y, p) in img.enumerate_pixels_mut() {
        if (x / 7 + y / 3) % 3 == 0 {
            *p = ImgRgba([20, 130, 200, 255]);
        }
    }
    assert!(checker::remove(&mut img).is_none());
}
