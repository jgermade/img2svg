//! Segmentación: de la imagen a un conjunto de regiones.
//!
//! Es uno de los dos ejes de la conversión. Hoy sólo existe la segmentación por
//! rejilla, que da por hecho que el dibujo está sobre una cuadrícula regular y
//! la recupera; la de clustering, para fotos, va aparte cuando exista.

use std::collections::HashMap;

use crate::color::Rgba;
use crate::grid::PixelMap;
use crate::region::{HalfEdge, Region, Regions};
use crate::trace;

/// Qué cuenta como una región.
///
/// Es una decisión de segmentación y no de dibujo: cambia en qué se parte la
/// imagen, no cómo se pinta lo que salga.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Grouping {
    /// Cada bloque de píxeles contiguos es una región.
    #[default]
    Region,
    /// Todo lo que comparte color es una sola región, con un anillo por bloque.
    Color,
}

/// Segmenta un mapa de píxeles lógicos.
///
/// Las regiones salen en orden de emisión: los colores más presentes primero
/// —así los paths grandes quedan al fondo del documento— y, dentro de un color,
/// por posición del primer píxel del bloque.
pub fn from_pixel_map(map: &PixelMap, grouping: Grouping) -> Regions {
    let mut order: Vec<Rgba> = Vec::new();
    let mut counts: HashMap<Rgba, usize> = HashMap::new();
    for pixel in map.pixels.iter().flatten() {
        if counts.insert(*pixel, 0).is_none() {
            order.push(*pixel);
        }
        *counts.get_mut(pixel).unwrap() += 1;
    }
    order.sort_by(|a, b| counts[b].cmp(&counts[a]).then(a.to_hex().cmp(&b.to_hex())));

    let mut regions: Vec<Region> = Vec::new();
    let mut edges: Vec<HalfEdge> = Vec::new();
    let mut scratch = vec![false; map.pixels.len()];

    for color in order {
        let mask: Vec<bool> = map
            .pixels
            .iter()
            .map(|p| p.as_ref() == Some(&color))
            .collect();

        // Cada entrada es (área, bucles del contorno).
        let blocks: Vec<(usize, Vec<Vec<trace::Point>>)> = match grouping {
            Grouping::Color => {
                vec![(counts[&color], trace::trace(&mask, map.width, map.height))]
            }
            Grouping::Region => trace::components(&mask, map.width, map.height)
                .into_iter()
                .map(|block| {
                    for &i in &block {
                        scratch[i] = true;
                    }
                    let loops = trace::trace(&scratch, map.width, map.height);
                    for &i in &block {
                        scratch[i] = false;
                    }
                    (block.len(), loops)
                })
                .collect(),
        };

        for (area, loops) in blocks {
            if loops.is_empty() {
                continue;
            }
            let id = regions.len();
            let rings = loops
                .into_iter()
                .map(|mut points| {
                    // `trace` devuelve el bucle sin repetir el punto de partida;
                    // el tramo sí lo repite, que es como se distingue un tramo
                    // cerrado de uno con dos puntas. `ring_points` lo descarta
                    // otra vez, así que el dibujo no cambia.
                    points.push(points[0]);
                    edges.push(HalfEdge {
                        points,
                        // La rejilla no tiene borde subpíxel que buscar: sus
                        // píxeles son los del dibujo, y la escalera es el dibujo.
                        offsets: Vec::new(),
                        left: id,
                        // La rejilla traza cada región por su cuenta, así que no
                        // hay tramos compartidos que anotar.
                        right: None,
                    });
                    vec![(edges.len() - 1, false)]
                })
                .collect();
            regions.push(Region { color, area, rings });
        }
    }

    Regions {
        width: map.width,
        height: map.height,
        colors: counts.len(),
        regions,
        edges,
    }
}
