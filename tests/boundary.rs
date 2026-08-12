//! Extracción de fronteras: grietas, cadenas compartidas y anillos.
#![cfg(feature = "illustration")]

use std::collections::HashSet;

use image::RgbaImage;
use img2svg::boundary;
use img2svg::cluster::{self, ClusterOptions};
use img2svg::color::Rgba;
use img2svg::fit::Fit;
use img2svg::region::Regions;
use img2svg::svg;

const ROJO: Rgba = Rgba {
    r: 214,
    g: 41,
    b: 41,
    a: 255,
};
const VERDE: Rgba = Rgba {
    r: 41,
    g: 173,
    b: 74,
    a: 255,
};
const AZUL: Rgba = Rgba {
    r: 33,
    g: 74,
    b: 214,
    a: 255,
};
const AMARILLO: Rgba = Rgba {
    r: 224,
    g: 214,
    b: 41,
    a: 255,
};

fn imagen(rows: &[&str], paleta: &[(char, Rgba)]) -> RgbaImage {
    let (w, h) = (rows[0].len() as u32, rows.len() as u32);
    let mut img = RgbaImage::new(w, h);
    for (y, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), w as usize, "las filas no miden lo mismo");
        for (x, c) in row.chars().enumerate() {
            let color = if c == '.' {
                Rgba::new(0, 0, 0, 0)
            } else {
                paleta
                    .iter()
                    .find(|&&(k, _)| k == c)
                    .unwrap_or_else(|| panic!("carácter {c:?} sin color"))
                    .1
            };
            img.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([color.r, color.g, color.b, color.a]),
            );
        }
    }
    img
}

/// Las opciones de este fichero, con el filtrado de motas **apagado**: aquí se
/// prueba el agrupado y no el filtro, y con el umbral por defecto —cuatro
/// píxeles— casi cualquier región de un dibujo de ejemplo sería una mota.
fn opciones() -> ClusterOptions {
    ClusterOptions {
        filter_speckle: 0,
        min_thickness: 0.0,
        ..ClusterOptions::default()
    }
}

fn contornos(rows: &[&str], paleta: &[(char, Rgba)]) -> Regions {
    let img = imagen(rows, paleta);
    let clustering = cluster::from_image(&img, &opciones());
    boundary::from_clustering(&clustering)
}

/// Área encerrada por un anillo, con signo, por la fórmula del cordón de zapato.
/// El signo dice en qué sentido está recorrido.
fn area_con_signo(points: &[(i32, i32)]) -> f64 {
    let n = points.len();
    let mut doble = 0i64;
    for i in 0..n {
        let (x0, y0) = points[i];
        let (x1, y1) = points[(i + 1) % n];
        doble += x0 as i64 * y1 as i64 - x1 as i64 * y0 as i64;
    }
    doble as f64 / 2.0
}

/// La prueba de fuego, y dice más de lo que parece.
///
/// La suma **con signo** de los anillos de una región tiene que ser exactamente
/// `-área`. El valor absoluto ya obliga a que no falte ni sobre ningún tramo; el
/// signo, además, fija la orientación: con la `y` hacia abajo, recorrer un
/// contorno dejando el interior a la izquierda da área negativa. Y que sea la
/// suma y no la suma de valores absolutos es lo que obliga a que los agujeros
/// resten en vez de sumar.
///
/// Un tramo perdido, uno recorrido al revés, un anillo mal encadenado o un
/// agujero con el sentido cambiado fallan todos aquí.
fn comprueba_areas(regions: &Regions) {
    for (id, region) in regions.regions.iter().enumerate() {
        let total: f64 = region
            .rings
            .iter()
            .map(|ring| area_con_signo(&regions.ring_points(ring)))
            .sum();
        assert_eq!(
            total,
            -(region.area as f64),
            "la región {id} ({}) encierra {} y ocupa {} píxeles en {} anillos",
            region.color.to_hex(),
            total,
            region.area,
            region.rings.len()
        );
    }
}

#[test]
fn un_bloque_suelto_es_un_anillo() {
    let r = contornos(&["...", ".R.", "..."], &[('R', ROJO)]);
    assert_eq!(r.regions.len(), 1);
    assert_eq!(r.regions[0].rings.len(), 1);
    assert_eq!(r.edges.len(), 1);
    // Nadie al otro lado: es el borde con lo transparente.
    assert_eq!(r.edges[0].right, None);
    // Cuatro grietas, cinco puntos: el primero se repite al final para marcar que
    // el tramo se cierra.
    assert_eq!(r.edges[0].points.len(), 5);
    assert_eq!(r.edges[0].points.first(), r.edges[0].points.last());
    assert_eq!(r.ring_points(&r.regions[0].rings[0]).len(), 4);
    comprueba_areas(&r);
}

#[test]
fn la_frontera_entre_dos_regiones_es_un_solo_tramo() {
    // Lo que justifica todo el módulo: si cada región se trazara por su cuenta,
    // esta frontera saldría dos veces y con curvas se abriría una rendija.
    let r = contornos(&["RRGG", "RRGG"], &[('R', ROJO), ('G', VERDE)]);
    assert_eq!(r.regions.len(), 2);

    let compartidos: Vec<&img2svg::region::HalfEdge> =
        r.edges.iter().filter(|e| e.right.is_some()).collect();
    assert_eq!(compartidos.len(), 1, "{:?}", r.edges);
    // Y es vertical, de arriba abajo de la imagen, con los dos píxeles a los
    // lados.
    assert_eq!(compartidos[0].points, vec![(2, 0), (2, 1), (2, 2)]);
    comprueba_areas(&r);
}

#[test]
fn el_tramo_compartido_lo_usan_las_dos_caras() {
    let r = contornos(&["RRGG", "RRGG"], &[('R', ROJO), ('G', VERDE)]);
    let compartido = r
        .edges
        .iter()
        .position(|e| e.right.is_some())
        .expect("no hay tramo compartido");

    let usos: Vec<(usize, bool)> = r
        .regions
        .iter()
        .enumerate()
        .flat_map(|(id, region)| {
            region
                .rings
                .iter()
                .flatten()
                .filter(|&&(edge, _)| edge == compartido)
                .map(move |&(_, reversed)| (id, reversed))
        })
        .collect();

    assert_eq!(usos.len(), 2, "{usos:?}");
    // Una cara lo recorre en un sentido y la otra en el contrario. Es lo que
    // hace que la geometría ajustada valga para las dos.
    assert_ne!(usos[0].1, usos[1].1);
    assert_ne!(usos[0].0, usos[1].0);
}

#[test]
fn cada_grieta_de_frontera_se_usa_una_vez() {
    let r = contornos(
        &["RRGG.", "RAAG.", "RAAGG", ".GGGG"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    // Ninguna grieta repetida entre tramos: el conjunto de segmentos unidad de
    // todos los tramos no tiene duplicados.
    let mut vistas: HashSet<((i32, i32), (i32, i32))> = HashSet::new();
    for edge in &r.edges {
        for par in edge.points.windows(2) {
            let (a, b) = (par[0], par[1]);
            let clave = if a < b { (a, b) } else { (b, a) };
            assert!(vistas.insert(clave), "la grieta {clave:?} sale dos veces");
        }
    }
    comprueba_areas(&r);
}

#[test]
fn los_tramos_encadenan_por_los_extremos() {
    let r = contornos(
        &["RRGG.", "RAAG.", "RAAGG", ".GGGG"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    for region in &r.regions {
        for ring in &region.rings {
            // El final de cada tramo es el principio del siguiente, y el del
            // último cierra sobre el primero.
            for par in ring.windows(2) {
                assert_eq!(
                    final_de(&r, par[0]),
                    principio_de(&r, par[1]),
                    "el anillo no encadena"
                );
            }
            assert_eq!(
                final_de(&r, *ring.last().unwrap()),
                principio_de(&r, ring[0]),
                "el anillo no cierra"
            );
        }
    }
}

fn principio_de(r: &Regions, (edge, reversed): (usize, bool)) -> (i32, i32) {
    let p = &r.edges[edge].points;
    if reversed {
        p[p.len() - 1]
    } else {
        p[0]
    }
}

fn final_de(r: &Regions, (edge, reversed): (usize, bool)) -> (i32, i32) {
    let p = &r.edges[edge].points;
    if reversed {
        p[0]
    } else {
        p[p.len() - 1]
    }
}

#[test]
fn un_agujero_es_otro_anillo_de_la_misma_region() {
    let r = contornos(
        &["RRRRR", "RRRRR", "RRGRR", "RRRRR", "RRRRR"],
        &[('R', ROJO), ('G', VERDE)],
    );
    assert_eq!(r.regions.len(), 2);
    let rojo = r
        .regions
        .iter()
        .find(|region| region.color.to_hex() == ROJO.to_hex())
        .unwrap();
    assert_eq!(rojo.rings.len(), 2, "el contorno y el agujero");
    assert_eq!(rojo.area, 24);
    // El agujero resta, no suma: con los dos anillos sumando, el área sale la del
    // cuadrado entero.
    comprueba_areas(&r);
}

#[test]
fn cuatro_regiones_en_una_esquina_dan_un_nodo() {
    // Grado 4: ahí la frontera se bifurca y las cadenas tienen que partirse, o
    // una de ellas mezclaría dos pares de regiones distintos.
    let r = contornos(
        &["RG", "AY"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL), ('Y', AMARILLO)],
    );
    assert_eq!(r.regions.len(), 4);
    // Los cuatro tramos internos salen del centro (1,1) a cada lado.
    let internos = r.edges.iter().filter(|e| e.right.is_some()).count();
    assert_eq!(internos, 4, "{:?}", r.edges);
    for edge in r.edges.iter().filter(|e| e.right.is_some()) {
        assert_eq!(edge.points.len(), 2, "un tramo interno es una sola grieta");
        assert!(edge.points.contains(&(1, 1)));
    }
    comprueba_areas(&r);
}

#[test]
fn el_damero_es_el_caso_peor_y_tambien_cuadra() {
    // Todos los cruces de grado 4, y todas las regiones tocándose en diagonal.
    let r = contornos(
        &["RGRG", "GRGR", "RGRG", "GRGR"],
        &[('R', ROJO), ('G', VERDE)],
    );
    comprueba_areas(&r);
    // Con vecindad de 8 cada color es una sola región conexa por las diagonales,
    // y su contorno son varios anillos.
    assert_eq!(r.regions.len(), 2, "{:?}", r.regions.len());
    for region in &r.regions {
        assert_eq!(region.area, 8);
        assert_eq!(region.rings.len(), 8, "un anillo por píxel suelto");
    }
}

#[test]
fn dos_diagonales_de_la_misma_region_son_anillos_distintos() {
    let r = contornos(&["R.", ".R"], &[('R', ROJO)]);
    assert_eq!(r.regions.len(), 1, "por vecindad de 8 es una sola región");
    assert_eq!(r.regions[0].area, 2);
    assert_eq!(r.regions[0].rings.len(), 2, "pero dos anillos");
    comprueba_areas(&r);
}

#[test]
fn lo_transparente_no_es_region_pero_si_frontera() {
    let r = contornos(&["R.R", ".R.", "R.R"], &[('R', ROJO)]);
    // Cinco píxeles rojos, todos tocándose en diagonal: una región.
    assert_eq!(r.regions.len(), 1);
    assert_eq!(r.regions[0].area, 5);
    // Y ningún tramo tiene región al otro lado, porque el otro lado es el hueco.
    assert!(r.edges.iter().all(|e| e.right.is_none()));
    comprueba_areas(&r);
}

#[test]
fn las_areas_cuadran_en_un_dibujo_revuelto() {
    let r = contornos(
        &[
            "RRGGAAYY", "RGGAAYYR", "GGAAYYRR", "GAAYYRRG", "AAYYRRGG", "AYYRRGGA",
        ],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL), ('Y', AMARILLO)],
    );
    comprueba_areas(&r);
    let suma: usize = r.regions.iter().map(|region| region.area).sum();
    assert_eq!(suma, 48);
}

#[test]
fn el_documento_sale_valido() {
    let r = contornos(
        &["RRGG.", "RAAG.", "RAAGG", ".GGGG"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    let out = svg::render(
        &r,
        &svg::Options {
            pixel_size: 8,
            display: None,
            background: None,
            fit: Fit::Pixel,
        },
    );
    assert!(out.svg.starts_with("<svg"));
    assert!(out.svg.trim_end().ends_with("</svg>"));
    assert!(out.svg.contains("viewBox=\"0 0 5 4\""));
    assert_eq!(out.paths, r.regions.len());
    // Ningún path vacío: un `d` sin datos sería una región sin contorno.
    assert!(!out.svg.contains("d=\"\""));
    for region in &r.regions {
        assert!(out.svg.contains(&region.color.to_hex()));
    }
}

#[test]
fn las_etiquetas_ya_filtradas_dan_contornos_sanos() {
    // El resto del fichero extrae contornos de un etiquetado sin filtrar. Filtrar
    // reasigna etiquetas y reordena regiones, así que la cadena completa —con las
    // opciones de verdad, no las de los tests— quiere su propia comprobación.
    let img = imagen(
        &[
            "RRRRGGGG", "RRRRGGGG", "RRAGGGGG", "AAAAGGGG", "RRRRGGGG", "RRRRGGGG",
        ],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    let clustering = cluster::from_image(&img, &ClusterOptions::default());
    let r = boundary::from_clustering(&clustering);
    comprueba_areas(&r);
    assert_eq!(
        r.regions.iter().map(|region| region.area).sum::<usize>(),
        48
    );
}

#[test]
fn una_imagen_grande_termina() {
    let (w, h) = (1200u32, 1200u32);
    let mut img = RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let ruido = (x * 7 + y * 13) % 11;
        *px = image::Rgba([
            ((x / 8 + ruido) % 256) as u8,
            ((y / 8 + ruido) % 256) as u8,
            (128 + ruido) as u8,
            255,
        ]);
    }
    let clustering = cluster::from_image(&img, &opciones());
    let empezado = std::time::Instant::now();
    let r = boundary::from_clustering(&clustering);
    println!(
        "{w}x{h}: {} regiones, {} tramos en {:?}",
        r.regions.len(),
        r.edges.len(),
        empezado.elapsed()
    );
    assert_eq!(r.regions.len(), clustering.clusters.len());
    // Comprobar el área de todas las regiones de una imagen así es la prueba más
    // dura que hay a mano: cualquier error de topología en cualquiera salta.
    comprueba_areas(&r);
}
