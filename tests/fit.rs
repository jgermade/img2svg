//! Ajuste: del contorno de una región a los datos de un `<path>`.
//!
//! Se prueba **sobre el documento emitido** y no sobre las estructuras
//! internas: lo que tiene que cumplirse es una propiedad de la geometría que se
//! dibuja, y leerla del `d` de cada path comprueba de paso que lo escrito se
//! puede volver a leer.

// Sólo la comprobación de costuras necesita las tablas, y sólo existe con la
// segmentación que comparte fronteras entre regiones.
#[cfg(feature = "photo")]
use std::collections::{HashMap, HashSet};

use img2svg::{Config, Conversion, Fit, GridOptions};

type Point = (i32, i32);

/// Colores bien separados: la tolerancia va a 0, así que cualquier par distinto
/// vale, pero mirando el SVG se distinguen. El punto es transparente, para poder
/// dibujar una figura suelta y que no salga también el contorno de su fondo.
const NEGRO: [u8; 4] = [0, 0, 0, 255];
const BLANCO: [u8; 4] = [255, 255, 255, 255];
const ROJO: [u8; 4] = [220, 40, 40, 255];
const NADA: [u8; 4] = [0, 0, 0, 0];

fn pixels(rows: &[&str]) -> (u32, u32, Vec<u8>) {
    let (w, h) = (rows[0].len() as u32, rows.len() as u32);
    let mut buf = Vec::with_capacity((w * h) as usize * 4);
    for row in rows {
        assert_eq!(row.len(), w as usize, "las filas no miden lo mismo");
        for c in row.chars() {
            buf.extend_from_slice(match c {
                '#' => &NEGRO,
                'o' => &BLANCO,
                'r' => &ROJO,
                '.' => &NADA,
                other => panic!("carácter {other:?} sin color"),
            });
        }
    }
    (w, h, buf)
}

/// Convierte un dibujo por el camino de rejilla, un píxel por celda.
///
/// El ajuste es un eje aparte de la segmentación, así que se prueba en la más
/// sencilla de las dos: contornos que se pueden contar a mano.
fn convert(rows: &[&str], fit: Fit) -> Conversion {
    let (w, h, buf) = pixels(rows);
    let config = Config {
        fit,
        ..Config::grid(GridOptions {
            // Sin detección ni fusión de colores: aquí se mira el contorno, y
            // que la rejilla o la paleta opinasen lo enturbiaría.
            scale: Some(1.0),
            tolerance: 0.0,
            remove_checkerboard: false,
            ..GridOptions::default()
        })
    };
    img2svg::convert_rgba(w, h, &buf, &config).expect("la conversión no debe fallar")
}

/* ------------------------------------------------------------- el documento --- */

/// Los atributos `d` del documento, en orden.
fn path_data(svg: &str) -> Vec<&str> {
    svg.split("d=\"")
        .skip(1)
        .map(|rest| &rest[..rest.find('"').expect("un atributo d sin cerrar")])
        .collect()
}

/// Los subtrazados del documento, como listas de puntos absolutos.
fn subpaths(svg: &str) -> Vec<Vec<Point>> {
    path_data(svg).iter().flat_map(|d| parse(d)).collect()
}

/// Lee un `d` de los que emite el ajuste: `M`, `h`, `v`, `l` y `z`, todos
/// relativos menos el primero. Cualquier otro comando hace fallar el test, que
/// es lo que se quiere si un día se emite algo que no se pretendía.
fn parse(d: &str) -> Vec<Vec<Point>> {
    let chars: Vec<char> = d.chars().collect();
    let mut paths = Vec::new();
    let mut points: Vec<Point> = Vec::new();
    let mut at = (0, 0);
    let mut i = 0;

    while i < chars.len() {
        let command = chars[i];
        i += 1;
        match command {
            'M' => {
                at = (number(&chars, &mut i), number(&chars, &mut i));
                points.push(at);
            }
            'h' => {
                at.0 += number(&chars, &mut i);
                points.push(at);
            }
            'v' => {
                at.1 += number(&chars, &mut i);
                points.push(at);
            }
            'l' => {
                at = (at.0 + number(&chars, &mut i), at.1 + number(&chars, &mut i));
                points.push(at);
            }
            // El cierre es implícito: el último punto no repite el primero.
            'z' => paths.push(std::mem::take(&mut points)),
            other => panic!("comando {other:?} inesperado en {d:?}"),
        }
    }
    assert!(points.is_empty(), "un subtrazado sin cerrar en {d:?}");
    paths
}

fn number(chars: &[char], i: &mut usize) -> i32 {
    let start = *i;
    if chars[*i] == '-' {
        *i += 1;
    }
    while *i < chars.len() && chars[*i].is_ascii_digit() {
        *i += 1;
    }
    let n = chars[start..*i]
        .iter()
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("número ilegible en la posición {start}"));
    // El separador es un espacio, o el signo del número siguiente.
    while *i < chars.len() && chars[*i] == ' ' {
        *i += 1;
    }
    n
}

/* ---------------------------------------------------------------- costuras --- */

/// La comprobación que justifica toda la estructura: **cada segmento interior
/// lo dibujan exactamente dos caras**, y con los mismos extremos.
///
/// Si las dos caras de una frontera compartida se ajustaran por separado, una
/// podría quitar un vértice que la otra conserva; entre las dos líneas quedaría
/// una franja de fondo a la vista, y aquí el segmento largo de una cara y los
/// dos cortos de la otra aparecerían una sola vez cada uno.
///
/// Antes de contar hay que partir cada segmento por los vértices que caigan
/// dentro de él: una junta colineal en un nodo se puede fundir en una cara y no
/// en la otra sin mover la línea ni un pelo, y lo que tiene que coincidir es la
/// línea, no en cuántos comandos se escribió.
#[cfg(feature = "photo")]
fn comprueba_costuras(out: &Conversion) {
    let (w, h) = (out.canvas.0 as i32, out.canvas.1 as i32);
    let paths = subpaths(&out.svg);
    let vertices: HashSet<Point> = paths.iter().flatten().copied().collect();

    let mut counts: HashMap<(Point, Point), usize> = HashMap::new();
    for path in &paths {
        for i in 0..path.len() {
            let ends = (path[i], path[(i + 1) % path.len()]);
            for (a, b) in split(ends, &vertices) {
                *counts
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_default() += 1;
            }
        }
    }

    let mut interiores = 0;
    for (&(a, b), &n) in &counts {
        // El borde del lienzo lo dibuja una sola cara: al otro lado no hay
        // región ninguna.
        let borde = (a.0 == 0 && b.0 == 0)
            || (a.1 == 0 && b.1 == 0)
            || (a.0 == w && b.0 == w)
            || (a.1 == h && b.1 == h);
        if borde {
            continue;
        }
        interiores += 1;
        assert_eq!(n, 2, "el segmento {a:?}-{b:?} lo dibujan {n} caras, no dos");
    }
    assert!(
        interiores > 0,
        "el dibujo no tiene ninguna frontera interior"
    );
}

/// Parte un segmento por los vértices que caen dentro de él.
#[cfg(feature = "photo")]
fn split((a, b): (Point, Point), vertices: &HashSet<Point>) -> Vec<(Point, Point)> {
    let d = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
    let largo = d.0 * d.0 + d.1 * d.1;
    let mut dentro: Vec<(i64, Point)> = vertices
        .iter()
        .filter_map(|&v| {
            let p = ((v.0 - a.0) as i64, (v.1 - a.1) as i64);
            let alineado = d.0 * p.1 - d.1 * p.0 == 0;
            let avance = d.0 * p.0 + d.1 * p.1;
            (alineado && avance > 0 && avance < largo).then_some((avance, v))
        })
        .collect();
    dentro.sort();

    let mut out = Vec::with_capacity(dentro.len() + 1);
    let mut from = a;
    for (_, v) in dentro {
        out.push((from, v));
        from = v;
    }
    out.push((from, b));
    out
}

/* -------------------------------------------------------------------- casos --- */

/// Una diagonal a 45° es una recta, y el ajuste de polígono la escribe como
/// tal. Es la ganancia que justifica el ajustador entero.
#[test]
fn una_diagonal_sale_recta() {
    let escalera = &["#....", "##...", "###..", "####.", "#####"];

    let escalones = subpaths(&convert(escalera, Fit::Pixel).svg);
    let recta = subpaths(&convert(escalera, Fit::polygon()).svg);

    assert_eq!(escalones.len(), 1);
    assert_eq!(recta.len(), 1, "el ajuste no debe partir el anillo");
    // El triángulo tiene doce vértices en escalera —dos por escalón— y cuatro
    // cuando la hipotenusa es un solo tramo: los tres del triángulo y el borde
    // de arriba, que mide un píxel porque la fila de arriba tiene un píxel.
    assert_eq!(escalones[0].len(), 12);
    assert_eq!(recta[0].len(), 4);
    // Y los cuatro son vértices que ya estaban: RDP elige, no inventa.
    let mut triangulo = recta[0].clone();
    triangulo.sort();
    assert_eq!(triangulo, vec![(0, 0), (0, 5), (1, 0), (5, 5)]);
}

/// Con tolerancia 0 el polígono dibuja exactamente la escalera: RDP sólo quita
/// lo que se aparta de la cuerda, y de una escalera no se aparta nada.
///
/// Fija el otro extremo del rango, que es lo que dice que la tolerancia es lo
/// único que decide cuánto se simplifica.
#[test]
fn sin_tolerancia_el_poligono_es_la_escalera() {
    let dibujo = &["#..r.", "##rr.", "###..", "#..##", "....#"];
    let pixel = convert(dibujo, Fit::Pixel);
    let poligono = convert(dibujo, Fit::Polygon { tolerance: 0.0 });

    assert_eq!(path_data(&poligono.svg), path_data(&pixel.svg));
}

/// Lo que la tolerancia promete, y es lo único que promete: **ningún vértice
/// del contorno queda a más de esa distancia de la línea que se dibuja**.
///
/// Conviene ser preciso aquí, porque lo evidente es falso. RDP no conserva un
/// detalle por ser más alto que la tolerancia: mide contra la cuerda que tenga
/// en ese momento, no contra los vecinos del vértice, y una cuerda que venga de
/// lejos se traga un píxel que sobresale aunque suelto se apartara 1.0. Lo que
/// no puede es apartarse de la figura más de lo pedido, y eso sí se comprueba.
#[test]
fn la_tolerancia_acota_lo_que_se_aparta() {
    // Diagonales, escalones tendidos, un píxel suelto que sobresale y un
    // entrante: cada uno se aparta de su cuerda una cantidad distinta.
    let dibujo = &[
        "..#####..",
        ".#######.",
        "#########",
        "####.####",
        "#########",
        ".#######.",
        "..##.##..",
    ];
    let contorno: Vec<Point> = subpaths(&convert(dibujo, Fit::Pixel).svg)
        .into_iter()
        .flatten()
        .collect();

    for tolerance in [0.0, 0.75, 1.5, 3.0] {
        let ajustado = subpaths(&convert(dibujo, Fit::Polygon { tolerance }).svg);
        for &p in &contorno {
            let d = distancia(p, &ajustado);
            assert!(
                d <= tolerance + 1e-9,
                "con tolerancia {tolerance} el vértice {p:?} se queda a {d:.3} de lo dibujado"
            );
        }
    }
}

/// Distancia de un punto al subtrazado más cercano de los dibujados.
fn distancia(p: Point, paths: &[Vec<Point>]) -> f64 {
    paths
        .iter()
        .flat_map(|path| (0..path.len()).map(|i| (path[i], path[(i + 1) % path.len()])))
        .map(|(a, b)| al_segmento(p, a, b))
        .fold(f64::INFINITY, f64::min)
}

fn al_segmento(p: Point, a: Point, b: Point) -> f64 {
    let (px, py) = (p.0 as f64, p.1 as f64);
    let (ax, ay) = (a.0 as f64, a.1 as f64);
    let (dx, dy) = ((b.0 - a.0) as f64, (b.1 - a.1) as f64);
    let len2 = dx * dx + dy * dy;
    // Proyección recortada al segmento: fuera de él, el punto más cercano es un
    // extremo, y medir contra la recta infinita daría de menos.
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    ((px - ax - t * dx).powi(2) + (py - ay - t * dy).powi(2)).sqrt()
}

/// `crispEdges` apaga el suavizado, que es lo correcto para una escalera sobre
/// coordenadas enteras y lo contrario de lo que quiere una oblicua: dejaría
/// escalonada justo la diagonal que el ajuste acaba de enderezar.
#[test]
fn el_suavizado_depende_del_ajuste() {
    let dibujo = &["#..", "##.", "###"];
    assert!(convert(dibujo, Fit::Pixel).svg.contains("crispEdges"));
    assert!(!convert(dibujo, Fit::polygon()).svg.contains("crispEdges"));
}

/// Las dos caras de cada frontera coinciden, con el ajuste que sea.
///
/// El dibujo lleva diagonales, tramos rectos y nodos donde se juntan tres
/// colores —incluidos nodos por los que la frontera pasa de largo, que son los
/// que un ajuste por anillo simplificaría de forma distinta en cada cara.
#[test]
#[cfg(feature = "photo")]
fn las_dos_caras_de_una_frontera_se_ajustan_igual() {
    let dibujo = &[
        "rrrrr####",
        "rrrr#####",
        "rrr######",
        "rrooo####",
        "roooo####",
        "rooooo###",
        "rroooo###",
        "rrrooo###",
    ];
    for fit in [Fit::Pixel, Fit::polygon(), Fit::Polygon { tolerance: 2.5 }] {
        let out = convert_cluster(dibujo, fit);
        comprueba_costuras(&out);
    }
}

/// El mismo dibujo por la segmentación de clustering, que es la que comparte
/// fronteras entre regiones. La de rejilla traza cada una por su cuenta.
#[cfg(feature = "photo")]
fn convert_cluster(rows: &[&str], fit: Fit) -> Conversion {
    use img2svg::ClusterOptions;

    let (w, h, buf) = pixels(rows);
    let config = Config {
        fit,
        ..Config::cluster(ClusterOptions {
            // Sin filtrar: aquí se mira el ajuste, y con el umbral por defecto
            // un dibujo de este tamaño es todo motas.
            filter_speckle: 0,
            min_thickness: 0.0,
            ..ClusterOptions::default()
        })
    };
    img2svg::convert_rgba(w, h, &buf, &config).expect("la conversión no debe fallar")
}
