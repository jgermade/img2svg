//! Detección de la rejilla de píxeles y reducción de la imagen.

use image::{Rgba as ImgRgba, RgbaImage};
use px2svg::grid::{self, Axis};

/// Pinta un tablero de celdas `cell`x`cell` con un desplazamiento dado.
fn upscaled(cells_x: u32, cells_y: u32, cell: u32, offset: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(
        cells_x * cell + offset,
        cells_y * cell + offset,
        ImgRgba([0, 0, 0, 255]),
    );
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (cx, cy) = (
            x.saturating_sub(offset) / cell,
            y.saturating_sub(offset) / cell,
        );
        let v = ((cx * 37 + cy * 91) % 5) as u8 * 50;
        *p = ImgRgba([v, 255 - v, (cx as u8).wrapping_mul(29), 255]);
    }
    img
}

#[test]
fn detecta_la_escala_de_una_imagen_ampliada() {
    let img = upscaled(20, 24, 8, 0);
    let (ax, ay) = grid::detect(&img);
    assert_eq!(ax.cell.round(), 8.0);
    assert_eq!(ay.cell.round(), 8.0);
}

#[test]
fn detecta_la_rejilla_aunque_este_desplazada() {
    let img = upscaled(20, 20, 12, 5);
    let (ax, _) = grid::detect(&img);
    assert_eq!(ax.cell.round(), 12.0);
    assert_eq!(ax.offset.round(), 5.0);
}

#[test]
fn la_reduccion_recupera_la_rejilla_original() {
    let cell = 9;
    let img = upscaled(11, 7, cell, 0);
    let (ax, ay) = grid::detect(&img);
    let map = grid::downscale(&img, ax, ay, 128);
    assert_eq!((map.width, map.height), (11, 7));
    for y in 0..map.height {
        for x in 0..map.width {
            let expected = img.get_pixel(x as u32 * cell, y as u32 * cell).0;
            let got = map.pixels[y * map.width + x].unwrap();
            assert_eq!(
                (got.r, got.g, got.b),
                (expected[0], expected[1], expected[2])
            );
        }
    }
}

#[test]
fn una_imagen_sin_rejilla_se_deja_a_escala_1() {
    let mut img = RgbaImage::from_pixel(64, 64, ImgRgba([0, 0, 0, 255]));
    for (x, y, p) in img.enumerate_pixels_mut() {
        let v = ((x * 7919 + y * 104729) % 251) as u8;
        *p = ImgRgba([v, v, v, 255]);
    }
    let (ax, ay) = grid::detect(&img);
    assert_eq!((ax.cell, ay.cell), (1.0, 1.0));
}

#[test]
fn el_alfa_por_debajo_del_umbral_queda_transparente() {
    let img = RgbaImage::from_pixel(4, 4, ImgRgba([10, 20, 30, 40]));
    let map = grid::downscale(&img, Axis::new(1.0, 0.0), Axis::new(1.0, 0.0), 128);
    assert!(map.pixels.iter().all(|p| p.is_none()));
}
