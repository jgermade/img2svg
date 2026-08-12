//! Retirada del fondo plano y recorte al contenido.
//!
//! Muchas imágenes traen el dibujo sobre un color liso en vez de sobre
//! transparencia. Aquí se identifica ese color por el borde del lienzo, se
//! vacía sólo la parte que llega desde fuera —lo mismo de ese color encerrado
//! dentro del dibujo se queda— y se recorta el resultado a lo que queda.
//!
//! Hay dos versiones porque hay dos formas de tener la imagen. Sobre un
//! [`PixelMap`] hace falta el relleno por inundación, porque los píxeles sólo
//! saben su color. Sobre un [`Clustering`] no: sus regiones **ya** son bloques
//! conexos de un color, así que «lo que llega desde fuera» es exactamente «las
//! regiones que tocan el borde», y no hay nada que recorrer.
//!
//! Sobre la tolerancia que el plan pedía aquí: no hace falta. Los dos caminos
//! unifican los colores parecidos *antes* de llegar —`reduce_palette` en el de
//! rejilla, la paleta en el de clustering—, así que cuando se busca el fondo ya
//! es un color exacto. Comparar por igualdad no es una limitación heredada, es
//! que el trabajo está hecho antes.

#[cfg(feature = "illustration")]
use crate::cluster::{self, Clustering, NONE};
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

/// Vacía el fondo liso de un etiquetado, y devuelve el color retirado.
///
/// Se van las regiones del color que domina el borde **que tocan el borde**. Lo
/// mismo de ese color encerrado dentro del dibujo se queda, igual que en la
/// versión de [`PixelMap`], pero aquí sale de la definición en vez de hacer falta
/// recorrer: una región es ya un bloque conexo, así que o llega al borde o no.
#[cfg(feature = "illustration")]
pub fn remove_clustered(clustering: &mut Clustering) -> Option<Rgba> {
    let color = border_color_of(clustering)?;
    let (w, h) = (clustering.width, clustering.height);

    let mut doomed = vec![false; clustering.clusters.len()];
    let mut border = |i: usize| {
        let label = clustering.labels[i];
        if label != NONE && clustering.clusters[label as usize].color == color {
            doomed[label as usize] = true;
        }
    };
    for x in 0..w {
        border(x);
        border((h - 1) * w + x);
    }
    for y in 0..h {
        border(y * w);
        border(y * w + w - 1);
    }
    if !doomed.iter().any(|&d| d) {
        return None;
    }

    for label in clustering.labels.iter_mut() {
        if *label != NONE && doomed[*label as usize] {
            *label = NONE;
        }
    }

    // Las regiones vaciadas se quedan sin píxeles, y reordenar es justo lo que
    // descarta las que no tienen ninguno.
    let identity: Vec<u32> = (0..clustering.clusters.len() as u32).collect();
    let colors: Vec<Rgba> = clustering.clusters.iter().map(|k| k.color).collect();
    let labels = std::mem::take(&mut clustering.labels);
    *clustering = cluster::gather(labels, w, h, &identity, &colors);
    clustering.background = Some(color);
    Some(color)
}

/// Color que domina el borde de un etiquetado, si alguno lo hace de verdad.
#[cfg(feature = "illustration")]
fn border_color_of(clustering: &Clustering) -> Option<Rgba> {
    let (w, h) = (clustering.width, clustering.height);
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
        let label = clustering.labels[y * w + x];
        if label == NONE {
            continue;
        }
        let color = clustering.clusters[label as usize].color;
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

/// Recorta un etiquetado al rectángulo que ocupa lo visible.
///
/// Las regiones no cambian: el rectángulo es el de **todos** los píxeles
/// visibles, así que ninguna se queda fuera ni pierde un píxel.
#[cfg(feature = "illustration")]
pub fn trim_clustered(clustering: &mut Clustering) {
    let (w, h) = (clustering.width, clustering.height);
    let visible = || {
        clustering
            .labels
            .iter()
            .enumerate()
            .filter(|&(_, &label)| label != NONE)
            .map(|(i, _)| (i % w, i / w))
    };
    let Some((first_x, first_y)) = visible().next() else {
        return;
    };
    let (mut x0, mut y0, mut x1, mut y1) = (first_x, first_y, first_x, first_y);
    for (x, y) in visible() {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    if (x0, y0, x1, y1) == (0, 0, w - 1, h - 1) {
        return;
    }

    let (width, height) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut labels = Vec::with_capacity(width * height);
    for y in y0..=y1 {
        labels.extend_from_slice(&clustering.labels[y * w + x0..y * w + x1 + 1]);
    }
    clustering.labels = labels;
    clustering.width = width;
    clustering.height = height;
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
