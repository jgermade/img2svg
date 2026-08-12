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
    /// `width`/`height` del documento, cuando no son el lienzo por `pixel_size`.
    ///
    /// Lo usa la escala de trabajo: ahí el lienzo está en píxeles de trabajo —una
    /// unidad del `viewBox` es uno de ellos, que es lo que hace que las
    /// coordenadas salgan tal como se trazaron— y el documento se anuncia al
    /// tamaño de la imagen que llegó, que es el que espera quien la trajo.
    pub display: Option<(usize, usize)>,
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

    let mut total_paths = 0;
    let mut total_subpaths = 0;

    // Los degradados van los primeros, justo encima del fondo. El orden no decide
    // nada —las regiones se reparten el lienzo sin solaparse y las fronteras
    // compartidas se ajustan una sola vez, así que ninguna tapa a otra—, pero la
    // convención del documento es que lo grande quede abajo, y una figura de
    // degradado es la unión de un montón de bandas.
    let mut defs = String::new();
    let mut body = String::new();
    for (i, ramp) in regions.ramps.iter().enumerate() {
        total_paths += 1;
        total_subpaths += ramp.rings.len();
        let stops: String = ramp
            .stops
            .iter()
            .map(|&(at, color)| {
                format!(
                    "<stop offset=\"{}\" stop-color=\"{}\"/>",
                    trim_float(at),
                    color.to_hex()
                )
            })
            .collect();
        defs.push_str(&format!(
            "    <linearGradient id=\"r{i}\" gradientUnits=\"userSpaceOnUse\" \
x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{stops}</linearGradient>\n",
            trim_float(ramp.from.0),
            trim_float(ramp.from.1),
            trim_float(ramp.to.0),
            trim_float(ramp.to.1),
        ));
        let d: String = ramp
            .rings
            .iter()
            .map(|ring| fitted.ring_data(ring))
            .collect();
        // Mismo criterio que en una región: el par-impar sólo hace falta si hay
        // agujeros, y una figura de degradado los tiene en cuanto algo del dibujo
        // cae dentro de la rampa.
        let rule = if ramp.rings.len() > 1 {
            " fill-rule=\"evenodd\""
        } else {
            ""
        };
        body.push_str(&format!("  <path fill=\"url(#r{i})\"{rule} d=\"{d}\"/>\n"));
    }

    let mut head = String::new();
    if !defs.is_empty() {
        head.push_str(&format!("  <defs>\n{defs}  </defs>\n"));
    }
    if let Some(bg) = &opts.background {
        head.push_str(&format!(
            "  <rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>\n"
        ));
    }
    body.insert_str(0, &head);

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
    let rendering = if opts.fit.smooth() {
        ""
    } else {
        " shape-rendering=\"crispEdges\""
    };

    let (dw, dh) = opts
        .display
        .map_or((w * scale, h * scale), |(dw, dh)| (dw as i64, dh as i64));
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{dw}\" height=\"{dh}\" \
viewBox=\"0 0 {w} {h}\"{rendering}>\n{body}</svg>\n",
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
