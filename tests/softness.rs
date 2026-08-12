//! La medida de blandura: qué cuenta como borde y qué como transición.
//!
//! Lo que hay que fijar de una medida que va a decidir dónde se pone un degradado:
//! que un borde duro salga en cero, que una transición salga con su anchura, y que
//! **la misma pareja de colores** pueda salir dura en un sitio y blanda en otro. Lo
//! tercero es lo que hace que la medida sea de la frontera y no del par de colores,
//! y es la propiedad de la que depende todo lo demás: en un dibujo real dos tonos se
//! encuentran a veces en el canto de una forma y a veces en un sombreado.

#![cfg(feature = "illustration")]

use img2svg::color::Rgba;
use img2svg::ClusterOptions;

const CLARO: Rgba = Rgba {
    r: 255,
    g: 222,
    b: 181,
    a: 255,
};
const OSCURO: Rgba = Rgba {
    r: 222,
    g: 156,
    b: 90,
    a: 255,
};

/// Con paleta impuesta y sin filtrar nada: así lo que se mide es la medida y no lo
/// que la paleta haya decidido por su cuenta.
fn opciones() -> ClusterOptions {
    ClusterOptions {
        simplify: Some(0.0),
        filter_speckle: 0,
        min_thickness: 0.0,
        relax: 0.0,
        smoothing: 0,
        palette: vec![CLARO, OSCURO],
        ..ClusterOptions::default()
    }
}

/// Una imagen de `w x h` pintada por una función de la fila.
fn imagen(w: u32, h: u32, fila: impl Fn(u32) -> Rgba) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        let c = fila(y);
        for x in 0..w {
            img.put_pixel(x, y, image::Rgba([c.r, c.g, c.b, c.a]));
        }
    }
    img
}

/// El color a medio camino entre los dos, con `t` de 0 a 1.
fn mezcla(t: f64) -> Rgba {
    let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
    Rgba::new(
        lerp(CLARO.r, OSCURO.r),
        lerp(CLARO.g, OSCURO.g),
        lerp(CLARO.b, OSCURO.b),
        255,
    )
}

/// Un canto sin nada entre medias mide cero: al lado de la grieta ya está el color
/// de cada lado, y un color no es una mezcla de sí mismo.
#[test]
fn un_borde_duro_mide_cero() {
    let img = imagen(20, 20, |y| if y < 10 { CLARO } else { OSCURO });
    let medidas = img2svg::softness_of(&img, &opciones());

    assert_eq!(medidas.len(), 1, "una sola frontera interior");
    assert_eq!(medidas[0].width, 0.0, "un canto duro no tiene mezcla");
    assert_eq!(
        medidas[0].cracks, 20,
        "y mide lo que mide la imagen de ancho"
    );
}

/// Una transición mide su anchura, no la de la banda.
///
/// La rampa va de un color al otro en seis filas y la medida encuentra cuatro, no
/// seis, y eso es a propósito: las filas de las puntas están a menos de un 15% del
/// color de su lado y ahí ya no hay mezcla que medir, sólo el color con un poco de
/// ruido. La cuenta se queda con el interior de la zona, así que **subestima** la
/// anchura de verdad en un par de píxeles, siempre en el mismo sentido.
#[test]
fn una_transicion_mide_su_anchura() {
    let rampa = 6u32;
    let img = imagen(20, 26, |y| match y {
        y if y < 10 => CLARO,
        y if y < 10 + rampa => mezcla((y - 9) as f64 / (rampa + 1) as f64),
        _ => OSCURO,
    });
    let medidas = img2svg::softness_of(&img, &opciones());

    assert_eq!(medidas.len(), 1, "sigue siendo una frontera");
    let ancho = medidas[0].width;
    assert!(
        ancho >= rampa as f64 - 2.0 && ancho <= rampa as f64,
        "una rampa de {rampa} filas tiene que medir entre {} y {rampa}, no {ancho}",
        rampa - 2
    );
}

/// Y la misma pareja de colores, dura en un sitio y blanda en otro.
///
/// Es la propiedad que hace que esto sea una medida de la **frontera** y no del par
/// de colores, y sin ella no serviría para nada: en un dibujo de verdad los mismos
/// dos tonos se encuentran en el canto de una forma y en un sombreado, y hay que
/// poder tratarlos distinto.
#[test]
fn la_misma_pareja_puede_ser_dura_y_blanda() {
    // Arriba, canto seco; abajo, la misma pareja con una rampa de seis filas.
    let img = imagen(20, 46, |y| match y {
        y if y < 8 => CLARO,
        y if y < 20 => OSCURO,
        y if y < 30 => CLARO,
        y if y < 36 => mezcla((y - 29) as f64 / 7.0),
        _ => OSCURO,
    });
    let medidas = img2svg::softness_of(&img, &opciones());

    // Tres cambios de color: dos cantos secos y la rampa.
    assert_eq!(
        medidas.len(),
        3,
        "tres fronteras entre los mismos dos colores"
    );
    let mut anchos: Vec<f64> = medidas.iter().map(|m| m.width).collect();
    anchos.sort_by(f64::total_cmp);
    assert_eq!(&anchos[..2], &[0.0, 0.0], "los dos cantos miden cero");
    assert!(
        anchos[2] >= 3.0,
        "y la transición no, que ha salido {}",
        anchos[2]
    );
    // Y las tres con el mismo salto de color, que es lo que deja claro que no es el
    // color lo que las separa.
    assert!(medidas.windows(2).all(|w| w[0].jump == w[1].jump));
}
