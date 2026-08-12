//! La escala de trabajo: a qué resolución se segmenta y qué promete el documento.
//!
//! Lo que se fija aquí no es el filtro —un reescalado se juzga mirándolo— sino las
//! cuatro cosas de las que depende el resto del modo: que un solo número decida el
//! lienzo de trabajo en cualquier imagen, que subir de escala tenga tope, que el
//! filtro no invente color donde no lo hay, y que el SVG siga anunciándose al
//! tamaño de la imagen que llegó.

#![cfg(feature = "illustration")]

use img2svg::resample::{self, FEATURE};
use img2svg::{ClusterOptions, Config, Detail};

/// La unificación, escrita como aserción: **el lienzo de trabajo sale de la
/// simplificación pedida y no del tamaño del fichero**.
///
/// Es lo que hace que `simplify` quiera decir lo mismo en una miniatura y en un
/// escaneo de 5 Mpx, y por tanto lo que hace que todas las demás constantes
/// —áreas, grosores, desviaciones, todas en píxeles absolutos— signifiquen lo
/// mismo en las dos.
#[test]
fn el_lienzo_de_trabajo_sale_del_mando_y_no_del_fichero() {
    // Dos imágenes que no se parecen en nada: una pequeña y cuadrada, otra grande
    // y apaisada, con la misma simplificación.
    let (aw, ah) = resample::working_size(400, 400, 5.0).expect("400 no es su escala");
    let (bw, bh) = resample::working_size(1800, 2823, 5.0).expect("1800 tampoco");

    let largo = FEATURE * 1000.0 / 5.0;
    assert_eq!(
        aw.max(ah) as f64,
        largo,
        "la pequeña sube hasta el objetivo"
    );
    assert_eq!(bw.max(bh) as f64, largo, "y la grande baja hasta el mismo");

    // Y cada una conserva su forma: reescalar no recorta ni estira.
    assert_eq!(aw, ah);
    let ratio = |w: usize, h: usize| w as f64 / h as f64;
    assert!((ratio(bw, bh) - ratio(1800, 2823)).abs() < 0.01);
}

/// Subir de escala tiene tope, y bajar no.
///
/// El tope existe porque lo que se recupera subiendo es el borde que el antialias
/// del original dejó escrito **dentro** del píxel, y eso se agota: más allá se
/// interpola una forma que nadie dibujó. Bajar no tiene tope porque el mando es
/// justo el que dice cuánto dibujo se quiere.
#[test]
fn subir_de_escala_tiene_tope() {
    // Una imagen de 60 px con la simplificación por defecto pediría x10.
    let (w, h) = resample::working_size(60, 60, resample::SIMPLIFY).expect("hay reescalado");
    assert_eq!((w, h), (240, 240), "cuatro veces y no más");

    // Bajando, en cambio, se llega a donde diga el mando.
    let (w, _) = resample::working_size(4000, 4000, 20.0).expect("hay reescalado");
    assert_eq!(w, (FEATURE * 1000.0 / 20.0) as usize, "x0,0375, sin tope");
}

/// `0` no reescala, y tampoco se reescala por un 2%: cambiar todas las
/// coordenadas del documento para mover el lienzo dos píxeles no compra nada.
#[test]
fn hay_casos_en_que_no_se_reescala() {
    assert_eq!(resample::working_size(300, 300, 0.0), None, "0 lo apaga");
    let largo = (FEATURE * 1000.0 / 5.0) as usize;
    assert_eq!(
        resample::working_size(largo, largo, 5.0),
        None,
        "y ya estando en la escala pedida, tampoco"
    );
    assert_eq!(
        resample::working_size(largo + 2, largo, 5.0),
        None,
        "ni por un 0,3%"
    );
}

/// Un color plano sigue plano: el cúbico tiene lóbulos negativos y en un borde
/// duro sobrepasa, pero **dentro** de una zona lisa no puede inventar nada.
///
/// Es la comprobación que separa un filtro de un error de signo: si los pesos no
/// suman uno, esto sale distinto del color de partida.
#[test]
fn un_color_plano_sigue_plano() {
    let plano = image::RgbaImage::from_pixel(40, 40, image::Rgba([37, 150, 190, 255]));
    for (w, h) in [(160, 160), (17, 17)] {
        let out = resample::resize(&plano, w, h);
        assert_eq!(out.dimensions(), (w as u32, h as u32));
        for px in out.pixels() {
            assert_eq!(px.0, [37, 150, 190, 255], "a {w}x{h}");
        }
    }
}

/// Bajando de escala, el borde de un dibujo recortado no se oscurece.
///
/// Es lo que compra premultiplicar: en un PNG con alfa los píxeles transparentes
/// suelen ser negro transparente, y filtrar color sin cobertura mezcla ese negro
/// con el borde y deja una orla. Lo que hay que promediar es color **con**
/// cobertura.
#[test]
fn el_borde_de_lo_transparente_no_se_ensucia() {
    // Un cuadrado rojo opaco sobre negro transparente.
    let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0]));
    for y in 16..48 {
        for x in 16..48 {
            img.put_pixel(x, y, image::Rgba([220, 40, 40, 255]));
        }
    }
    let out = resample::resize(&img, 32, 32);

    // Cualquier píxel con algo de cobertura tiene que seguir siendo rojo: el
    // canal rojo manda y los otros dos no aparecen.
    for px in out.pixels().filter(|p| p.0[3] > 8) {
        let [r, g, b, _] = px.0;
        assert!(
            r > 150 && g < 90 && b < 90,
            "un borde ensuciado por el negro transparente: {:?}",
            px.0
        );
    }
}

/// El contrato del documento: el `viewBox` va en píxeles **de trabajo** —así las
/// coordenadas salen tal como se trazaron— y `width`/`height` en los de la imagen
/// que llegó, que es el tamaño al que quien la trajo espera verla.
#[test]
fn el_documento_se_anuncia_al_tamano_de_la_imagen() {
    let (w, h) = (64u32, 48u32);
    let buf: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let v = (i % 255) as u8;
            [v, 255 - v, 128, 255]
        })
        .collect();

    let out = img2svg::convert_rgba(w, h, &buf, &Config::cluster(ClusterOptions::default()))
        .expect("la conversión no debe fallar");

    let escala = match out.detail {
        Detail::Cluster { scale, .. } => scale,
        _ => panic!("una ilustración trae detalle de clustering"),
    };
    assert_eq!(escala, 4.0, "una imagen de 64 px se sube hasta el tope");
    assert_eq!(out.canvas, (256, 192), "el lienzo es el de trabajo");
    assert!(
        out.svg
            .contains("width=\"64\" height=\"48\" viewBox=\"0 0 256 192\""),
        "el documento tiene que anunciarse a 64x48 sobre un viewBox de 256x192: {}",
        &out.svg[..120.min(out.svg.len())]
    );

    // Y sin reescalado, las dos cifras vuelven a ser la misma.
    let out = img2svg::convert_rgba(
        w,
        h,
        &buf,
        &Config::cluster(ClusterOptions {
            simplify: Some(0.0),
            ..ClusterOptions::default()
        }),
    )
    .expect("la conversión no debe fallar");
    assert_eq!(out.canvas, (64, 48));
    assert!(out
        .svg
        .contains("width=\"64\" height=\"48\" viewBox=\"0 0 64 48\""));
}
