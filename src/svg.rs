//! Generación del documento SVG.
//!
//! Cada bloque de píxeles contiguos del mismo color es un `<path>`, y todos los
//! bloques de un color van dentro de un `<g fill="…">`. Así el documento se
//! puede editar bloque a bloque en un editor vectorial en vez de tener una sola
//! figura por color repartida por todo el dibujo.

use std::collections::HashMap;

use crate::color::Rgba;
use crate::grid::PixelMap;
use crate::trace::{self, Point};

/// Cómo se reparten los píxeles entre los `<path>` del documento.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Grouping {
    /// Un path por bloque de píxeles contiguos.
    #[default]
    Region,
    /// Un único path por color, con todos sus bloques como subtrazados.
    Color,
}

pub struct Options {
    /// Tamaño de render de cada píxel lógico. El `viewBox` va siempre en píxeles
    /// del dibujo (1 unidad = 1 píxel); esto sólo fija `width`/`height`.
    pub pixel_size: u32,
    /// Color de fondo opcional (se emite como rectángulo bajo los paths).
    pub background: Option<String>,
    pub grouping: Grouping,
}

pub struct Output {
    pub svg: String,
    pub colors: usize,
    /// Elementos `<path>` emitidos.
    pub paths: usize,
    /// Subtrazados, sumando los de todos los paths.
    pub subpaths: usize,
}

pub fn render(map: &PixelMap, opts: &Options) -> Output {
    let mut order: Vec<Rgba> = Vec::new();
    let mut counts: HashMap<Rgba, usize> = HashMap::new();
    for pixel in map.pixels.iter().flatten() {
        if counts.insert(*pixel, 0).is_none() {
            order.push(*pixel);
        }
        *counts.get_mut(pixel).unwrap() += 1;
    }
    // Los colores más presentes primero: paths grandes al fondo del documento.
    order.sort_by(|a, b| counts[b].cmp(&counts[a]).then(a.to_hex().cmp(&b.to_hex())));

    let (w, h) = (map.width as i64, map.height as i64);
    let scale = opts.pixel_size.max(1) as i64;

    let mut body = String::new();
    if let Some(bg) = &opts.background {
        body.push_str(&format!(
            "  <rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>\n"
        ));
    }

    let mut scratch = vec![false; map.pixels.len()];
    let mut total_paths = 0;
    let mut total_subpaths = 0;

    for color in &order {
        let mask: Vec<bool> = map
            .pixels
            .iter()
            .map(|p| p.as_ref() == Some(color))
            .collect();

        let blocks: Vec<Vec<Vec<Point>>> = match opts.grouping {
            Grouping::Color => vec![trace::trace(&mask, map.width, map.height)],
            Grouping::Region => trace::components(&mask, map.width, map.height)
                .into_iter()
                .map(|region| {
                    for &i in &region {
                        scratch[i] = true;
                    }
                    let loops = trace::trace(&scratch, map.width, map.height);
                    for &i in &region {
                        scratch[i] = false;
                    }
                    loops
                })
                .collect(),
        };

        let paths: Vec<String> = blocks
            .iter()
            .filter(|loops| !loops.is_empty())
            .map(|loops| {
                total_subpaths += loops.len();
                let d: String = loops.iter().map(|points| path_data(points)).collect();
                // El relleno par-impar sólo hace falta cuando el bloque tiene
                // agujeros, es decir, cuando trae más de un bucle.
                let rule = if loops.len() > 1 {
                    " fill-rule=\"evenodd\""
                } else {
                    ""
                };
                format!("{rule} d=\"{d}\"")
            })
            .collect();

        if paths.is_empty() {
            continue;
        }
        total_paths += paths.len();

        let mut fill = format!(" fill=\"{}\"", color.to_hex());
        if color.a < 255 {
            fill.push_str(&format!(
                " fill-opacity=\"{}\"",
                trim_float(color.a as f64 / 255.0)
            ));
        }

        if paths.len() == 1 {
            body.push_str(&format!("  <path{fill}{}/>\n", paths[0]));
        } else {
            body.push_str(&format!("  <g{fill}>\n"));
            for path in &paths {
                body.push_str(&format!("    <path{path}/>\n"));
            }
            body.push_str("  </g>\n");
        }
    }

    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" \
viewBox=\"0 0 {w} {h}\" shape-rendering=\"crispEdges\">\n{body}</svg>\n",
        w * scale,
        h * scale
    );

    Output {
        svg,
        colors: order.len(),
        paths: total_paths,
        subpaths: total_subpaths,
    }
}

/// Un subtrazado cerrado. Al ser todos los tramos horizontales o verticales se
/// usan los comandos relativos `h`/`v`, que ocupan la mitad.
fn path_data(points: &[Point]) -> String {
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

fn trim_float(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
