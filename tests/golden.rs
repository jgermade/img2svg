//! Instantáneas sobre dibujos sintéticos.
//!
//! Complementan a las de `corpus.rs`: aquí la entrada cabe en el fichero, se
//! versiona sola y corre en cualquier clon, así que es la red que sí puede ir a
//! CI. A cambio no tiene ruido, que es lo que aportan las imágenes reales.
//!
//! El sprite está dibujado para tocar todo lo que puede romperse sin que se note
//! a simple vista, y a propósito es asimétrico: un dibujo con un motivo que se
//! repita hace que `grid::detect` vea una rejilla donde no la hay.
//!
//! Cada caso, además, afirma que su función se ha activado de verdad. Una
//! instantánea que no llega a ejecutar el código que dice cubrir pasa en verde
//! para siempre.

mod common;

use common::check;
use px2svg::{Config, Conversion, Grouping};

/// Paleta del arte ASCII. Sólo ASCII: las filas se miden en bytes.
fn paint(ch: char) -> Option<[u8; 4]> {
    match ch {
        '#' => Some([20, 30, 40, 255]),    // tinta
        'o' => Some([220, 60, 50, 255]),   // segundo color
        '~' => Some([220, 60, 50, 160]),   // semitransparente -> fill-opacity
        '+' => Some([250, 250, 250, 255]), // fondo liso
        _ => None,                         // '.' transparente
    }
}

/// Rasteriza el arte a `scale`x y devuelve el búfer RGBA que espera la web.
fn raster(rows: &[&str], scale: usize) -> (u32, u32, Vec<u8>) {
    let (cols, lines) = (rows[0].len(), rows.len());
    let (w, h) = (cols * scale, lines * scale);
    let mut buf = vec![0u8; w * h * 4];
    for (y, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), cols, "todas las filas deben medir lo mismo");
        for (x, ch) in row.chars().enumerate() {
            let Some(px) = paint(ch) else { continue };
            for dy in 0..scale {
                let base = ((y * scale + dy) * w + x * scale) * 4;
                for dx in 0..scale {
                    buf[base + dx * 4..base + dx * 4 + 4].copy_from_slice(&px);
                }
            }
        }
    }
    (w as u32, h as u32, buf)
}

/// Convierte por la vía del búfer crudo, que es la que usa la página.
fn convert(rows: &[&str], scale: usize, config: &Config) -> Conversion {
    let (w, h, buf) = raster(rows, scale);
    px2svg::convert_rgba(w, h, &buf, config).expect("la conversión no debe fallar")
}

/// Escala forzada a 1: la rejilla se toma como dada en vez de detectarla.
fn sin_detectar() -> Config {
    Config {
        scale: Some(1.0),
        ..Config::default()
    }
}

/// Sprite asimétrico con todo lo que puede romperse en silencio: huecos
/// interiores (`evenodd`), píxeles que sólo se tocan por la esquina
/// (conectividad 8 al agrupar, 4 al trazar), tramos rectos largos —que delatan
/// un `simplify` perdido— y un tono semitransparente (`fill-opacity`).
const SPRITE: &[&str] = &[
    "...###########..",
    "..#############.",
    "..##o#######o##.",
    "..###o#####o###.",
    "..#####ooo#####.",
    "..#############.",
    "..#.###########.",
    "...###~~~~~###..",
    "....##~~~~~##...",
    "....ooo###ooo...",
    "...oo.......oo..",
    "..oo.........oo.",
    ".oo...........o.",
    "..#............#",
];

/// El mismo sprite sobre fondo liso, con margen de dos píxeles y un bolsillo del
/// mismo color encerrado en la masa de tinta: el de fuera se va, el de dentro se
/// queda.
const SOBRE_FONDO: &[&str] = &[
    "++++++++++++++++++++",
    "++++++++++++++++++++",
    "++...###########..++",
    "++..#############.++",
    "++..##o#######o##.++",
    "++..###o#####o###.++",
    "++..#####+++#####.++",
    "++..#############.++",
    "++..#.###########.++",
    "++...###~~~~~###..++",
    "++....##~~~~~##...++",
    "++....ooo###ooo...++",
    "++...oo.......oo..++",
    "++..oo.........oo.++",
    "++.oo...........o.++",
    "++..#............#++",
    "++++++++++++++++++++",
    "++++++++++++++++++++",
];

#[test]
fn escala_forzada_a_uno() {
    let out = convert(SPRITE, 1, &sin_detectar());
    assert_eq!(out.grid, (16, 14), "un píxel lógico por píxel real");
    assert_eq!(out.cell, (1.0, 1.0));
    assert!(out.colors >= 3, "tinta, color plano y semitransparente");
    assert!(
        out.svg.contains("fill-opacity"),
        "el tono semitransparente debe salir con fill-opacity"
    );
    assert!(
        out.svg.contains("evenodd"),
        "hay bloques con agujero, deben ir con fill-rule"
    );
    check("escala-forzada-a-uno", &out, "SPRITE a 1x, scale = 1.0");
}

#[test]
fn rejilla_detectada() {
    let out = convert(SPRITE, 7, &Config::default());
    assert_eq!(out.cell.0, 7.0, "debe enganchar la rejilla en X");
    assert_eq!(out.cell.1, 7.0, "y en Y");
    assert_eq!(out.grid, (16, 14), "reducido a los píxeles lógicos");
    check(
        "rejilla-detectada",
        &out,
        "SPRITE a 7x, opciones por defecto",
    );
}

/// El mismo dibujo a cualquier escala tiene que dar el mismo SVG: es lo que
/// promete la detección de rejilla, y es lo primero que se rompe al tocar
/// `grid.rs`.
///
/// Se fija `pixel_size` porque si no lo único que cambia, y con razón, es el
/// `width`/`height` de render: por defecto sigue a la celda detectada.
#[test]
fn la_escala_no_cambia_el_resultado() {
    let base = Config {
        pixel_size: Some(1),
        ..Config::default()
    };
    let forzado = convert(
        SPRITE,
        1,
        &Config {
            scale: Some(1.0),
            ..base.clone()
        },
    );
    let cinco = convert(SPRITE, 5, &base);
    let siete = convert(SPRITE, 7, &base);
    assert_eq!(
        forzado.svg, siete.svg,
        "1x forzado y 7x detectado describen el mismo dibujo lógico"
    );
    assert_eq!(cinco.svg, siete.svg, "5x y 7x también");
}

#[test]
fn un_path_por_color() {
    let config = Config {
        grouping: Grouping::Color,
        ..sin_detectar()
    };
    let out = convert(SPRITE, 1, &config);
    assert_eq!(out.paths, out.colors, "un path por color, sin <g>");
    assert!(out.subpaths > out.paths, "y varios subtrazados dentro");
    check("un-path-por-color", &out, "SPRITE a 1x, grouping = Color");
}

#[test]
fn fondo_retirado_y_recortado() {
    let config = Config {
        remove_background: true,
        ..sin_detectar()
    };
    let out = convert(SOBRE_FONDO, 1, &config);
    let fondo = out.background.expect("debe encontrar el fondo liso");
    assert_eq!(fondo.to_hex(), "#fafafa");
    // 15 y no 16: el recorte va a la caja de lo dibujado, y la primera columna
    // del sprite está vacía en todas las filas.
    assert_eq!(out.grid, (15, 14), "y recortar el margen hasta el dibujo");
    assert!(
        out.svg.contains("#fafafa"),
        "el bolsillo encerrado del mismo color se conserva"
    );
    check(
        "fondo-retirado",
        &out,
        "SOBRE_FONDO a 1x, scale = 1.0, remove_background",
    );
}

/// Sin retirar el fondo, el mismo dibujo conserva el marco entero: fija el
/// contraste con el caso anterior.
#[test]
fn fondo_conservado() {
    let out = convert(SOBRE_FONDO, 1, &sin_detectar());
    assert!(out.background.is_none());
    assert_eq!(out.grid, (20, 18), "el lienzo entero");
    check("fondo-conservado", &out, "SOBRE_FONDO a 1x, scale = 1.0");
}

/// Los defaults del camino de pixel art no se mueven cuando `Config` se parta en
/// `Segmentation` / `Fit`: las necesidades del modo foto (tolerancia mucho más
/// alta) no deben arrastrarlos.
#[test]
fn los_defaults_no_derivan() {
    let d = Config::default();
    assert_eq!(d.scale, None);
    assert_eq!(d.offset, None);
    assert_eq!(d.tolerance, 12.0);
    assert_eq!(d.alpha_threshold, 128);
    assert_eq!(d.pixel_size, None);
    assert_eq!(d.background, None);
    assert_eq!(d.grouping, Grouping::Region);
    assert!(d.remove_checkerboard);
    assert!(!d.remove_background);
}
