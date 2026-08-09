//! Instantáneas del camino de foto, sobre una imagen sintética.
//!
//! Son a la segmentación por clustering lo que `golden.rs` a la de rejilla: la
//! entrada cabe en el fichero, corre en cualquier clon y va a CI. El corpus no
//! sirve aquí —no está versionado— y además una foto real es mal fixture para
//! esto: lo que hay que fijar son los comportamientos que se decidieron mirando
//! resultados, y cada uno quiere un motivo que lo aísle.
//!
//! El dibujo los lleva todos a propósito:
//!
//! - un **degradado** suave, que es lo que obliga a la paleta a repartir una
//!   rampa continua en escalones, y lo que ensancha `gradient_step`;
//! - dos **bloques planos** de tonos muy distintos, que deben salir de una pieza;
//! - una **línea de un píxel**, que es el detalle fino que el filtrado por
//!   grosor se lleva por delante —y el caso que hay que poder mirar para decidir
//!   si eso está bien;
//! - unos **puntos sueltos**, que es la mota clásica, la que sí quita el área.

#![cfg(feature = "photo")]

mod common;

use common::check;
use img2svg::{ClusterOptions, Config, Conversion, Detail};

/// Tamaño del dibujo, sin contar el margen de fondo.
const W: usize = 64;
const H: usize = 48;

/// Color de la línea fina y de los puntos sueltos. Es el más saturado posible y
/// no se parece a nada más del dibujo: si desaparece del SVG es porque se ha
/// fundido, no porque la paleta lo haya confundido con un vecino.
const MAGENTA: &str = "#ff00ff";

/// Los dos bloques planos, **ya cuantizados**: `color_precision` recorta a 5
/// bits por canal antes de agrupar, así que el color que sale no es exactamente
/// el que entró. Que estos dos aparezcan tal cual es lo que dice que un bloque
/// plano sobrevive entero de punta a punta.
const ROJO: &str = "#c52929";
const VERDE: &str = "#299c3a";

/// Degradado de azul oscuro a claro con dos bloques planos encima, una línea de
/// un píxel de alto y cuatro puntos sueltos, sobre un margen de fondo liso.
///
/// El degradado va **en vertical** y la línea **en horizontal** para que se
/// crucen: así la línea atraviesa varias bandas del degradado en vez de quedarse
/// dentro de una, que es donde el filtrado por grosor tendría menos que decidir.
///
/// Con `margin` a 0 el dibujo tapa el fondo entero y no queda nada que quitar,
/// que es como lo quieren todos los casos menos el del fondo.
fn paint(margin: usize) -> (u32, u32, Vec<u8>) {
    let (cw, ch) = (W + margin * 2, H + margin * 2);
    let mut buf = vec![255u8; cw * ch * 4];
    let mut put = |x: usize, y: usize, px: [u8; 4]| {
        let base = ((y + margin) * cw + x + margin) * 4;
        buf[base..base + 4].copy_from_slice(&px);
    };

    for y in 0..H {
        // De 40 a 220 en 48 filas: unos cuatro niveles por fila, bastante fino
        // como para que la tolerancia tenga que agrupar filas contiguas.
        let l = 40 + (y * 180 / H) as u8;
        for x in 0..W {
            put(x, y, [l / 3, l / 2, l, 255]);
        }
    }

    for y in 8..20 {
        for x in 6..26 {
            put(x, y, [200, 40, 40, 255]);
        }
    }
    for y in 26..42 {
        for x in 36..58 {
            put(x, y, [40, 160, 60, 255]);
        }
    }

    for x in 2..62 {
        put(x, 23, [255, 0, 255, 255]);
    }
    for (x, y) in [(4, 4), (60, 6), (31, 45), (50, 12)] {
        put(x, y, [255, 0, 255, 255]);
    }

    (cw as u32, ch as u32, buf)
}

/// Convierte por la vía del búfer crudo, que es la que usa la página.
fn convert(margin: usize, options: ClusterOptions) -> Conversion {
    let (w, h, buf) = paint(margin);
    img2svg::convert_rgba(w, h, &buf, &Config::cluster(options))
        .expect("la conversión no debe fallar")
}

fn regions(out: &Conversion) -> usize {
    match out.detail {
        Detail::Cluster { regions } => regions,
        _ => panic!("una conversión de foto debe traer detalle de clustering"),
    }
}

/// Con las opciones por defecto: la rampa sale a bandas, los bloques planos
/// enteros, y el detalle de un píxel —línea y puntos— no sobrevive.
#[test]
fn por_defecto() {
    let out = convert(0, ClusterOptions::default());

    assert_eq!(out.canvas, (W, H), "sin quitar el fondo no se recorta");
    assert!(
        out.svg.contains(ROJO) && out.svg.contains(VERDE),
        "los dos bloques planos deben salir enteros y con su color"
    );
    assert!(
        !out.svg.contains(MAGENTA),
        "con min_thickness = 1 no queda nada de un píxel de ancho, \
         ni la línea ni los puntos"
    );
    assert!(
        out.colors > 5,
        "la rampa tiene que dar varias bandas, no {} colores",
        out.colors
    );

    check("foto-por-defecto", &out, "dibujo, opciones por defecto");
}

/// Sin filtrar, la línea de un píxel y los puntos siguen ahí. Es el contraste
/// que hace que el caso anterior signifique algo: fija que lo que desaparece lo
/// hace por el filtro y no porque la paleta se lo haya comido.
#[test]
fn sin_filtrar_sobrevive_el_detalle_fino() {
    let base = convert(0, ClusterOptions::default());
    let out = convert(
        0,
        ClusterOptions {
            filter_speckle: 0,
            min_thickness: 0.0,
            ..ClusterOptions::default()
        },
    );

    assert!(
        out.svg.contains(MAGENTA),
        "sin filtro, la línea de un píxel debe llegar al documento"
    );
    assert!(
        regions(&out) > regions(&base),
        "y con ella más regiones: {} sin filtrar, {} con filtro",
        regions(&out),
        regions(&base)
    );

    check(
        "foto-sin-filtrar",
        &out,
        "dibujo, filter_speckle = 0, min_thickness = 0",
    );
}

/// `gradient_step` ensancha las bandas de la rampa. Deja menos colores; el
/// número de regiones **no** tiene por qué bajar, y por eso no se afirma nada de
/// él: fundir dos bandas vecinas puede partir en dos lo que las rodeaba.
#[test]
fn el_escalon_ensancha_las_bandas() {
    let base = convert(0, ClusterOptions::default());
    let out = convert(
        0,
        ClusterOptions {
            gradient_step: 0.15,
            ..ClusterOptions::default()
        },
    );

    assert!(
        out.colors < base.colors,
        "con gradient_step deben quedar menos colores que sin él ({} vs {})",
        out.colors,
        base.colors
    );
    assert!(
        out.svg.contains(ROJO) && out.svg.contains(VERDE),
        "y los bloques planos no se tocan: sólo se funde a lo largo de la luz"
    );

    check("foto-gradiente", &out, "dibujo, gradient_step = 0.15");
}

/// El fondo del camino de foto es lo que toca el borde de la imagen. Con el
/// dibujo sobre un margen liso, quitarlo devuelve el lienzo al tamaño del dibujo.
#[test]
fn fondo_retirado_y_recortado() {
    let out = convert(
        6,
        ClusterOptions {
            remove_background: true,
            ..ClusterOptions::default()
        },
    );

    let fondo = out.background.expect("debe encontrar el margen liso");
    assert_eq!(fondo.to_hex(), "#ffffff");
    assert_eq!(out.canvas, (W, H), "y recortar el margen hasta el dibujo");
    assert!(
        !out.svg.contains("#ffffff"),
        "el color retirado no debe seguir pintándose"
    );

    check(
        "foto-fondo-retirado",
        &out,
        "dibujo con margen de 6 px, remove_background",
    );
}

/// Sin retirarlo, el mismo dibujo conserva el margen entero: fija el contraste
/// con el caso anterior.
#[test]
fn fondo_conservado() {
    let out = convert(6, ClusterOptions::default());

    assert!(out.background.is_none());
    assert_eq!(out.canvas, (W + 12, H + 12), "el lienzo entero");
    assert!(out.svg.contains("#ffffff"), "y el margen sigue pintado");
}
