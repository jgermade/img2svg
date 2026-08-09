//! Filtrado de motas: qué se funde, en quién, y qué no se toca.
#![cfg(feature = "photo")]

use image::RgbaImage;
use img2svg::cluster::{self, ClusterOptions, Clustering, NONE};
use img2svg::color::Rgba;
use img2svg::speckle;

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

/// Segmenta sin filtrar y filtra aparte, que es como se ve lo que hace el filtro
/// y no lo que hacía ya el agrupado.
fn filtrado(
    rows: &[&str],
    paleta: &[(char, Rgba)],
    max_area: usize,
    min_thickness: f64,
) -> Clustering {
    let mut c = cluster::from_image(
        &imagen(rows, paleta),
        &ClusterOptions {
            filter_speckle: 0,
            min_thickness: 0.0,
            ..ClusterOptions::default()
        },
    );
    speckle::filter(&mut c, max_area, min_thickness);
    c
}

/// El color con el que acaba pintado cada píxel, como dibujo, para poder afirmar
/// sobre el resultado y no sobre índices.
fn dibujo(c: &Clustering, paleta: &[(char, Rgba)]) -> Vec<String> {
    (0..c.height)
        .map(|y| {
            (0..c.width)
                .map(|x| {
                    let label = c.labels[y * c.width + x];
                    if label == NONE {
                        return '.';
                    }
                    let color = c.clusters[label as usize].color;
                    paleta
                        .iter()
                        .find(|&&(_, k)| k.to_hex() == color.to_hex())
                        .map(|&(ch, _)| ch)
                        .unwrap_or('?')
                })
                .collect()
        })
        .collect()
}

#[test]
fn el_grosor_mide_lo_que_dice() {
    // Los números de la tabla de la documentación, que son lo que justifica usar
    // area/perímetro en vez de la caja envolvente.
    assert_eq!(speckle::thickness(1, 4), 0.5);
    assert_eq!(speckle::thickness(4, 8), 1.0);
    assert_eq!(speckle::thickness(9, 12), 1.5);
    assert_eq!(speckle::thickness(100, 40), 5.0);
    // Una banda de 1x8: ocho píxeles, que un umbral de área a 4 dejaría vivos.
    assert!(speckle::thickness(8, 18) < 1.0);
}

#[test]
fn una_mota_se_funde_en_su_vecina() {
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let c = filtrado(&["RRRRR", "RRGRR", "RRRRR", "RRRRR"], paleta, 4, 0.0);
    assert_eq!(c.clusters.len(), 1);
    assert_eq!(c.clusters[0].color.to_hex(), ROJO.to_hex());
    assert_eq!(c.clusters[0].area, 20, "la mota suma al área de la vecina");
    assert_eq!(dibujo(&c, paleta), vec!["RRRRR", "RRRRR", "RRRRR", "RRRRR"]);
}

#[test]
fn lo_que_pasa_del_umbral_no_se_toca() {
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let c = filtrado(&["RRRRR", "RGGGR", "RGGGR", "RRRRR"], paleta, 4, 0.0);
    assert_eq!(c.clusters.len(), 2, "seis píxeles verdes no son una mota");
    assert_eq!(dibujo(&c, paleta), vec!["RRRRR", "RGGGR", "RGGGR", "RRRRR"]);
}

#[test]
fn la_banda_delgada_sobrevive_al_umbral_de_area_y_cae_por_grosor() {
    // El hallazgo que cambió el diseño: una banda de 1x5 tiene cinco píxeles, así
    // que el área no la ve. Es el reborde de antialias de cualquier borde.
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let dibujo_original = &["RRRRR", "GGGGG", "RRRRR", "RRRRR"];

    let solo_area = filtrado(dibujo_original, paleta, 4, 0.0);
    assert_eq!(solo_area.clusters.len(), 3, "el área deja la banda viva");

    let con_grosor = filtrado(dibujo_original, paleta, 4, 1.0);
    assert_eq!(con_grosor.clusters.len(), 1, "el grosor sí la funde");
    assert_eq!(
        dibujo(&con_grosor, paleta),
        vec!["RRRRR", "RRRRR", "RRRRR", "RRRRR"]
    );
}

#[test]
fn la_mota_va_a_quien_mas_frontera_comparte_no_a_la_mas_grande() {
    // El caso donde las dos reglas posibles discrepan, que es el único que decide
    // cuál está implementada. La banda `A` de cuatro píxeles comparte ocho lados
    // con el verde y dos con el rojo, y el rojo es casi el triple de grande: 34
    // píxeles frente a 12. Tiene que irse al verde.
    //
    // No es un detalle: una banda de reborde es, por definición, el borde de la
    // región de la que es reborde. Elegir por tamaño la mandaría al fondo grande
    // que apenas la toca, y el reborde reaparecería como un escalón de color
    // equivocado justo en el contorno.
    let paleta = &[('R', ROJO), ('G', VERDE), ('A', AZUL)];
    let c = filtrado(
        &[
            "RRRRRRRRRR",
            "RRGGGGGGRR",
            "RRGAAAAGRR",
            "RRGGRRGGRR",
            "RRRRRRRRRR",
        ],
        paleta,
        4,
        0.0,
    );
    assert_eq!(
        dibujo(&c, paleta),
        vec![
            "RRRRRRRRRR",
            "RRGGGGGGRR",
            "RRGGGGGGRR",
            "RRGGRRGGRR",
            "RRRRRRRRRR",
        ]
    );
    assert_eq!(c.clusters.len(), 2);
    let verde = c
        .clusters
        .iter()
        .find(|k| k.color.to_hex() == VERDE.to_hex())
        .unwrap();
    assert_eq!(verde.area, 16, "doce que tenía más los cuatro de la banda");
}

#[test]
fn una_mota_sin_vecinas_se_queda() {
    // Un punto suelto sobre transparente no tiene dónde fundirse, y borrarlo
    // abriría un agujero donde había dibujo.
    let paleta = &[('R', ROJO)];
    let c = filtrado(&["...", ".R.", "..."], paleta, 4, 1.0);
    assert_eq!(c.clusters.len(), 1);
    assert_eq!(c.clusters[0].area, 1);
    assert_eq!(dibujo(&c, paleta), vec!["...", ".R.", "..."]);
}

#[test]
fn una_isla_de_motas_acaba_junta() {
    // Dos motas de colores distintos que sólo se tocan entre ellas: no hay
    // superviviente a la que ir, así que se funden en una sola región y el color
    // sale de la mayor. Lo que no puede pasar es que se queden las dos.
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let c = filtrado(&[".RG.", ".RG."], paleta, 4, 0.0);
    assert_eq!(c.clusters.len(), 1, "{:?}", c.clusters);
    assert_eq!(c.clusters[0].area, 4);
}

#[test]
fn no_sobrevive_ninguna_mota_que_tuviera_donde_ir() {
    let paleta = &[('R', ROJO), ('G', VERDE), ('A', AZUL)];
    let c = filtrado(
        &[
            "RRRRRRRR", "RGRRRARR", "RRRRRRRR", "RRAGRRRR", "RRRRRRRR", "GRRRRRRA",
        ],
        paleta,
        4,
        0.0,
    );
    assert_eq!(c.clusters.len(), 1, "{:?}", c.clusters);
    assert_eq!(c.clusters[0].area, 48);
    assert_eq!(c.colors, 1);
}

#[test]
fn el_area_total_no_cambia() {
    // Fundir reparte píxeles, no los crea ni los borra.
    let paleta = &[('R', ROJO), ('G', VERDE), ('A', AZUL)];
    let rows = &["RRGA.", ".RAGR", "GGRRA", "RRARG"];
    let sin = filtrado(rows, paleta, 0, 0.0);
    let con = filtrado(rows, paleta, 4, 1.0);
    let suma = |c: &Clustering| c.clusters.iter().map(|k| k.area).sum::<usize>();
    assert_eq!(suma(&sin), suma(&con));
    // Y los transparentes siguen siendo exactamente los mismos.
    let huecos = |c: &Clustering| c.labels.iter().filter(|&&l| l == NONE).count();
    assert_eq!(huecos(&sin), huecos(&con));
}

#[test]
fn las_etiquetas_siguen_siendo_validas_y_el_orden_se_mantiene() {
    let paleta = &[('R', ROJO), ('G', VERDE), ('A', AZUL)];
    let c = filtrado(
        &["RRRRGGGG", "RRRRGGGG", "RRAGGGGG", "AAAAGGGG", "RRRRGGGG"],
        paleta,
        2,
        0.0,
    );
    for &label in &c.labels {
        assert!(label == NONE || (label as usize) < c.clusters.len());
    }
    for id in 0..c.clusters.len() as u32 {
        assert!(c.labels.contains(&id), "la región {id} no etiqueta nada");
    }
    // Las de un color, seguidas: es lo que `svg::render` necesita, y el filtro
    // reordena, así que hay que comprobarlo también después.
    let mut tramos: Vec<String> = Vec::new();
    for cluster in &c.clusters {
        let hex = cluster.color.to_hex();
        if tramos.last() != Some(&hex) {
            tramos.push(hex);
        }
    }
    let distintos: std::collections::BTreeSet<&String> = tramos.iter().collect();
    assert_eq!(tramos.len(), distintos.len(), "{tramos:?}");
}

#[test]
fn a_cero_no_hace_nada() {
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let rows = &["RRRRR", "RRGRR", "RRRRR"];
    let intacto = filtrado(rows, paleta, 0, 0.0);
    assert_eq!(intacto.clusters.len(), 2);
    assert_eq!(dibujo(&intacto, paleta), vec!["RRRRR", "RRGRR", "RRRRR"]);
}

#[test]
fn el_filtro_esta_puesto_por_defecto() {
    // Que exista y no esté conectado sería peor que no tenerlo.
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let c = cluster::from_image(
        &imagen(&["RRRRR", "RRGRR", "RRRRR"], paleta),
        &ClusterOptions::default(),
    );
    assert_eq!(c.clusters.len(), 1);
}

#[test]
fn el_resultado_no_depende_del_recorrido_de_la_tabla_hash() {
    let paleta = &[('R', ROJO), ('G', VERDE), ('A', AZUL)];
    let rows = &["RRGARRGA", "GARRGARR", "RRGARRGA", "AGRRAGRR"];
    let primera = filtrado(rows, paleta, 4, 1.0);
    for _ in 0..8 {
        let otra = filtrado(rows, paleta, 4, 1.0);
        assert_eq!(otra.labels, primera.labels);
        assert_eq!(
            otra.clusters
                .iter()
                .map(|k| (k.color.to_hex(), k.area))
                .collect::<Vec<_>>(),
            primera
                .clusters
                .iter()
                .map(|k| (k.color.to_hex(), k.area))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn en_una_imagen_grande_recorta_de_verdad_y_deprisa() {
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
    let sin = ClusterOptions {
        filter_speckle: 0,
        min_thickness: 0.0,
        ..ClusterOptions::default()
    };
    let antes = cluster::from_image(&img, &sin);

    let mut despues = antes.clone();
    let empezado = std::time::Instant::now();
    speckle::filter(&mut despues, 4, 1.0);
    let tardado = empezado.elapsed();

    println!(
        "{w}x{h}: {} regiones -> {} ({:.1}%) en {:?}",
        antes.clusters.len(),
        despues.clusters.len(),
        100.0 * despues.clusters.len() as f64 / antes.clusters.len() as f64,
        tardado
    );
    assert!(
        despues.clusters.len() * 4 < antes.clusters.len(),
        "de {} a {} no es recortar",
        antes.clusters.len(),
        despues.clusters.len()
    );
    // Y ni un píxel visible de más ni de menos.
    let visibles = |c: &Clustering| c.labels.iter().filter(|&&l| l != NONE).count();
    assert_eq!(visibles(&antes), visibles(&despues));
    assert_eq!(
        antes.clusters.iter().map(|k| k.area).sum::<usize>(),
        despues.clusters.iter().map(|k| k.area).sum::<usize>()
    );
}
