//! Retirada del fondo plano y recorte al contenido.
//!
//! Muchas imágenes traen el dibujo sobre un color liso en vez de sobre
//! transparencia. Aquí se identifica ese color por el borde del lienzo, se
//! vacía sólo la parte que llega desde fuera —lo mismo de ese color encerrado
//! dentro del dibujo se queda— y se recorta el resultado a lo que queda.

use crate::color::Rgba;
use crate::grid::PixelMap;

/// Proporción del borde que debe ocupar un color para tomarlo por fondo.
const BORDER_SHARE: f64 = 0.5;

/// Vacía el fondo liso, si lo hay, y devuelve el color retirado.
pub fn remove(map: &mut PixelMap) -> Option<Rgba> {
    let color = border_color(map)?;

    // Se entra desde el borde y sólo se avanza por píxeles de ese color: el
    // mismo tono en el interior del dibujo (el blanco de un ojo, otra vez) no
    // se toca porque no hay camino hasta él.
    let (w, h) = (map.width, map.height);
    let mut stack: Vec<usize> = Vec::new();
    let mut seen = vec![false; w * h];
    let push = |i: usize, stack: &mut Vec<usize>, seen: &mut Vec<bool>| {
        if !seen[i] && map.pixels[i] == Some(color) {
            seen[i] = true;
            stack.push(i);
        }
    };
    for x in 0..w {
        push(x, &mut stack, &mut seen);
        push((h - 1) * w + x, &mut stack, &mut seen);
    }
    for y in 0..h {
        push(y * w, &mut stack, &mut seen);
        push(y * w + w - 1, &mut stack, &mut seen);
    }

    let mut removed = 0;
    while let Some(i) = stack.pop() {
        map.pixels[i] = None;
        removed += 1;
        let (x, y) = (i % w, i / w);
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < w && ny < h {
                let j = ny * w + nx;
                if !seen[j] && map.pixels[j] == Some(color) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
    }

    (removed > 0).then_some(color)
}

/// Color que domina el borde del lienzo, si alguno lo hace de verdad.
fn border_color(map: &PixelMap) -> Option<Rgba> {
    let (w, h) = (map.width, map.height);
    if w == 0 || h == 0 {
        return None;
    }
    let border = (0..w)
        .map(|x| (x, 0))
        .chain((0..w).map(|x| (x, h - 1)))
        .chain((0..h).map(|y| (0, y)))
        .chain((0..h).map(|y| (w - 1, y)));

    let mut counts: Vec<(Rgba, usize)> = Vec::new();
    let mut total = 0;
    for (x, y) in border {
        total += 1;
        // Un borde ya transparente no tiene fondo que quitar, pero cuenta para
        // que un color minoritario no se lleve el puesto.
        let Some(color) = map.pixels[y * w + x] else {
            continue;
        };
        match counts.iter_mut().find(|(c, _)| *c == color) {
            Some((_, n)) => *n += 1,
            None => counts.push((color, 1)),
        }
    }

    counts
        .into_iter()
        .max_by_key(|&(_, n)| n)
        .filter(|&(_, n)| n as f64 >= total as f64 * BORDER_SHARE)
        .map(|(color, _)| color)
}

/// Recorta el mapa al rectángulo que ocupa el contenido visible.
pub fn trim(map: PixelMap) -> PixelMap {
    let (w, h) = (map.width, map.height);
    let visible: Vec<(usize, usize)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| map.pixels[y * w + x].is_some())
        .collect();

    let Some(&(first_x, first_y)) = visible.first() else {
        return map;
    };
    let (mut x0, mut y0, mut x1, mut y1) = (first_x, first_y, first_x, first_y);
    for (x, y) in visible {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    if (x0, y0, x1, y1) == (0, 0, w - 1, h - 1) {
        return map;
    }

    let (width, height) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut pixels = Vec::with_capacity(width * height);
    for y in y0..=y1 {
        pixels.extend_from_slice(&map.pixels[y * w + x0..y * w + x1 + 1]);
    }
    PixelMap {
        width,
        height,
        pixels,
    }
}
