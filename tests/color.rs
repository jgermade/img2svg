//! Cuantización de canales y distancia perceptual en Oklab.

use img2svg::color::Rgba;

fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::new(r, g, b, 255)
}

#[test]
fn la_cuantizacion_conserva_los_extremos() {
    // Recortar sin repartir dejaría el blanco en 248 con 5 bits, y la imagen
    // entera saldría un poco apagada.
    for bits in 1..=8 {
        assert_eq!(rgb(255, 255, 255).quantize(bits), rgb(255, 255, 255));
        assert_eq!(rgb(0, 0, 0).quantize(bits), rgb(0, 0, 0));
    }
}

#[test]
fn la_cuantizacion_deja_los_niveles_que_dicen_los_bits() {
    for bits in 1..=7 {
        let niveles: std::collections::BTreeSet<u8> =
            (0..=255u8).map(|v| rgb(v, 0, 0).quantize(bits).r).collect();
        assert_eq!(niveles.len(), 1 << bits, "con {bits} bits");
    }
}

#[test]
fn la_cuantizacion_es_idempotente() {
    // Si no lo fuera, cuantizar dos veces movería los cortes de los tramos y el
    // clustering dejaría de ser reproducible.
    for v in 0..=255u8 {
        let una = rgb(v, v / 2, 255 - v).quantize(5);
        assert_eq!(una.quantize(5), una, "sobre {v}");
    }
}

#[test]
fn la_cuantizacion_no_desplaza_mas_de_medio_nivel() {
    // Quedarse con los bits altos daría un nivel entero de error, no medio.
    for bits in 1..=7 {
        let salto = 255.0 / f64::from((1u16 << bits) - 1);
        for v in 0..=255u8 {
            let q = rgb(v, 0, 0).quantize(bits).r;
            let error = (f64::from(q) - f64::from(v)).abs();
            assert!(
                error <= salto / 2.0 + 0.5,
                "con {bits} bits, {v} -> {q}: error {error} sobre un salto de {salto}"
            );
        }
    }
}

#[test]
fn la_cuantizacion_no_apaga_la_imagen() {
    // El sesgo de truncar no se ve en un color aislado: se ve en el conjunto,
    // como medio nivel de menos en toda la imagen.
    fn media(f: impl Fn(u8) -> u8) -> f64 {
        (0..=255u8).map(|v| f64::from(f(v))).sum::<f64>() / 256.0
    }
    for bits in 1..=7 {
        let original = media(|v| v);
        let cuantizada = media(|v| rgb(v, 0, 0).quantize(bits).r);
        assert!(
            (cuantizada - original).abs() < 1.0,
            "con {bits} bits la media se va de {original} a {cuantizada}"
        );
    }
}

#[test]
fn ocho_bits_o_ninguno_no_tocan_el_color() {
    let c = Rgba::new(17, 200, 33, 90);
    assert_eq!(c.quantize(8), c);
    assert_eq!(c.quantize(9), c);
    assert_eq!(c.quantize(0), c);
}

#[test]
fn la_cuantizacion_tambien_recorta_el_alfa() {
    // Un degradado de transparencia trae el mismo ruido que uno de color.
    let niveles: std::collections::BTreeSet<u8> = (0..=255u8)
        .map(|a| Rgba::new(0, 0, 0, a).quantize(2).a)
        .collect();
    assert_eq!(niveles.len(), 4);
}

#[cfg(feature = "illustration")]
mod oklab {
    use super::rgb;
    use img2svg::color::{Oklab, Rgba};

    /// Los valores publicados por Ottosson para sRGB. Si la conversión se
    /// tuerce, aquí se ve, y no en una foto tres fases más adelante.
    #[test]
    fn coincide_con_los_valores_de_referencia() {
        let casos = [
            (rgb(255, 255, 255), (1.00000, 0.00000, 0.00000)),
            (rgb(255, 0, 0), (0.62796, 0.22486, 0.12585)),
            (rgb(0, 255, 0), (0.86644, -0.23389, 0.17950)),
            (rgb(0, 0, 255), (0.45201, -0.03246, -0.31153)),
        ];
        for (color, (l, a, b)) in casos {
            let got = Oklab::from(color);
            for (nombre, got, esperado) in [("l", got.l, l), ("a", got.a, a), ("b", got.b, b)] {
                assert!(
                    (f64::from(got) - esperado).abs() < 1e-4,
                    "{} {nombre}: {got} en vez de {esperado}",
                    color.to_hex()
                );
            }
        }
    }

    #[test]
    fn el_gris_no_tiene_croma() {
        for v in [0u8, 40, 128, 200, 255] {
            let c = Oklab::from(rgb(v, v, v));
            assert!(c.a.abs() < 1e-6 && c.b.abs() < 1e-6, "gris {v}: {c:?}");
        }
    }

    #[test]
    fn del_negro_al_blanco_hay_uno() {
        // Es lo que fija la escala de la distancia: se lee en fracciones de todo
        // el recorrido de luminosidad, así que 0.02 es el límite de lo notable.
        let d = Oklab::from(rgb(0, 0, 0)).distance(&Oklab::from(rgb(255, 255, 255)));
        assert!((d - 1.0).abs() < 1e-4, "distancia {d}");
    }

    /// La razón de ser del cambio de espacio: sobre un color saturado, la
    /// distancia ponderada en RGB llega a **invertir** el orden del ojo.
    #[test]
    fn rgb_invierte_el_orden_que_oklab_respeta() {
        // Un canal entero de azul metido en un amarillo saturado: apenas se
        // distingue, sólo se desatura.
        let (amarillo, mas_azul) = (rgb(255, 255, 0), rgb(255, 255, 60));
        // Un azul oscuro que sí cambia de tono a la vista.
        let (azul, mas_claro) = (rgb(0, 0, 60), rgb(0, 0, 100));

        assert!(
            amarillo.distance(&mas_azul) > azul.distance(&mas_claro),
            "en RGB el par amarillo ya no puntuaba más alto"
        );

        let par_amarillo = Oklab::from(amarillo).distance(&Oklab::from(mas_azul));
        let par_azul = Oklab::from(azul).distance(&Oklab::from(mas_claro));
        assert!(
            par_azul > 4.0 * par_amarillo,
            "en Oklab el azul sólo va {}x el amarillo",
            par_azul / par_amarillo
        );
    }

    #[test]
    fn dos_pares_iguales_en_rgb_se_separan_en_oklab() {
        // 13.0 y 13.3 en RGB: indistinguibles para un umbral. Para el ojo, el
        // salto de gris claro es la mitad que el cambio de tono del azul.
        let (gris, gris_claro) = (rgb(200, 200, 200), rgb(213, 213, 213));
        let (azul, azul_claro) = (rgb(0, 0, 60), rgb(0, 0, 100));
        assert!((gris.distance(&gris_claro) - azul.distance(&azul_claro)).abs() < 1.0);

        let par_gris = Oklab::from(gris).distance(&Oklab::from(gris_claro));
        let par_azul = Oklab::from(azul).distance(&Oklab::from(azul_claro));
        assert!(par_azul > 1.8 * par_gris, "{par_azul} frente a {par_gris}");
    }

    #[test]
    fn el_alfa_separa_dos_colores_por_lo_demas_iguales() {
        let opaco = Rgba::new(200, 30, 30, 255);
        let medio = Rgba::new(200, 30, 30, 128);
        assert_eq!(Oklab::from(opaco).distance(&Oklab::from(opaco)), 0.0);
        let d = Oklab::from(opaco).distance(&Oklab::from(medio));
        assert!(d > 0.4, "distancia {d}");
    }

    #[test]
    fn es_continuo_en_el_codo_de_la_gamma() {
        // La curva sRGB cambia de tramo en 0.04045, unos 10/255. Un salto ahí
        // saldría como una banda en las sombras.
        let bajo = Oklab::from(rgb(9, 9, 9)).distance(&Oklab::from(rgb(10, 10, 10)));
        let alto = Oklab::from(rgb(11, 11, 11)).distance(&Oklab::from(rgb(12, 12, 12)));
        assert!(bajo < 1.5 * alto, "salto {bajo} frente a {alto}");
    }

    #[test]
    fn la_luz_es_monotona_en_la_rampa_de_grises() {
        // `gradient_step` va a bandear sobre `l`; si no fuese monótona, una rampa
        // suave saldría con las bandas desordenadas.
        let mut previa = f32::NEG_INFINITY;
        for v in 0..=255u8 {
            let l = Oklab::from(rgb(v, v, v)).l;
            assert!(l > previa, "l no crece en {v}: {l} tras {previa}");
            previa = l;
        }
    }
}
