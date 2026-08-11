//! Segmentación por clustering: paleta, componentes conexas y orden de salida.
#![cfg(feature = "photo")]

use std::collections::BTreeMap;

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
const AZUL: Rgba = Rgba {
    r: 33,
    g: 74,
    b: 214,
    a: 255,
};

/// Construye una imagen a partir de un dibujo. `.` es transparente y cada otro
/// carácter es un color de la tabla.
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

/// Las opciones de este fichero, con **todo lo que funde apagado**: el filtrado
/// de motas y el de grosor, porque aquí se prueba el agrupado y no el filtro —con
/// el umbral por defecto, cuatro píxeles, casi cualquier región de un dibujo de
/// ejemplo sería una mota—, y el suelo de la paleta y la regularización, porque
/// son las dos etapas que gastan la cota de color y casi todo lo que se afirma
/// aquí es sobre la cota estrecha.
fn opciones() -> ClusterOptions {
    ClusterOptions {
        min_color_share: 0.0,
        smoothing: 0,
        filter_speckle: 0,
        min_thickness: 0.0,
        ..ClusterOptions::default()
    }
}

fn segmenta(rows: &[&str], paleta: &[(char, Rgba)]) -> Clustering {
    cluster::from_image(&imagen(rows, paleta), &opciones())
}

/// Las regiones como `(color, área)`, que es lo que se quiere afirmar casi
/// siempre sin depender del orden.
fn resumen(c: &Clustering) -> BTreeMap<(String, usize), usize> {
    let mut out = BTreeMap::new();
    for cluster in &c.clusters {
        *out.entry((cluster.color.to_hex(), cluster.area))
            .or_insert(0) += 1;
    }
    out
}

#[test]
fn los_bloques_planos_salen_uno_por_region() {
    let c = segmenta(
        &["RRRGG", "RRRGG", "AAAGG"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    assert_eq!(c.clusters.len(), 3);
    assert_eq!(c.colors, 3);
    let areas: BTreeMap<String, usize> = c
        .clusters
        .iter()
        .map(|k| (k.color.to_hex(), k.area))
        .collect();
    assert_eq!(areas.len(), 3);
    assert_eq!(areas[&ROJO.to_hex()], 6);
    assert_eq!(areas[&VERDE.to_hex()], 6);
    assert_eq!(areas[&AZUL.to_hex()], 3);
}

#[test]
fn dos_bloques_del_mismo_color_separados_son_dos_regiones() {
    let c = segmenta(&["RR.RR", "RR.RR"], &[('R', ROJO)]);
    assert_eq!(c.clusters.len(), 2, "{:?}", c.clusters);
    // Un solo color de paleta, dos regiones: es la distinción que hace falta
    // para que el SVG las envuelva en un `<g>` y no en dos.
    assert_eq!(c.colors, 1);
    assert!(c.clusters.iter().all(|k| k.area == 4));
}

#[test]
fn los_que_se_tocan_en_diagonal_son_una_sola_region() {
    // Vecindad de 8, igual que en el camino de la rejilla: una diagonal de un
    // dibujo es una sola pieza.
    let c = segmenta(&["R.", ".R"], &[('R', ROJO)]);
    assert_eq!(c.clusters.len(), 1);
    assert_eq!(c.clusters[0].area, 2);
}

#[test]
fn el_ruido_por_debajo_de_la_tolerancia_no_parte_la_region() {
    // Cuatro tonos de rojo con dos o tres niveles de diferencia: compresión, no
    // dibujo. Sin cuantización ni tolerancia saldrían cuatro regiones.
    let casi = [
        ('a', Rgba::new(214, 41, 41, 255)),
        ('b', Rgba::new(216, 43, 40, 255)),
        ('c', Rgba::new(213, 39, 43, 255)),
        ('d', Rgba::new(215, 42, 42, 255)),
    ];
    let c = segmenta(&["abcd", "dcba", "abcd"], &casi);
    assert_eq!(c.clusters.len(), 1, "{:?}", c.clusters);
    assert_eq!(c.clusters[0].area, 12);
}

#[test]
fn los_colores_distintos_no_se_funden() {
    let c = segmenta(&["RG", "AR"], &[('R', ROJO), ('G', VERDE), ('A', AZUL)]);
    assert_eq!(c.colors, 3);
    // Tres regiones y no cuatro: los dos rojos están en diagonal, y con vecindad
    // de 8 eso es una sola pieza.
    assert_eq!(c.clusters.len(), 3);
    assert_eq!(
        c.clusters
            .iter()
            .find(|k| k.color.to_hex() == ROJO.to_hex())
            .unwrap()
            .area,
        2
    );
}

#[test]
fn los_transparentes_no_son_de_nadie() {
    let c = segmenta(&["R.R", "...", "R.R"], &[('R', ROJO)]);
    assert_eq!(c.clusters.len(), 4);
    assert_eq!(c.labels.iter().filter(|&&l| l == NONE).count(), 5);
    assert_eq!(c.labels.iter().filter(|&&l| l != NONE).count(), 4);
}

#[test]
fn el_alfa_por_encima_del_umbral_cuenta_y_separa() {
    // Medio transparente no es invisible, pero tampoco es el mismo color: el
    // alfa entra en la distancia.
    let translucido = Rgba::new(214, 41, 41, 200);
    let c = segmenta(&["RT"], &[('R', ROJO), ('T', translucido)]);
    assert_eq!(c.clusters.len(), 2, "{:?}", c.clusters);
    assert!(c.clusters.iter().any(|k| k.color.a < 255));
}

#[test]
fn las_areas_suman_los_pixeles_visibles() {
    let c = segmenta(
        &["RRGG.", ".AARR", "GG.RR"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    let visibles = c.labels.iter().filter(|&&l| l != NONE).count();
    assert_eq!(c.clusters.iter().map(|k| k.area).sum::<usize>(), visibles);
}

#[test]
fn cada_etiqueta_apunta_a_una_region_que_existe() {
    let c = segmenta(
        &["RRGG.", ".AARR", "GG.RR"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    for &label in &c.labels {
        assert!(
            label == NONE || (label as usize) < c.clusters.len(),
            "etiqueta {label} fuera de {} regiones",
            c.clusters.len()
        );
    }
    // Y toda región tiene al menos un píxel que la señala.
    for id in 0..c.clusters.len() as u32 {
        assert!(c.labels.contains(&id), "la región {id} no etiqueta nada");
    }
}

#[test]
fn las_regiones_de_un_color_van_seguidas() {
    // Es lo que `svg::render` necesita para envolver un color en un solo `<g>`:
    // recorre tramos contiguos de igual color y abre un grupo por cada tramo.
    let c = segmenta(
        &["R.R.R", "GGGGG", "R.R.R", "AAAAA", "R.R.R"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    let mut tramos: Vec<String> = Vec::new();
    for cluster in &c.clusters {
        let hex = cluster.color.to_hex();
        if tramos.last() != Some(&hex) {
            tramos.push(hex);
        }
    }
    let distintos: std::collections::BTreeSet<&String> = tramos.iter().collect();
    assert_eq!(
        tramos.len(),
        distintos.len(),
        "un color abre más de un tramo: {tramos:?}"
    );
}

#[test]
fn el_color_mas_presente_va_primero() {
    // Así los paths grandes quedan al fondo del documento, como en la rejilla.
    let c = segmenta(
        &["RRRRR", "RRRRR", "GGGAA", "GGGAA"],
        &[('R', ROJO), ('G', VERDE), ('A', AZUL)],
    );
    assert_eq!(c.clusters[0].color.to_hex(), ROJO.to_hex());
    assert_eq!(c.clusters[0].area, 10);
}

#[test]
fn el_resultado_no_depende_del_recorrido_de_la_tabla_hash() {
    // La paleta se construye recorriendo un HashMap, cuyo orden cambia entre
    // ejecuciones. Sin el desempate explícito esto saldría distinto cada vez.
    let rows = &["RRGGAA", "GGAARR", "AARRGG", "RRGGAA"];
    let paleta = &[('R', ROJO), ('G', VERDE), ('A', AZUL)];
    let primera = resumen(&segmenta(rows, paleta));
    for _ in 0..8 {
        let otra = segmenta(rows, paleta);
        assert_eq!(resumen(&otra), primera);
        assert_eq!(
            otra.clusters
                .iter()
                .map(|k| k.color.to_hex())
                .collect::<Vec<_>>(),
            segmenta(rows, paleta)
                .clusters
                .iter()
                .map(|k| k.color.to_hex())
                .collect::<Vec<_>>()
        );
    }
}

/// La garantía del módulo: la paleta se fija antes de recorrer la imagen, así que
/// ningún píxel queda a más de `tolerance` del color con el que se va a pintar.
/// Un clustering que fundiese regiones vecinas mientras avanza no puede
/// prometerlo, porque cada fusión mueve el color del grupo.
#[test]
fn ningun_pixel_se_pinta_mas_lejos_de_la_tolerancia() {
    let options = opciones();
    let mut img = RgbaImage::new(256, 32);
    for (x, y, px) in img.enumerate_pixels_mut() {
        // Un degradado en dos direcciones, para que la rampa no sea sólo gris.
        *px = image::Rgba([x as u8, (y * 8) as u8, 255 - x as u8, 255]);
    }
    let c = cluster::from_image(&img, &options);

    let mut peor = 0.0f64;
    for (i, &label) in c.labels.iter().enumerate() {
        let px = img.as_raw()[i * 4..i * 4 + 4].to_vec();
        let original = Rgba::new(px[0], px[1], px[2], px[3]);
        let pintado = c.clusters[label as usize].color;
        // El límite es sobre el color ya cuantizado, que es sobre lo que decide
        // la paleta; la cuantización añade su medio nivel aparte.
        let d =
            Oklab::from(original.quantize(options.color_precision)).distance(&Oklab::from(pintado));
        assert!(
            d <= options.tolerance + 1e-9,
            "el píxel {i} ({original:?}) se pinta {pintado:?}, a {d}"
        );
        peor = peor.max(Oklab::from(original).distance(&Oklab::from(pintado)));
    }
    // Y contando la cuantización, el error sigue siendo pequeño.
    assert!(peor < options.tolerance + 0.03, "peor caso {peor}");
    println!("peor desvío con cuantización incluida: {peor:.4}");
}

#[test]
fn con_los_valores_por_defecto_la_cota_es_la_del_arrastre() {
    // Las dos etapas que gastan la cota y vienen puestas de fábrica —el suelo de
    // la paleta y la regularización— tienen que **componer sin aflojarse**: la
    // cota conjunta es la más ancha de las dos, no la suma ni el producto. Se
    // prueba sobre la rampa, que es donde hay una frontera de decisión en cada
    // columna y por tanto donde las dos tienen más ocasión de morder.
    let options = ClusterOptions {
        filter_speckle: 0,
        min_thickness: 0.0,
        ..ClusterOptions::default()
    };
    assert!(
        options.min_color_share > 0.0 && options.smoothing > 0,
        "las dos vienen puestas"
    );

    let mut img = RgbaImage::new(256, 32);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = image::Rgba([x as u8, (y * 8) as u8, 255 - x as u8, 255]);
    }
    let c = cluster::from_image(&img, &options);

    let techo = options.tolerance * cluster::SNAP_CEILING;
    let mut peor = 0.0f64;
    for (i, &label) in c.labels.iter().enumerate() {
        let px = &img.as_raw()[i * 4..i * 4 + 4];
        let original = Rgba::new(px[0], px[1], px[2], px[3]);
        let pintado = c.clusters[label as usize].color;
        let d =
            Oklab::from(original.quantize(options.color_precision)).distance(&Oklab::from(pintado));
        assert!(
            d <= techo + 1e-9,
            "el píxel {i} ({original:?}) se pinta {pintado:?}, a {d}, con techo {techo}"
        );
        peor = peor.max(d);
    }
    assert!(
        peor > options.tolerance,
        "no ha gastado nada de la cota: {peor}"
    );
    println!("peor desvío con los valores por defecto: {peor:.4} (techo {techo:.4})");
}

/// Un plano de rojo con `n` píxeles de otro color estampados en una esquina.
///
/// Se construye a mano y no con [`imagen`] porque lo que se prueba aquí es una
/// fracción de la imagen, y para que el presupuesto signifique algo hace falta un
/// lienzo de miles de píxeles, no un dibujito de dieciséis de lado.
fn plano_con_mancha(n: usize, color: Rgba) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(64, 64, image::Rgba([ROJO.r, ROJO.g, ROJO.b, ROJO.a]));
    for i in 0..n {
        img.put_pixel(
            (i % 64) as u32,
            (i / 64) as u32,
            image::Rgba([color.r, color.g, color.b, color.a]),
        );
    }
    img
}

fn colores_de(img: &RgbaImage, options: &ClusterOptions) -> Vec<String> {
    cluster::from_image(img, options)
        .clusters
        .iter()
        .map(|k| k.color.to_hex())
        .collect()
}

/// Las opciones por defecto sin nada que funda regiones, para mirar sólo la
/// paleta.
fn solo_paleta() -> ClusterOptions {
    ClusterOptions {
        smoothing: 0,
        filter_speckle: 0,
        min_thickness: 0.0,
        ..ClusterOptions::default()
    }
}

#[test]
fn el_mismo_color_funda_entrada_o_no_segun_cuanto_pinte() {
    // El término del recuento de `min_color_share`, aislado: el **mismo** color a
    // la **misma** distancia del rojo (0,062 en Oklab, o sea fuera de la
    // tolerancia y dentro del techo), tres píxeles en un caso y seiscientos en el
    // otro. Antes los dos fundaban entrada, porque la frecuencia sólo ordenaba.
    let borde = Rgba::new(214, 82, 41, 255);
    let options = solo_paleta();

    let pocos = colores_de(&plano_con_mancha(3, borde), &options);
    assert!(
        !pocos.contains(&borde.to_hex()),
        "tres píxeles no dan para entrada propia: {pocos:?}"
    );

    let muchos = colores_de(&plano_con_mancha(600, borde), &options);
    assert!(
        muchos.contains(&borde.to_hex()),
        "seiscientos sí: {muchos:?}"
    );

    // Y sin suelo vuelven los dos, que es lo que dice que la diferencia la hace
    // el suelo y no otra cosa del camino.
    let sin_suelo = colores_de(
        &plano_con_mancha(3, borde),
        &ClusterOptions {
            min_color_share: 0.0,
            ..options
        },
    );
    assert!(
        sin_suelo.contains(&borde.to_hex()),
        "sin suelo funda cualquiera: {sin_suelo:?}"
    );
}

#[test]
fn un_color_lejano_funda_entrada_por_raro_que_sea() {
    // El techo, aislado. Un verde saturado está a 0,35 del rojo —más de
    // `SNAP_CEILING` veces la tolerancia—, así que se lleva entrada con tres
    // píxeles, los mismos con los que el rojo de borde del test anterior no se
    // la llevaba. Es lo que protege un lunar pequeño de acabar del color del
    // plano que lo rodea.
    let colores = colores_de(&plano_con_mancha(3, VERDE), &solo_paleta());
    assert!(
        colores.contains(&VERDE.to_hex()),
        "lo que está lejos de todo funda entrada aunque casi no pinte: {colores:?}"
    );
}

#[test]
fn un_degradado_se_reparte_en_bandas() {
    // Un vector no tiene degradado por región, así que una rampa suave tiene que
    // volverse escalones. Lo que no puede pasar es que salga una sola región de
    // un color plano que no se parece a ninguno de sus extremos.
    let mut img = RgbaImage::new(256, 4);
    for (x, _, px) in img.enumerate_pixels_mut() {
        *px = image::Rgba([x as u8, x as u8, x as u8, 255]);
    }
    let c = cluster::from_image(&img, &opciones());
    assert!(
        c.colors > 10 && c.colors < 60,
        "la rampa de grises da {} bandas",
        c.colors
    );
    // Cada banda es una franja vertical completa, luego una región por banda.
    assert_eq!(c.clusters.len(), c.colors);
}

#[test]
fn una_tolerancia_mas_alta_da_menos_regiones() {
    let mut img = RgbaImage::new(128, 16);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = image::Rgba([(x * 2) as u8, (y * 16) as u8, 128, 255]);
    }
    let pocas = cluster::from_image(
        &img,
        &ClusterOptions {
            tolerance: 0.2,
            ..opciones()
        },
    );
    let muchas = cluster::from_image(
        &img,
        &ClusterOptions {
            tolerance: 0.01,
            ..opciones()
        },
    );
    assert!(
        pocas.colors < muchas.colors,
        "{} frente a {}",
        pocas.colors,
        muchas.colors
    );
}

#[test]
fn menos_bits_dan_menos_colores() {
    let mut img = RgbaImage::new(128, 16);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = image::Rgba([(x * 2) as u8, (y * 16) as u8, 200, 255]);
    }
    // Con tolerancia cero la paleta es exactamente el resultado de cuantizar, así
    // que se ve el efecto de los bits sin que la agrupación lo tape.
    let bits = |b: u8| {
        cluster::from_image(
            &img,
            &ClusterOptions {
                color_precision: b,
                tolerance: 0.0,
                ..opciones()
            },
        )
        .colors
    };
    assert!(bits(3) < bits(5), "{} frente a {}", bits(3), bits(5));
    assert!(bits(5) < bits(8), "{} frente a {}", bits(5), bits(8));
}

#[test]
fn una_imagen_grande_termina_en_un_tiempo_razonable() {
    // El objetivo del plan es una foto de 4 Mpx en un par de segundos en wasm.
    // Aquí no se mide eso —esto es un test, no un banco, y en depuración va sin
    // optimizar—, sólo que el coste no se ha vuelto cuadrático.
    let (w, h) = (2000u32, 2000u32);
    let mut img = RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        // Degradado con ruido: los tramos salen cortos, que es el caso malo.
        let ruido = (x * 7 + y * 13) % 11;
        *px = image::Rgba([
            ((x / 8 + ruido) % 256) as u8,
            ((y / 8 + ruido) % 256) as u8,
            (128 + ruido) as u8,
            255,
        ]);
    }
    let empezado = std::time::Instant::now();
    let c = cluster::from_image(&img, &opciones());
    let tardado = empezado.elapsed();
    println!(
        "{}x{} en {:?}: {} colores, {} regiones",
        w,
        h,
        tardado,
        c.colors,
        c.clusters.len()
    );
    assert_eq!(c.labels.len(), (w * h) as usize);
    assert!(
        tardado.as_secs() < 60,
        "4 Mpx han tardado {tardado:?}: algo ha dejado de ser lineal"
    );
}
