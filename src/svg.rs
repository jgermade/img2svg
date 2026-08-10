//! Generación del documento SVG a partir de las regiones ya ajustadas.
//!
//! Todas las regiones de un color van dentro de un `<g fill="…">`, y cada una es
//! un `<path>`. Así el documento se puede editar bloque a bloque en un editor
//! vectorial en vez de tener una sola figura por color repartida por todo el
//! dibujo.

use crate::fit::{Fit, Fitted};
use crate::region::Regions;

pub struct Options {
    /// Tamaño de render de cada píxel lógico. El `viewBox` va siempre en píxeles
    /// del dibujo (1 unidad = 1 píxel); esto sólo fija `width`/`height`.
    pub pixel_size: u32,
    /// Color de fondo opcional (se emite como rectángulo bajo los paths).
    pub background: Option<String>,
    pub fit: Fit,
}

pub struct Output {
    pub svg: String,
    pub colors: usize,
    /// Elementos `<path>` emitidos.
    pub paths: usize,
    /// Subtrazados, sumando los de todos los paths.
    pub subpaths: usize,
}

pub fn render(regions: &Regions, opts: &Options) -> Output {
    let (w, h) = (regions.width as i64, regions.height as i64);
    let scale = opts.pixel_size.max(1) as i64;

    // Se ajustan **todos** los tramos antes de ensamblar ningún anillo: una
    // frontera compartida se ajusta una sola vez y sus dos caras reciben lo
    // mismo. Ver [`crate::fit`], que es donde se explica por qué el orden no
    // puede ser el otro.
    let fitted = Fitted::new(regions, opts.fit);

    let mut body = String::new();
    if let Some(bg) = &opts.background {
        body.push_str(&format!(
            "  <rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>\n"
        ));
    }

    let mut total_paths = 0;
    let mut total_subpaths = 0;

    // Las regiones llegan con las de un color seguidas, así que basta con
    // avanzar por tramos.
    let mut i = 0;
    while i < regions.regions.len() {
        let color = regions.regions[i].color;
        let end = regions.regions[i..]
            .iter()
            .position(|r| r.color != color)
            .map_or(regions.regions.len(), |n| i + n);

        let paths: Vec<String> = regions.regions[i..end]
            .iter()
            .map(|region| {
                total_subpaths += region.rings.len();
                let d: String = region
                    .rings
                    .iter()
                    .map(|ring| fitted.ring_data(ring))
                    .collect();
                // El relleno par-impar sólo hace falta cuando la región tiene
                // agujeros, es decir, cuando trae más de un anillo.
                let rule = if region.rings.len() > 1 {
                    " fill-rule=\"evenodd\""
                } else {
                    ""
                };
                format!("{rule} d=\"{d}\"")
            })
            .collect();
        i = end;

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

    // `crispEdges` apaga el suavizado, que es lo que quiere una escalera sobre
    // coordenadas enteras: los bordes salen limpios en vez de medio grises. En
    // cuanto hay un tramo oblicuo hace lo contrario —deja la diagonal
    // escalonada, que es exactamente lo que el ajuste acaba de quitar—, así que
    // depende del ajuste y no del documento.
    let rendering = match opts.fit {
        Fit::Pixel => " shape-rendering=\"crispEdges\"",
        Fit::Polygon { .. } => "",
    };

    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" \
viewBox=\"0 0 {w} {h}\"{rendering}>\n{body}</svg>\n",
        w * scale,
        h * scale
    );

    Output {
        svg,
        colors: regions.colors,
        paths: total_paths,
        subpaths: total_subpaths,
    }
}

fn trim_float(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
