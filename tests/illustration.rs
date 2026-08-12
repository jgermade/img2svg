//! Instantáneas del camino de ilustración, sobre una imagen sintética.
//!
//! Son a la segmentación por clustering lo que `golden.rs` a la de rejilla: la
//! entrada cabe en el fichero, corre en cualquier clon y va a CI. El corpus no
//! sirve aquí —no está versionado— y además una imagen real es mal fixture para
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

#![cfg(feature = "illustration")]

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

/// Las opciones de partida de estas instantáneas: las de fábrica pero **sobre la
/// retícula del dibujo**.
///
/// Que `simplify` vaya apagado no es comodidad de fixture, es lo que hace que
/// estos tests digan algo: el dibujo lleva una línea de un píxel y cuatro puntos
/// sueltos colocados a mano para aislar cada comportamiento, y reescalarlo antes
/// de segmentar mediría el reescalado y no la etapa que toca. La escala de trabajo
/// tiene su propio fichero, `tests/resample.rs`, y su propia instantánea aquí.
fn en_la_reticula() -> ClusterOptions {
    ClusterOptions {
        simplify: Some(0.0),
        ..ClusterOptions::default()
    }
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
        _ => panic!("una conversión de ilustración debe traer detalle de clustering"),
    }
}

/// Cuántos `<path>` se pintan de un color. Es lo que distingue «la línea fina
/// sobrevivió» de «sobrevivieron la línea y los puntos», que comparten color.
fn paths_con(out: &Conversion, hex: &str) -> usize {
    let fill = format!("fill=\"{hex}\"");
    match out.svg.split(&fill).count() - 1 {
        // Un `<g fill>` con varios paths dentro: el color sale una vez y hay que
        // contar los paths del grupo.
        1 => {
            let desde = out.svg.find(&fill).unwrap();
            let hasta = out.svg[desde..].find("</g>").map(|n| desde + n);
            let bloque = &out.svg[desde..hasta.unwrap_or(out.svg.len())];
            bloque.matches("<path").count().max(1)
        }
        n => n,
    }
}

fn ramps(out: &Conversion) -> usize {
    match out.detail {
        Detail::Cluster { ramps, .. } => ramps,
        _ => panic!("una conversión de ilustración debe traer detalle de clustering"),
    }
}

/// Con las opciones por defecto: la rampa sale a bandas, los bloques planos
/// enteros, **la línea fina sobrevive y las motas no**.
///
/// La rampa sale como un degradado **más una banda plana**, y eso es la costura
/// entre dos ajustes que tiran en sentidos opuestos: `gradient_step` viene puesto
/// por la tinta partida y ensancha las bandas, lo que agranda el salto entre
/// ellas, y un salto grande es justo lo que [`img2svg::ramp`] rechaza. En una
/// rampa real —un cielo con grano— no llega a pasar; en cinco bandas sobre 48
/// píxeles, sí. Queda escrito aquí porque es el precio, y se ve en la instantánea.
///
/// Esa última pareja es el criterio de [`img2svg::speckle`] puesto a prueba con
/// las dos cosas que miden lo mismo. La línea es de un píxel de ancho, así que el
/// grosor la propone; su magenta no se parece a ninguna mezcla de las bandas que
/// tiene a los dos lados, así que se queda. Los cuatro puntos sueltos miden un
/// píxel de área y se van por área, que no pregunta nada del color.
#[test]
fn por_defecto() {
    let out = convert(0, en_la_reticula());

    assert_eq!(out.canvas, (W, H), "sin quitar el fondo no se recorta");
    assert!(
        out.svg.contains(ROJO) && out.svg.contains(VERDE),
        "los dos bloques planos deben salir enteros y con su color"
    );
    assert_eq!(
        paths_con(&out, MAGENTA),
        1,
        "la línea fina tiene que llegar entera y sola: es tinta, no mezcla, \
         y los cuatro puntos se van por área"
    );
    assert!(
        out.colors > 5,
        "la rampa tiene que dar varias bandas, no {} colores",
        out.colors
    );

    check(
        "ilustracion-por-defecto",
        &out,
        "dibujo, opciones por defecto",
    );
}

/// Sin filtrar, la línea de un píxel y los puntos siguen ahí. Es el contraste
/// que hace que el caso anterior signifique algo: fija que lo que desaparece lo
/// hace por el filtro y no porque la paleta se lo haya comido.
#[test]
fn sin_filtrar_sobrevive_el_detalle_fino() {
    let base = convert(0, en_la_reticula());
    let out = convert(
        0,
        ClusterOptions {
            filter_speckle: 0,
            min_thickness: 0.0,
            ..en_la_reticula()
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
        "ilustracion-sin-filtrar",
        &out,
        "dibujo, filter_speckle = 0, min_thickness = 0",
    );
}

/// `gradient_step` ensancha las bandas de la rampa. Deja menos colores; el
/// número de regiones **no** tiene por qué bajar, y por eso no se afirma nada de
/// él: fundir dos bandas vecinas puede partir en dos lo que las rodeaba.
#[test]
fn el_escalon_ensancha_las_bandas() {
    let base = convert(0, en_la_reticula());
    let out = convert(
        0,
        ClusterOptions {
            gradient_step: 0.15,
            ..en_la_reticula()
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

    check(
        "ilustracion-gradiente",
        &out,
        "dibujo, gradient_step = 0.15",
    );
}

/// El fondo del camino de ilustración es lo que toca el borde de la imagen. Con el
/// dibujo sobre un margen liso, quitarlo devuelve el lienzo al tamaño del dibujo.
#[test]
fn fondo_retirado_y_recortado() {
    let out = convert(
        6,
        ClusterOptions {
            remove_background: true,
            ..en_la_reticula()
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
        "ilustracion-fondo-retirado",
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
        ..en_la_reticula()
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

    check("ilustracion-poligono", &out, "dibujo, fit = polygon (0.75)");
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
        ..en_la_reticula()
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
        ..Config::cluster(en_la_reticula())
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
        "ilustracion-curvas",
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
            ..en_la_reticula()
        },
    );
    let out = convert(0, en_la_reticula());

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

/// Un foco sobre una pared sale como **un** degradado radial, y no como uno
/// lineal.
///
/// Es el otro modelo, y hace falta porque el error de usar el que no toca no es
/// pequeño: un sombreado que cae con la distancia a un punto, explicado con la
/// proyección sobre un eje, sale embadurnado a lo largo de una dirección que no
/// existe en el dibujo. Aquí se comprueba que el ajuste elige, y que elige bien.
///
/// Se prueba con el foco **en el centro del lienzo y descentrado**, porque son dos
/// casos distintos del ajuste. Centrado, las bandas son anillos completos y
/// concéntricos: todos tienen el mismo centroide, así que no hay ninguna dirección
/// en la que el color cambie más y el eje ni existe —el grupo sólo puede crecer si
/// el modelo radial entra en el crecimiento, y no si se elige al final—. Descentrado
/// hay eje, y entonces lo que se comprueba es que el radial le gana.
#[test]
fn un_foco_es_un_degradado_radial() {
    for luz in [(40.0f64, 40.0f64), (25.0, 25.0)] {
        let (w, h) = (80usize, 80usize);
        // Hasta la esquina más lejana, para que la caída llegue al final del lienzo.
        let alcance = (w as f64 - luz.0).hypot(h as f64 - luz.1);
        let mut buf = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let t = (x as f64 - luz.0).hypot(y as f64 - luz.1) / alcance;
                let l = 235.0 - 165.0 * t;
                buf.extend_from_slice(&[l as u8, (l * 0.80) as u8, (l * 0.58) as u8, 255]);
            }
        }
        let out =
            img2svg::convert_rgba(w as u32, h as u32, &buf, &Config::cluster(en_la_reticula()))
                .expect("la conversión no debe fallar");

        assert!(out.colors > 3, "la caída tiene que dar varias bandas");
        assert_eq!(
            ramps(&out),
            1,
            "una caída, un degradado, con la luz en {luz:?}"
        );
        assert!(
            out.svg.contains("<radialGradient"),
            "y con la geometría que le toca, no con la otra:\n{}",
            out.svg.lines().take(4).collect::<Vec<_>>().join("\n")
        );
        assert!(
            !out.svg.contains("<linearGradient"),
            "el eje no explica esto y no tiene que aparecer"
        );

        // Y el centro cae donde está el foco. Sale de ajustar un círculo a la
        // costura, así que no es exacto —la costura es un arco de unos cuantos
        // píxeles de ancho—, pero sí tiene que ser ese sitio y no otro.
        let cx: f64 = atributo(&out.svg, "cx=\"");
        let cy: f64 = atributo(&out.svg, "cy=\"");
        assert!(
            (cx - luz.0).hypot(cy - luz.1) < 8.0,
            "el centro del degradado ({cx}, {cy}) tiene que estar en el foco {luz:?}"
        );
    }
}

/// El primer valor de un atributo del documento, para mirar dónde acabó un
/// degradado.
fn atributo(svg: &str, name: &str) -> f64 {
    let rest = &svg[svg.find(name).expect("el atributo tiene que estar") + name.len()..];
    rest[..rest.find('"').expect("un atributo sin cerrar")]
        .parse()
        .expect("un número")
}

/// Con paleta impuesta de dos colores: así el grupo es **una pareja**, que es el
/// único caso que la blandura de la costura decide por su cuenta.
///
/// El par se elige con un salto de color moderado —0,12 en Oklab— para que lo que
/// esté a prueba sea la puerta y no el techo: con dos paradas el degradado se
/// equivoca la mitad de lo que abarca por pura aritmética, así que una pareja de
/// tonos muy lejanos se caería por el techo de la tolerancia antes de que nadie
/// preguntase por la costura.
fn a_dos_colores() -> ClusterOptions {
    ClusterOptions {
        palette: vec![
            img2svg::color::Rgba::new(222, 196, 168, 255),
            img2svg::color::Rgba::new(186, 158, 128, 255),
        ],
        filter_speckle: 0,
        min_thickness: 0.0,
        relax: 0.0,
        smoothing: 0,
        gradient_step: 0.0,
        ..en_la_reticula()
    }
}

/// Dos bandas de esos dos colores, con `difusa` filas de mezcla entre ellas.
fn dos_bandas(difusa: usize) -> (u32, u32, Vec<u8>) {
    let (w, h) = (40usize, 40 + difusa);
    let opciones = a_dos_colores();
    let (claro, oscuro) = (opciones.palette[0], opciones.palette[1]);
    let mut buf = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let c = match y {
            y if y < 20 => claro,
            y if y < 20 + difusa => {
                let t = (y - 19) as f64 / (difusa + 1) as f64;
                let mezcla =
                    |a: u8, b: u8| (f64::from(a) + t * (f64::from(b) - f64::from(a))).round() as u8;
                img2svg::color::Rgba::new(
                    mezcla(claro.r, oscuro.r),
                    mezcla(claro.g, oscuro.g),
                    mezcla(claro.b, oscuro.b),
                    255,
                )
            }
            _ => oscuro,
        };
        for _ in 0..w {
            buf.extend_from_slice(&[c.r, c.g, c.b, c.a]);
        }
    }
    (w as u32, h as u32, buf)
}

/// Una costura difuminada junta dos bandas en un degradado, aunque sean dos
/// colores y no tres.
///
/// Es la puerta que abre la medida de blandura, y la única forma de que una pareja
/// entre. El motivo es el dibujo: la barriga de un personaje sombreado a aerógrafo
/// son dos tonos con una transición ancha entre ellos, y por recuento de colores no
/// llega a rampa nunca —serían dos paradas, que es hacer la media—, así que salía
/// con una media luna de borde duro en medio del volumen.
#[test]
fn una_costura_blanda_junta_dos_bandas() {
    let (w, h, buf) = dos_bandas(10);
    let out = img2svg::convert_rgba(w, h, &buf, &Config::cluster(a_dos_colores()))
        .expect("la conversión no debe fallar");

    assert_eq!(out.colors, 2, "la paleta impuesta, tal cual");
    assert_eq!(
        ramps(&out),
        1,
        "dos bandas con la costura difuminada son una transición"
    );
    assert!(
        out.svg.contains("<linearGradient"),
        "y una costura recta se explica con un eje"
    );
}

/// Y con la costura seca, las mismas dos bandas se quedan como están.
///
/// Es lo que hace que la puerta de arriba no sea una puerta abierta: dos regiones
/// vecinas cualesquiera siempre se pueden partir con un degradado tendido, y lo que
/// distingue una transición de un canto no es el ajuste —que en los dos casos vale
/// igual— sino lo que el original pintó entre los dos colores.
#[test]
fn una_costura_dura_no_junta_dos_bandas() {
    let (w, h, buf) = dos_bandas(0);
    let out = img2svg::convert_rgba(w, h, &buf, &Config::cluster(a_dos_colores()))
        .expect("la conversión no debe fallar");

    assert_eq!(out.colors, 2, "el mismo par de colores");
    assert_eq!(ramps(&out), 0, "un canto no es un degradado");
    assert!(!out.svg.contains("Gradient"));
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
    let out = img2svg::convert_rgba(w as u32, h as u32, &buf, &Config::cluster(en_la_reticula()))
        .expect("la conversión no debe fallar");

    assert_eq!(ramps(&out), 0, "cuatro franjas planas no son una rampa");
    assert!(!out.svg.contains("<linearGradient"));
}

/// Sin retirarlo, el mismo dibujo conserva el margen entero: fija el contraste
/// con el caso anterior.
#[test]
fn fondo_conservado() {
    let out = convert(6, en_la_reticula());

    assert!(out.background.is_none());
    assert_eq!(out.canvas, (W + 12, H + 12), "el lienzo entero");
    assert!(out.svg.contains("#ffffff"), "y el margen sigue pintado");
}
