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
use img2svg::{ClusterOptions, Config, Conversion, Detail, Fit};

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
    convert_con(margin, options, Fit::Pixel)
}

/// Lo mismo eligiendo el ajuste, que es el otro eje y no depende de éste.
fn convert_con(margin: usize, options: ClusterOptions, fit: Fit) -> Conversion {
    let (w, h, buf) = paint(margin);
    let config = Config {
        fit,
        ..Config::cluster(options)
    };
    img2svg::convert_rgba(w, h, &buf, &config).expect("la conversión no debe fallar")
}

fn regions(out: &Conversion) -> usize {
    match out.detail {
        Detail::Cluster { regions, .. } => regions,
        _ => panic!("una conversión de foto debe traer detalle de clustering"),
    }
}

fn ramps(out: &Conversion) -> usize {
    match out.detail {
        Detail::Cluster { ramps, .. } => ramps,
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

/// El ajuste de polígono sobre la misma segmentación: el mismo dibujo, las
/// mismas regiones y bastante menos path.
///
/// Va aquí y no en las instantáneas de pixel art porque es donde se ve: la
/// rejilla deja contornos de píxeles cuadrados, que ya son lo que son, y el
/// degradado a bandas de esta imagen deja fronteras largas y tendidas, que es
/// justo lo que una escalera describe mal.
#[test]
fn el_poligono_dibuja_lo_mismo_con_menos_datos() {
    // Sobre la retícula, que es donde la comparación tiene sentido: con el
    // contorno subpíxel los dos ajustes dejan de dibujar la misma línea —uno la
    // escalera de la retícula y otro el borde de verdad—, y entonces comparar sus
    // tamaños no dice nada de la simplificación. Ese otro compromiso lo fija
    // `el_subpixel_cuesta_bytes_y_compra_sitio`.
    let sobre_reticula = ClusterOptions {
        subpixel: false,
        ..ClusterOptions::default()
    };
    let escalera = convert(0, sobre_reticula.clone());
    let out = convert_con(0, sobre_reticula, Fit::polygon());

    assert_eq!(
        out.paths, escalera.paths,
        "el ajuste no cambia en cuántas figuras se parte el dibujo"
    );
    assert_eq!(out.colors, escalera.colors, "ni la paleta");
    assert!(
        out.svg.len() < escalera.svg.len(),
        "y tiene que ocupar menos: {} bytes contra {}",
        out.svg.len(),
        escalera.svg.len()
    );
    assert!(
        !out.svg.contains("crispEdges"),
        "con oblicuas el suavizado tiene que estar puesto"
    );

    check("foto-poligono", &out, "dibujo, fit = polygon (0.75)");
}

/// El compromiso del contorno subpíxel, fijado para que no se olvide.
///
/// Cuesta bytes **siempre**, y no por descuido: sobre la retícula un tramo
/// horizontal se escribe `h` con un número porque sus dos extremos comparten la
/// `y`, y en cuanto los vértices se salen de la retícula ese mismo tramo pasa a
/// `l` con dos números y decimales. Lo que compra es sitio: los vértices caen
/// donde la imagen dice que está el borde, que en un dibujo pequeño es la
/// diferencia entre una lente redonda y un octógono.
///
/// El ajuste `pixel` no lo lee —es la escalera literal por definición—, y eso
/// también se comprueba aquí: es lo que hace que las instantáneas de rejilla
/// sigan valiendo.
///
/// Con los degradados **apagados**, y no por comodidad: en este dibujo las únicas
/// fronteras con mezcla de color son las de las bandas de la rampa —los bloques
/// planos tienen los dos lados puros y ahí el desplazamiento sale exactamente
/// cero—, y ésas son justo las que se lleva el degradado al fundirlas. Con
/// degradados los dos documentos salen idénticos byte a byte, que es cierto y no
/// es lo que este test quiere decir.
#[test]
fn el_subpixel_cuesta_bytes_y_compra_sitio() {
    let sin_degradados = ClusterOptions {
        ramps: false,
        ..ClusterOptions::default()
    };
    let con = convert_con(0, sin_degradados.clone(), Fit::polygon());
    let sin = convert_con(
        0,
        ClusterOptions {
            subpixel: false,
            ..sin_degradados.clone()
        },
        Fit::polygon(),
    );
    assert_eq!(con.paths, sin.paths, "no cambia en qué se parte el dibujo");
    assert_eq!(con.colors, sin.colors, "ni la paleta");
    assert!(
        con.svg.len() > sin.svg.len(),
        "sale más grande, que es lo que cuesta: {} contra {}",
        con.svg.len(),
        sin.svg.len()
    );

    // Y con el ajuste de escalera los dos son idénticos byte a byte, porque ése
    // no mira el desplazamiento.
    let escalera_con = convert(0, sin_degradados.clone());
    let escalera_sin = convert(
        0,
        ClusterOptions {
            subpixel: false,
            ..sin_degradados
        },
    );
    assert_eq!(
        escalera_con.svg, escalera_sin.svg,
        "`pixel` tiene que ignorar el subpíxel"
    );
}

/// Y el de curvas, que es el único que inventa puntos que no estaban en la
/// retícula: sus controles no los fija ningún otro test byte a byte.
///
/// Con **dibujo propio**, y no con el de arriba: ese es todo bloques y bandas
/// horizontales, así que el ajuste de curvas no encuentra nada que curvar y la
/// instantánea saldría sin una sola `c`, fijando exactamente nada. Dos discos
/// que se solapan dan lo que hace falta: contorno curvo, y una frontera curva
/// compartida por dos regiones.
///
/// La instantánea es aquí la red que importa. Las propiedades —la costura, el
/// techo de la tolerancia, que un rectángulo no se redondee— viven en
/// `tests/fit.rs` y seguirían pasando aunque una reparametrización cambiara de
/// resultado; esto dice si ha cambiado.
#[test]
fn las_curvas_se_quedan_donde_estan() {
    let (w, h) = (72usize, 56usize);
    let discos = [
        (28.0f64, 28.0f64, 20.0f64, [200u8, 60, 60, 255]),
        (46.0, 30.0, 18.0, [60, 110, 200, 255]),
    ];
    let mut buf = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let mut color = [245u8, 245, 240, 255];
            for &(cx, cy, r, c) in &discos {
                if ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt() <= r {
                    color = c;
                }
            }
            buf.extend_from_slice(&color);
        }
    }

    let config = Config {
        fit: Fit::spline(),
        ..Config::cluster(ClusterOptions::default())
    };
    let out = img2svg::convert_rgba(w as u32, h as u32, &buf, &config)
        .expect("la conversión no debe fallar");

    // Contando comandos dentro de los `d`, no letras sueltas del documento: la
    // primera versión de esto miraba `svg.contains('c')`, y la `c` de «colores»
    // de la cabecera la daba por buena.
    let curvas: usize = out
        .svg
        .split("d=\"")
        .skip(1)
        .map(|rest| {
            rest[..rest.find('"').expect("un atributo d sin cerrar")]
                .matches('c')
                .count()
        })
        .sum();
    assert!(curvas > 0, "sin una sola curva no se está fijando nada");
    assert!(
        !out.svg.contains("crispEdges"),
        "con curvas el suavizado tiene que estar puesto"
    );

    check(
        "foto-curvas",
        &out,
        &format!("dos discos, fit = spline (1.5), {curvas} curvas"),
    );
}

/// La rampa vertical sale como **un** degradado y no como una pila de bandas, y
/// los dos bloques planos no entran en él.
///
/// Es lo que este dibujo tenía preparado desde el principio: una rampa continua
/// que la paleta parte en escalones, y encima dos bloques de tonos muy distintos
/// que no son ninguna rampa. Que el degradado se lleve lo primero y no lo segundo
/// es el criterio entero.
#[test]
fn la_rampa_sale_de_una_pieza() {
    let plano = convert(
        0,
        ClusterOptions {
            ramps: false,
            ..ClusterOptions::default()
        },
    );
    let out = convert(0, ClusterOptions::default());

    assert_eq!(ramps(&out), 1, "una rampa, un degradado");
    assert_eq!(ramps(&plano), 0, "y sin la opción, ninguno");
    assert!(
        regions(&out) < regions(&plano),
        "las bandas dejan de ser regiones: {} contra {}",
        regions(&out),
        regions(&plano)
    );
    assert!(
        out.svg.contains("<linearGradient") && out.svg.contains("url(#r0)"),
        "el degradado tiene que estar definido y usado"
    );
    assert!(
        out.svg.contains(ROJO) && out.svg.contains(VERDE),
        "y los bloques planos siguen siendo planos, con su color"
    );
    assert_eq!(
        out.colors, plano.colors,
        "la paleta es la misma: esto no funde colores, funde figuras"
    );
}

/// Un borde duro no se ablanda. Dos franjas de colores lejanos apiladas cumplen
/// la parte geométrica de ser una rampa —el color va con la altura— y aun así no
/// pueden salir como degradado, porque el escalón entre ellas es enorme.
#[test]
fn una_bandera_no_es_un_degradado() {
    let (w, h) = (48usize, 48usize);
    let franjas = [
        [220u8, 30, 30, 255],
        [240, 240, 240, 255],
        [30, 60, 200, 255],
        [20, 20, 20, 255],
    ];
    let mut buf = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for _ in 0..w {
            buf.extend_from_slice(&franjas[y * franjas.len() / h]);
        }
    }
    let out = img2svg::convert_rgba(
        w as u32,
        h as u32,
        &buf,
        &Config::cluster(ClusterOptions::default()),
    )
    .expect("la conversión no debe fallar");

    assert_eq!(ramps(&out), 0, "cuatro franjas planas no son una rampa");
    assert!(!out.svg.contains("<linearGradient"));
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
