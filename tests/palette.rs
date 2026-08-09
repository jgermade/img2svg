//! Bandeado de degradados, tope de colores, paleta impuesta y retirada de fondo.
#![cfg(feature = "photo")]

use std::collections::BTreeSet;

use image::RgbaImage;
use img2svg::cluster::{self, ClusterOptions, Clustering, NONE};
use img2svg::color::{Oklab, Rgba};

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
const BLANCO: Rgba = Rgba {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};

/// Sin filtrar motas: aquí se mira la paleta, y con el filtro puesto media rampa
/// de un píxel de alto sería una mota.
fn opciones() -> ClusterOptions {
    ClusterOptions {
        filter_speckle: 0,
        min_thickness: 0.0,
        ..ClusterOptions::default()
    }
}

fn imagen(rows: &[&str], paleta: &[(char, Rgba)]) -> RgbaImage {
    let (w, h) = (rows[0].len() as u32, rows.len() as u32);
    let mut img = RgbaImage::new(w, h);
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            let color = if c == '.' {
                Rgba::new(0, 0, 0, 0)
            } else {
                paleta.iter().find(|&&(k, _)| k == c).unwrap().1
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

/// Una rampa de gris horizontal, de negro a blanco.
fn rampa(w: u32, h: u32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    for (x, _, px) in img.enumerate_pixels_mut() {
        let v = (x * 255 / (w - 1)) as u8;
        *px = image::Rgba([v, v, v, 255]);
    }
    img
}

fn colores(c: &Clustering) -> BTreeSet<String> {
    c.clusters.iter().map(|k| k.color.to_hex()).collect()
}

#[test]
fn el_paso_de_degradado_ensancha_las_bandas() {
    let img = rampa(256, 4);
    let sin = cluster::from_image(&img, &opciones());
    let con = cluster::from_image(
        &img,
        &ClusterOptions {
            gradient_step: 0.15,
            ..opciones()
        },
    );
    assert!(
        con.colors * 2 < sin.colors,
        "de {} bandas a {} no es ensanchar",
        sin.colors,
        con.colors
    );
}

#[test]
fn ensanchar_por_luz_no_funde_tonos_distintos() {
    // La diferencia con subir la tolerancia. Rojo y verde están lejos en tono y
    // razonablemente cerca en luz, así que un `gradient_step` grande no los toca
    // mientras que una tolerancia igual de grande los fundiría en uno.
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let dibujo = &["RRRRGGGG", "RRRRGGGG"];

    let separacion = Oklab::from(ROJO).distance(&Oklab::from(VERDE));
    let banda = cluster::from_image(
        &imagen(dibujo, paleta),
        &ClusterOptions {
            gradient_step: separacion * 2.0,
            ..opciones()
        },
    );
    assert_eq!(banda.colors, 2, "el paso de luz ha fundido dos tonos");

    let tolerante = cluster::from_image(
        &imagen(dibujo, paleta),
        &ClusterOptions {
            tolerance: separacion * 2.0,
            ..opciones()
        },
    );
    assert_eq!(tolerante.colors, 1, "la tolerancia sí los funde");
}

#[test]
fn el_paso_de_degradado_a_cero_no_cambia_nada() {
    let img = rampa(256, 2);
    let sin = cluster::from_image(&img, &opciones());
    let cero = cluster::from_image(
        &img,
        &ClusterOptions {
            gradient_step: 0.0,
            ..opciones()
        },
    );
    assert_eq!(colores(&sin), colores(&cero));
    assert_eq!(sin.labels, cero.labels);
}

#[test]
fn el_tope_de_colores_se_respeta() {
    let img = rampa(256, 4);
    for tope in [2usize, 5, 12] {
        let c = cluster::from_image(
            &img,
            &ClusterOptions {
                max_colors: tope,
                ..opciones()
            },
        );
        assert_eq!(c.colors, tope, "con tope {tope}");
        assert_eq!(colores(&c).len(), tope);
    }
}

#[test]
fn con_tope_se_quedan_los_colores_mas_presentes() {
    // Mucho rojo, mucho verde y un pellizco de blanco: con tope de dos, el blanco
    // tiene que irse al más cercano de los otros dos y no quitarle el sitio a
    // ninguno.
    let paleta = &[('R', ROJO), ('G', VERDE), ('B', BLANCO)];
    let c = cluster::from_image(
        &imagen(&["RRRRGGGG", "RRRRGGGG", "RRRBGGGG"], paleta),
        &ClusterOptions {
            max_colors: 2,
            ..opciones()
        },
    );
    assert_eq!(c.colors, 2);
    assert_eq!(
        colores(&c),
        [ROJO.to_hex(), VERDE.to_hex()].into_iter().collect()
    );
}

#[test]
fn el_tope_no_estorba_si_sobran_entradas() {
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let c = cluster::from_image(
        &imagen(&["RG"], paleta),
        &ClusterOptions {
            max_colors: 16,
            ..opciones()
        },
    );
    assert_eq!(c.colors, 2);
}

#[test]
fn la_paleta_impuesta_es_la_que_sale() {
    // Ni un color más, y ninguno de los de la imagen: cada píxel va al más cercano
    // de los dados, por lejos que quede.
    let negro = Rgba::new(0, 0, 0, 255);
    let impuesta = vec![negro, BLANCO];
    let c = cluster::from_image(
        &rampa(256, 2),
        &ClusterOptions {
            palette: impuesta.clone(),
            ..opciones()
        },
    );
    assert_eq!(
        colores(&c),
        [negro.to_hex(), BLANCO.to_hex()].into_iter().collect(),
        "la rampa tiene que repartirse entre los dos extremos"
    );
}

#[test]
fn con_paleta_impuesta_nada_se_sale_de_ella() {
    // Y con colores que no se parecen a ninguna entrada, tampoco: el más cercano
    // gana igual, que es lo que distingue una paleta de un umbral.
    let paleta = &[('R', ROJO), ('G', VERDE), ('B', BLANCO)];
    let impuesta = vec![Rgba::new(0, 0, 0, 255), BLANCO];
    let c = cluster::from_image(
        &imagen(&["RRGGBB", "RRGGBB"], paleta),
        &ClusterOptions {
            palette: impuesta.clone(),
            ..opciones()
        },
    );
    for hex in colores(&c) {
        assert!(
            impuesta.iter().any(|c| c.to_hex() == hex),
            "{hex} no estaba en la paleta"
        );
    }
}

#[test]
fn la_paleta_impuesta_manda_sobre_la_tolerancia() {
    // Con tolerancia enorme se fundiría todo en un color; la paleta impuesta no
    // deja, porque no es un límite de distancia sino un conjunto cerrado.
    let paleta = &[('R', ROJO), ('B', BLANCO)];
    let c = cluster::from_image(
        &imagen(&["RRBB"], paleta),
        &ClusterOptions {
            tolerance: 10.0,
            palette: vec![ROJO, BLANCO],
            ..opciones()
        },
    );
    assert_eq!(c.colors, 2);
}

#[test]
fn el_fondo_se_va_por_el_borde_y_lo_de_dentro_se_queda() {
    // El blanco del borde se va; el blanco encerrado dentro del dibujo —el brillo
    // de un ojo, siempre el mismo ejemplo— se queda, porque es otra región.
    let paleta = &[('R', ROJO), ('B', BLANCO)];
    let c = cluster::from_image(
        &imagen(&["BBBBBB", "BRRRRB", "BRBBRB", "BRRRRB", "BBBBBB"], paleta),
        &ClusterOptions {
            remove_background: true,
            ..opciones()
        },
    );
    assert_eq!(c.background, Some(BLANCO));
    // Quedan el rojo y el blanco de dentro.
    assert_eq!(
        colores(&c),
        [ROJO.to_hex(), BLANCO.to_hex()].into_iter().collect()
    );
    // Y el recorte se ha comido el marco: de 6x5 a 4x3.
    assert_eq!((c.width, c.height), (4, 3));
    let dentro: usize = c
        .clusters
        .iter()
        .filter(|k| k.color.to_hex() == BLANCO.to_hex())
        .map(|k| k.area)
        .sum();
    assert_eq!(dentro, 2, "el blanco de dentro son dos píxeles");
}

#[test]
fn sin_fondo_dominante_no_se_toca_nada() {
    // Hacen falta tres colores en el borde para que ninguno llegue a la mitad.
    // Con dos a medias justas la mitad se alcanza —el umbral es `>=`, igual que en
    // el camino de la rejilla, del que esto copia el criterio— y uno de los dos se
    // lleva el puesto.
    let paleta = &[('R', ROJO), ('G', VERDE), ('B', BLANCO)];
    let c = cluster::from_image(
        &imagen(&["RRGGBB", "RRGGBB", "RRGGBB"], paleta),
        &ClusterOptions {
            remove_background: true,
            ..opciones()
        },
    );
    assert_eq!(c.background, None);
    assert_eq!((c.width, c.height), (6, 3));
    assert_eq!(c.clusters.iter().map(|k| k.area).sum::<usize>(), 18);
}

#[test]
fn con_medio_borde_justo_si_hay_fondo() {
    // El caso de arriba al otro lado del umbral, para que quede escrito cuál es:
    // dos colores a mitades exactas del borde, y el `>=` deja que uno gane.
    let paleta = &[('R', ROJO), ('G', VERDE)];
    let c = cluster::from_image(
        &imagen(&["RRRGGG", "RRRGGG", "RRRGGG"], paleta),
        &ClusterOptions {
            remove_background: true,
            ..opciones()
        },
    );
    assert!(c.background.is_some(), "medio borde ya es mayoría");
}

#[test]
fn quitar_el_fondo_no_deja_etiquetas_rotas() {
    let paleta = &[('R', ROJO), ('B', BLANCO), ('G', VERDE)];
    let c = cluster::from_image(
        &imagen(
            &["BBBBBBBB", "BRRGGRRB", "BRGGGGRB", "BBRRGGBB", "BBBBBBBB"],
            paleta,
        ),
        &ClusterOptions {
            remove_background: true,
            ..opciones()
        },
    );
    for &label in &c.labels {
        assert!(label == NONE || (label as usize) < c.clusters.len());
    }
    for id in 0..c.clusters.len() as u32 {
        assert!(c.labels.contains(&id), "la región {id} no etiqueta nada");
    }
    let visibles = c.labels.iter().filter(|&&l| l != NONE).count();
    assert_eq!(c.clusters.iter().map(|k| k.area).sum::<usize>(), visibles);
    // Y las de un color siguen seguidas, que es lo que necesita el SVG.
    let mut tramos: Vec<String> = Vec::new();
    for cluster in &c.clusters {
        let hex = cluster.color.to_hex();
        if tramos.last() != Some(&hex) {
            tramos.push(hex);
        }
    }
    let distintos: BTreeSet<&String> = tramos.iter().collect();
    assert_eq!(tramos.len(), distintos.len(), "{tramos:?}");
}

#[test]
fn las_motas_del_fondo_se_van_con_el_fondo() {
    // El orden de las dos etapas. La mota roja está sobre el fondo blanco: se
    // funde con él y desaparece con él. Al revés, quitando el fondo primero, se
    // quedaría flotando sin nada alrededor en que fundirse.
    let paleta = &[('R', ROJO), ('B', BLANCO), ('G', VERDE)];
    let c = cluster::from_image(
        &imagen(
            &["BBBBBBBB", "BBBRBBBB", "BBGGGGBB", "BBGGGGBB", "BBBBBBBB"],
            paleta,
        ),
        &ClusterOptions {
            remove_background: true,
            ..ClusterOptions::default()
        },
    );
    assert_eq!(c.background, Some(BLANCO));
    assert_eq!(colores(&c), [VERDE.to_hex()].into_iter().collect());
    assert_eq!((c.width, c.height), (4, 2));
}
