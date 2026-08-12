//! La escala de trabajo: a qué resolución se segmenta, que no tiene por qué ser
//! la del fichero.
//!
//! # Por qué existe
//!
//! Todas las constantes de simplificación de este modo son **píxeles absolutos**
//! —`filter_speckle` es un área, `min_thickness` un grosor, la tolerancia del
//! ajuste una distancia— y la retícula sobre la que se traza es la del original.
//! Lo que esos números significan depende entonces de cuántos píxeles gasta la
//! imagen en un rasgo, y dos imágenes cualesquiera no coinciden en eso ni de
//! lejos:
//!
//! | imagen | un rasgo mide | su grano mide | las constantes son |
//! | --- | --- | --- | --- |
//! | una portada de 300x300 | ~2 px | ~1 px | del tamaño del dibujo |
//! | un aerógrafo de 1800x2823 | ~200 px | ~2 px | del tamaño del ruido |
//!
//! En la primera no hay retícula suficiente para que quepa una curva, y en la
//! segunda no se simplifica nada. Medido, con el resto del proceso sin tocar y
//! sólo reescalando la entrada: la portada a 900 baja de 501 a 318 paths y de
//! 77,3 a 59,8 KB **y** sale bien dibujada; el aerógrafo a 900 de lado largo baja
//! de 6.895 a 376 paths y de 1.870 a 198 KB, sin grano.
//!
//! Así que la escala a la que se trabaja es un parámetro, y es el primero: lo que
//! decide es cuánto dibujo hay: subir de escala en una imagen pequeña —lo que
//! recupera el borde que el antialias del original dejó escrito entre píxeles— y
//! bajar en una grande, donde promediar es exactamente lo que quita el grano.
//!
//! # El mando es uno y no es la resolución
//!
//! Lo que se pide no es un tamaño, es cuánto detalle sobrevive: **el rasgo más
//! pequeño que se conserva, en tantos por mil del lado largo**. De ahí sale la
//! resolución de trabajo, porque las constantes viven en píxeles de trabajo y hay
//! que llevar ese rasgo a donde ellas están, que es [`FEATURE`] píxeles.
//!
//! Y sale una consecuencia que conviene ver escrita, porque es la unificación
//! entera en una línea: como el rasgo se pide en fracción del lado, **el lienzo
//! de trabajo depende sólo de la simplificación pedida y no del tamaño del
//! fichero**. Una miniatura de 300 px y un escaneo de 5 Mpx con la misma
//! simplificación se segmentan al mismo tamaño, que es lo que hace que un solo
//! número quiera decir lo mismo en las dos.

use image::RgbaImage;

/// A cuántos píxeles de trabajo se lleva el rasgo más pequeño que sobrevive.
///
/// Tres, y el motivo es la escalera de tolerancias del ajuste: sobre la retícula
/// hay un escalón en `0,5` px —se aplana un peldaño suelto— y otro en
/// `raíz(2)/2 = 0,707` —una escalera de 45 grados colapsa en su diagonal, y el
/// borde de un círculo pequeño es una sucesión de escaleras de 45 grados, así que
/// colapsa con ella—. Esos escalones están en píxeles absolutos, así que **dejan
/// de morder en cuanto la retícula es fina frente a la tolerancia**: con el rasgo
/// a tres píxeles, una tolerancia de 0,75 endereza los cantos largos y todavía no
/// llega a facetar un arco.
///
/// Por debajo de dos no hay retícula donde quepa una curva; por encima de cuatro
/// se paga resolución que ninguna constante aprovecha.
pub const FEATURE: f64 = 3.0;

/// La simplificación que se aplica cuando no se pide ninguna, en tantos por mil
/// del lado largo.
///
/// Equivale a segmentar con el lado largo en `FEATURE * 1000 / SIMPLIFY = 600` px.
/// Medido sobre las dos imágenes del informe, y el dato que lo fija es que entre
/// 600 y 900 de lado **no se distingue el resultado**: hay meseta, así que el
/// valor automático no tiene que ser fino, tiene que caer dentro. Y dentro de la
/// meseta se coge el extremo barato, que en el aerógrafo son 110 KB en vez de 198.
pub const SIMPLIFY: f64 = 5.0;

/// Hasta cuánto se sube de escala. Más allá no queda borde que reconstruir: el
/// antialias del original describe dónde cae el canto dentro del píxel, no una
/// forma cuatro veces más detallada.
const MAX_UPSCALE: f64 = 4.0;

/// Lado largo mínimo del lienzo de trabajo. Es una red por si llega una
/// simplificación absurda por la API: con menos que esto no hay dibujo que mirar.
const MIN_SIDE: usize = 96;

/// El lienzo al que hay que llevar una imagen de `w x h`, o `None` si ya está
/// donde tiene que estar.
///
/// `simplify` va en tantos por mil del lado largo y `0` apaga el reescalado, que
/// es la vía para trabajar sobre la retícula del original tal cual.
pub fn working_size(w: usize, h: usize, simplify: f64) -> Option<(usize, usize)> {
    if simplify <= 0.0 || w == 0 || h == 0 {
        return None;
    }
    let long = w.max(h) as f64;
    let target = (FEATURE * 1000.0 / simplify).max(MIN_SIDE as f64);
    // Nunca se sube más de `MAX_UPSCALE`; bajar no tiene tope, porque el propio
    // mando es el que dice cuánto dibujo se quiere y bajar sí quita ruido.
    let factor = (target / long).min(MAX_UPSCALE);

    // Un reescalado de un 2% cuesta lo mismo que uno de tres veces y no cambia
    // nada, salvo cambiar todas las coordenadas del documento.
    if (factor - 1.0).abs() < 0.02 {
        return None;
    }
    let scaled = |v: usize| ((v as f64 * factor).round() as usize).max(1);
    Some((scaled(w), scaled(h)))
}

/// Reescala a `tw x th`.
///
/// Separable y con alfa premultiplicada. Lo segundo no es un detalle: sin
/// premultiplicar, el filtro mezcla el color de los píxeles transparentes —que en
/// un PNG suele ser negro— con el del borde, y un dibujo recortado sale con una
/// orla oscura alrededor. Lo que hay que promediar es color *con* cobertura.
///
/// El filtro depende del sentido, porque los dos sentidos no son el mismo
/// problema:
///
/// - **bajando**, un triángulo de radio `1/escala`, que es promediar el área que
///   cae en el píxel de salida. Eso es lo que se lleva el grano, que es la razón
///   de bajar;
/// - **subiendo**, Catmull-Rom, que reconstruye el canto que el antialias del
///   original dejó escrito y no lo deja romo como haría el bilineal.
pub fn resize(img: &RgbaImage, tw: usize, th: usize) -> RgbaImage {
    let (sw, sh) = (img.width() as usize, img.height() as usize);
    // Premultiplicado y en real, que es donde se puede filtrar.
    let mut src = vec![0f32; sw * sh * 4];
    for (i, px) in img.as_raw().chunks_exact(4).enumerate() {
        let a = px[3] as f32 / 255.0;
        src[i * 4] = px[0] as f32 * a;
        src[i * 4 + 1] = px[1] as f32 * a;
        src[i * 4 + 2] = px[2] as f32 * a;
        src[i * 4 + 3] = px[3] as f32;
    }

    // Primero una pasada en x sobre la imagen entera y luego otra en y sobre el
    // resultado: dos pasadas de coste lineal en el radio en vez de una cuadrática.
    let horizontal = pass(&src, sw, sh, tw);
    let vertical = pass_transposed(&horizontal, tw, sh, th);

    let mut out = RgbaImage::new(tw as u32, th as u32);
    for (i, px) in out.as_mut().chunks_exact_mut(4).enumerate() {
        let a = vertical[i * 4 + 3].clamp(0.0, 255.0);
        // Deshacer la premultiplicación. Con cobertura nula no hay color que
        // recuperar y da igual cuál se escriba: no se ve.
        let un = |v: f32| {
            if a <= 0.0 {
                0
            } else {
                (v * 255.0 / a).round().clamp(0.0, 255.0) as u8
            }
        };
        px[0] = un(vertical[i * 4]);
        px[1] = un(vertical[i * 4 + 1]);
        px[2] = un(vertical[i * 4 + 2]);
        px[3] = a.round() as u8;
    }
    out
}

/// Una pasada en x: `sw -> tw` columnas, dejando las filas donde estaban.
fn pass(src: &[f32], sw: usize, sh: usize, tw: usize) -> Vec<f32> {
    let taps = weights(sw, tw);
    let mut out = vec![0f32; tw * sh * 4];
    for y in 0..sh {
        for (x, (first, ws)) in taps.iter().enumerate() {
            let mut acc = [0f32; 4];
            for (i, w) in ws.iter().enumerate() {
                let base = (y * sw + first + i) * 4;
                for c in 0..4 {
                    acc[c] += src[base + c] * w;
                }
            }
            let base = (y * tw + x) * 4;
            out[base..base + 4].copy_from_slice(&acc);
        }
    }
    out
}

/// La pasada en y, escrita como la de x sobre la imagen transpuesta para no
/// duplicar el bucle: se lee por columnas y se escribe en el sitio.
fn pass_transposed(src: &[f32], w: usize, sh: usize, th: usize) -> Vec<f32> {
    let taps = weights(sh, th);
    let mut out = vec![0f32; w * th * 4];
    for (y, (first, ws)) in taps.iter().enumerate() {
        for x in 0..w {
            let mut acc = [0f32; 4];
            for (i, weight) in ws.iter().enumerate() {
                let base = ((first + i) * w + x) * 4;
                for c in 0..4 {
                    acc[c] += src[base + c] * weight;
                }
            }
            let base = (y * w + x) * 4;
            out[base..base + 4].copy_from_slice(&acc);
        }
    }
    out
}

/// Para cada píxel de salida, el primer píxel de entrada que aporta y sus pesos.
///
/// Se calculan una vez por eje y se reutilizan en todas las filas o columnas, que
/// es lo que hace que un radio grande —bajar mucho de escala— no cueste por
/// píxel.
fn weights(src: usize, dst: usize) -> Vec<(usize, Vec<f32>)> {
    let scale = dst as f64 / src as f64;
    // Bajando, el soporte se estira para promediar todo lo que cae dentro; los
    // pesos son entonces un triángulo, que es la media del área con un borde
    // suave. Subiendo, el soporte es el de Catmull-Rom y no depende de la escala.
    let (support, cubic) = if scale < 1.0 {
        (1.0 / scale, false)
    } else {
        (2.0, true)
    };

    (0..dst)
        .map(|i| {
            // El centro del píxel de salida, llevado a coordenadas de entrada.
            let center = (i as f64 + 0.5) / scale - 0.5;
            let first = (center - support).ceil().max(0.0) as usize;
            let last = ((center + support).floor() as isize).min(src as isize - 1);
            let last = last.max(first as isize) as usize;

            let mut ws: Vec<f32> = (first..=last)
                .map(|j| {
                    let d = (j as f64 - center) / support;
                    let k = if cubic {
                        catmull_rom(d * 2.0)
                    } else {
                        1.0 - d.abs()
                    };
                    k.max(if cubic { f64::NEG_INFINITY } else { 0.0 }) as f32
                })
                .collect();
            // Los pesos se normalizan siempre, y eso es lo que arregla los bordes:
            // en la primera fila la mitad del soporte cae fuera de la imagen, y
            // repartir lo que hay evita que el borde se vaya a transparente.
            let sum: f32 = ws.iter().sum();
            if sum != 0.0 {
                for w in &mut ws {
                    *w /= sum;
                }
            } else {
                ws = vec![1.0];
            }
            (first, ws)
        })
        .collect()
}

/// Catmull-Rom: el cúbico que pasa por los puntos y no los suaviza.
///
/// Tiene lóbulos negativos, y son deseables —son los que dejan el canto afilado
/// en vez de romo—, así que el resultado hay que recortarlo al rango al escribir.
fn catmull_rom(x: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 {
        1.5 * x * x * x - 2.5 * x * x + 1.0
    } else if x < 2.0 {
        -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
    } else {
        0.0
    }
}
