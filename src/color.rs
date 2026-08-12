//! Tipos de color, distancias y reducción de paleta.
//!
//! Hay dos formas de medir cuánto se parecen dos colores, y son de dos caminos
//! distintos a propósito: [`Rgba::distance`] para la rejilla y [`Oklab`] para el
//! clustering. Ver el porqué en [`Oklab`].

/// Color RGBA de un píxel lógico. `None` en el mapa equivale a transparente.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba { r, g, b, a }
    }

    /// Distancia perceptual aproximada (ponderada por sensibilidad del ojo),
    /// en la misma escala que un canal 0..255.
    pub fn distance(&self, other: &Rgba) -> f64 {
        let dr = self.r as f64 - other.r as f64;
        let dg = self.g as f64 - other.g as f64;
        let db = self.b as f64 - other.b as f64;
        let da = self.a as f64 - other.a as f64;
        (0.30 * dr * dr + 0.59 * dg * dg + 0.11 * db * db + da * da).sqrt()
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Reduce cada canal a `2^bits` niveles repartidos por todo el rango, y
    /// devuelve el valor al más cercano de ellos. Fuera de 1..=8 no hace nada.
    ///
    /// Es lo primero que hace el clustering, antes de agrupar y no después:
    /// cuesta lo mismo por píxel sea la imagen que sea, y hace estables los
    /// cortes entre tramos —dos píxeles que sólo se diferencian en el ruido del
    /// último bit acaban idénticos y dejan de abrir un corte donde no hay borde.
    ///
    /// Al nivel **más cercano**, y no quedándose con los bits altos, que es lo
    /// que se suele hacer y lo que hace `--color-precision` de VTracer. Truncar
    /// redondea siempre hacia abajo: cada canal pierde de media medio nivel —con
    /// 5 bits, cuatro de 255— y la imagen entera sale un poco apagada. El sesgo
    /// no se ve en un color aislado, se ve en el conjunto.
    ///
    /// Los extremos se conservan: con 1 bit los niveles son `0` y `255`, no `0` y
    /// `128`, así que el blanco sigue siendo blanco.
    ///
    /// El alfa se recorta igual que el resto. Un degradado de transparencia
    /// tiene el mismo ruido que uno de color y merece el mismo trato.
    pub fn quantize(self, bits: u8) -> Rgba {
        if bits == 0 || bits >= 8 {
            return self;
        }
        let max = u16::from((1u8 << bits) - 1);
        let level = |c: u8| {
            let paso = (u16::from(c) * max + 127) / 255;
            ((paso * 255 + max / 2) / max) as u8
        };
        Rgba::new(level(self.r), level(self.g), level(self.b), level(self.a))
    }
}

/// Color en Oklab, el espacio donde la distancia euclídea se corresponde con lo
/// que ve el ojo.
///
/// [`Rgba::distance`] vale para el ruido de compresión del pixel art, donde los
/// tonos que se funden son ya casi iguales de por sí, pero no para agrupar los
/// de una foto. Sus pesos `0.30/0.59/0.11` son los coeficientes de luminancia,
/// así que lo que mide en realidad es **cuánto ha cambiado el brillo**, y sobre
/// un color saturado eso llega a dar la vuelta al orden del ojo:
///
/// | par | en RGB | en Oklab |
/// | --- | --- | --- |
/// | amarillo `255,255,0` → `255,255,60` | 19.9 | 0.014 |
/// | azul oscuro `0,0,60` → `0,0,100` | 13.3 | 0.081 |
///
/// Al amarillo se le ha metido un canal entero de azul y apenas se distingue
/// —se desatura un poco—, mientras que el azul oscuro cambia de tono a la vista.
/// RGB dice que el primer par está *más* separado que el segundo; Oklab, que el
/// segundo lo está casi seis veces más. Un solo umbral sobre esa métrica no
/// existe: el que respeta los azules del cielo trocea cualquier superficie
/// saturada, y el que no la trocea funde el cielo en un borrón.
///
/// Lo que no arregla, y conviene no prometer: en la escala de grises la
/// distancia en RGB ya es bastante decente —la gamma de sRGB es de por sí casi
/// perceptual, y de las sombras a las luces Oklab sólo la corrige un 50%—. La
/// diferencia está en el croma, no en la luz.
///
/// Aparte, tener `l` separada del croma es lo que va a necesitar el bandeado de
/// degradados: cuantizar la luz dejando el tono quieto no se puede expresar
/// sobre tres canales de RGB.
///
/// El camino de la rejilla sigue con la distancia en RGB: es la que produjo sus
/// once instantáneas y cambiarla las movería todas de golpe, sin que eso mejore
/// nada de lo que ese camino hace.
#[cfg(feature = "illustration")]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Oklab {
    /// Luminosidad, 0..1 aproximadamente.
    pub l: f32,
    /// Eje verde–rojo.
    pub a: f32,
    /// Eje azul–amarillo.
    pub b: f32,
    /// Alfa en 0..1. Oklab no lo cubre, y hay que arrastrarlo para que dos
    /// píxeles del mismo color con transparencia distinta no acaben juntos.
    pub alpha: f32,
}

#[cfg(feature = "illustration")]
impl Oklab {
    /// Matrices de Björn Ottosson: sRGB lineal → LMS, raíz cúbica, LMS → Lab.
    //
    // Las constantes van con todos sus dígitos, tal como están publicadas,
    // aunque en `f32` sobre la mitad: así se pueden cotejar con la fuente de un
    // vistazo, que es lo que hace comprobable el test de valores de referencia.
    #[allow(clippy::excessive_precision)]
    pub fn from_rgba(c: Rgba) -> Self {
        let r = srgb_to_linear(c.r);
        let g = srgb_to_linear(c.g);
        let b = srgb_to_linear(c.b);

        let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
        let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
        let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

        let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());

        Oklab {
            l: 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            a: 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            b: 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
            alpha: f32::from(c.a) / 255.0,
        }
    }

    /// Distancia perceptual. La escala es la de la luminosidad: `1.0` es todo el
    /// recorrido de negro a blanco, y a partir de `0.02` la diferencia se
    /// empieza a notar.
    ///
    /// El alfa entra con el mismo peso que la luminosidad, que es lo que ya
    /// hacía [`Rgba::distance`] en su propia escala.
    pub fn distance(&self, other: &Oklab) -> f64 {
        let dl = f64::from(self.l - other.l);
        let da = f64::from(self.a - other.a);
        let db = f64::from(self.b - other.b);
        let dalpha = f64::from(self.alpha - other.alpha);
        (dl * dl + da * da + db * db + dalpha * dalpha).sqrt()
    }

    /// Cuánta de esa distancia es sólo luz.
    pub fn lightness_gap(&self, other: &Oklab) -> f64 {
        f64::from(self.l - other.l).abs()
    }

    /// Y cuánta es todo lo demás: tono, saturación y alfa.
    ///
    /// Separar las dos es lo que permite bandear un degradado sin fundir tonos
    /// distintos, que sobre tres canales de RGB no se puede ni plantear.
    pub fn chroma_distance(&self, other: &Oklab) -> f64 {
        let da = f64::from(self.a - other.a);
        let db = f64::from(self.b - other.b);
        let dalpha = f64::from(self.alpha - other.alpha);
        (da * da + db * db + dalpha * dalpha).sqrt()
    }
}

#[cfg(feature = "illustration")]
impl From<Rgba> for Oklab {
    fn from(c: Rgba) -> Self {
        Oklab::from_rgba(c)
    }
}

/// De sRGB con gamma a sRGB lineal, por tabla: la potencia de 2.4 es de lejos lo
/// más caro de la conversión, y sólo hay 256 valores posibles de entrada.
#[cfg(feature = "illustration")]
fn srgb_to_linear(c: u8) -> f32 {
    use std::sync::OnceLock;
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|i| {
            let x = i as f32 / 255.0;
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        })
    })[usize::from(c)]
}

/// Agrupa los colores parecidos en una paleta reducida.
///
/// Se recorren los colores de más a menos frecuente: cada uno se fusiona con la
/// primera entrada de la paleta que quede dentro de `tolerance`, de forma que
/// el ruido de compresión colapsa sobre el tono dominante.
pub fn build_palette(counts: &[(Rgba, usize)], tolerance: f64) -> Vec<(Rgba, Rgba)> {
    let mut sorted: Vec<(Rgba, usize)> = counts.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_hex().cmp(&b.0.to_hex())));

    let mut palette: Vec<Rgba> = Vec::new();
    let mut mapping = Vec::with_capacity(sorted.len());

    for (color, _) in sorted {
        if tolerance > 0.0 {
            if let Some(hit) = palette
                .iter()
                .copied()
                .filter(|p| p.distance(&color) <= tolerance)
                .min_by(|a, b| a.distance(&color).partial_cmp(&b.distance(&color)).unwrap())
            {
                mapping.push((color, hit));
                continue;
            }
        }
        palette.push(color);
        mapping.push((color, color));
    }

    mapping
}
