//! Cuántos píxeles tarda una frontera en pasar de un color al otro.
//!
//! # Para qué
//!
//! Es la medida de la que depende decidir dónde el original dibujó un **borde** y
//! dónde dibujó una **transición**. Un trazo de tinta contra una piel cambia de
//! color en un píxel; el terminador de una superficie redonda pintada a aerógrafo
//! tarda cinco o diez. Lo primero tiene que salir duro y lo segundo pide un
//! degradado, y sin esta medida no hay forma de distinguirlos: en la paleta las dos
//! cosas son dos regiones vecinas de colores distintos y nada más.
//!
//! # Por qué aquí y no con un detector de bordes
//!
//! Porque un detector genérico mira la imagen sin saber qué separa cada borde, y
//! entonces lo único que puede preguntar es *dónde cambia el color*. Esa pregunta
//! está mal planteada: cruzando un trazo de dos píxeles sobre una piel el color se
//! mueve seis u ocho píxeles seguidos —piel, dentro del trazo, fuera del trazo—, así
//! que un recorrido en línea recta cuenta una transición ancha donde hay dos bordes
//! duros pegados. Medido así, la portada de un disco —cartel de color plano con
//! trazo negro— sale blanda en el 58% de sus bordes, que es al revés de la verdad.
//!
//! Aquí eso no pasa porque cuando se llega a esta etapa **los dos colores ya están
//! decididos**: una frontera separa la región `A` de la región `B` y sus entradas de
//! paleta se conocen antes de medir nada. La pregunta pasa a ser *cuántos píxeles a
//! lo largo de esta normal son una mezcla de estos dos colores concretos*, que es la
//! misma proyección que [`crate::speckle`] usa para distinguir un reborde de un
//! trazo. Tercera vez que esa proyección paga.
//!
//! # Qué hace y qué no
//!
//! Mide y devuelve. No funde nada, no toca el documento y no decide ningún umbral:
//! el umbral sale de mirar la distribución en imágenes de verdad, y hasta que esa
//! distribución esté mirada no hay criterio que escribir. Ver
//! `SESSIONS/2026-08-12_02h14.soft-seams-and-what-would-force-them.md`.

use image::RgbaImage;

use crate::cluster::Clustering;
use crate::color::{Oklab, Rgba};
use crate::region::Regions;
use crate::speckle;

/// Hasta cuántos píxeles se mira a cada lado de la grieta.
///
/// Ocho: por encima de la transición más ancha que tiene sentido llamar blanda a la
/// escala de trabajo, donde el lado largo son unos cientos de píxeles. Más lejos, lo
/// que se cuenta ya es otra cosa del dibujo y no esta frontera.
const REACH: usize = 8;

/// Hasta dónde puede apartarse del segmento entre las dos caras el color de un
/// píxel para contar como mezcla de ellas, en múltiplos de `tolerance`.
///
/// El mismo techo que usa el test de rebordes, y por el mismo motivo: la entrada de
/// la paleta de cada cara ya trae su error, y la mezcla que se busca se hizo entre
/// los colores de verdad y no entre sus entradas.
const CEILING: f64 = 3.0;

/// Qué parte del camino entre las dos caras hay que llevar para contar como mezcla.
///
/// Un píxel pegado a una cara tiene su color, no una mezcla, y contarlo haría que la
/// cuenta se fuese por el interior liso de la región hasta el tope. Con el margen,
/// un borde duro mide cero: al lado de la grieta ya está el color de cada lado.
const MARGIN: f64 = 0.15;

/// Blandura, en píxeles de trabajo, a partir de la cual una frontera es una
/// transición y no un canto.
///
/// Cinco, y sale de mirar la distribución en tres dibujos que no se parecen en nada,
/// pesada por píxeles de frontera:
///
/// | blandura | `Sonic1.png` | `cover.jpg` | un cielo sintético |
/// | --- | --- | --- | --- |
/// | 0–1 px | 28% | 19% | 34% |
/// | 2–4 px | 20% | **72%** | 0% |
/// | 5–8 px | 24% | 2% | 0% |
/// | 9–16 px | 20% | 1% | **64%** |
///
/// La portada es un cartel de color plano: **el 91% de sus fronteras no llega a 5**,
/// que es lo que tiene que pasar. El cielo es una rampa pura y satura el alcance de
/// la medida. Y en el dibujo con volumen hay de las dos cosas, con un valle justo
/// aquí: sus cantos de tinta miden 0 y 1, y las costuras de sombreado de la barriga y
/// el morro, 7 y 15.
///
/// No es «un píxel», que es lo que decía el criterio antes de medirlo: un JPEG subido
/// de escala no tiene cantos de un píxel, y aun así es duro.
pub const SOFT: f64 = 5.0;

/// La blandura de una frontera.
#[derive(Clone, Debug)]
pub struct Softness {
    /// Índice de la frontera en `regions.edges`.
    pub edge: usize,
    /// Píxeles de mezcla contados a través de la frontera: la **mediana** de lo
    /// medido en cada grieta, que es lo que no se lleva por delante un extremo.
    pub width: f64,
    /// Grietas medidas, que es la longitud de la frontera en píxeles.
    pub cracks: usize,
    /// Los colores de las dos caras, en el orden `left`, `right`.
    pub colors: (Rgba, Rgba),
    /// Distancia en Oklab entre esos dos colores.
    pub jump: f64,
}

/// Mide todas las fronteras interiores de una segmentación.
///
/// Se salta las que dan al exterior: sin dos caras no hay dos colores entre los que
/// mezclar, y el borde de la silueta lo puso el umbral de alfa.
pub fn measure(
    regions: &Regions,
    clustering: &Clustering,
    img: &RgbaImage,
    tolerance: f64,
) -> Vec<Softness> {
    let lab: Vec<Oklab> = clustering
        .clusters
        .iter()
        .map(|c| Oklab::from_rgba(c.color))
        .collect();
    let (w, h) = (img.width() as i32, img.height() as i32);

    let mut out = Vec::new();
    for (id, edge) in regions.edges.iter().enumerate() {
        let Some(right) = edge.right else { continue };
        let (a, b) = (lab[edge.left], lab[right]);
        let mut anchos: Vec<usize> = Vec::with_capacity(edge.points.len());

        for pair in edge.points.windows(2) {
            let (p, q) = (pair[0], pair[1]);
            // La grieta entre dos vértices consecutivos es vertical u horizontal, y
            // la normal es el otro eje. El píxel de cada lado es el que toca la
            // grieta, así que se cuenta hacia fuera desde ahí.
            let (dentro, fuera, paso) = if p.0 == q.0 {
                let y = p.1.min(q.1);
                ((p.0 - 1, y), (p.0, y), (-1, 0))
            } else if p.1 == q.1 {
                let x = p.0.min(q.0);
                ((x, p.1 - 1), (x, p.1), (0, -1))
            } else {
                continue;
            };

            let mut ancho = 0;
            for (desde, sentido) in [(dentro, paso), (fuera, (-paso.0, -paso.1))] {
                for k in 0..REACH {
                    let at = (
                        desde.0 + sentido.0 * k as i32,
                        desde.1 + sentido.1 * k as i32,
                    );
                    if at.0 < 0 || at.1 < 0 || at.0 >= w || at.1 >= h {
                        break;
                    }
                    let px = img.get_pixel(at.0 as u32, at.1 as u32).0;
                    let color = Oklab::from_rgba(Rgba::new(px[0], px[1], px[2], px[3]));
                    match speckle::mixture(color, a, b) {
                        Some(mix)
                            if mix.gap <= CEILING * tolerance
                                && mix.t > MARGIN
                                && mix.t < 1.0 - MARGIN =>
                        {
                            ancho += 1;
                        }
                        _ => break,
                    }
                }
            }
            anchos.push(ancho);
        }

        if anchos.is_empty() {
            continue;
        }
        anchos.sort_unstable();
        out.push(Softness {
            edge: id,
            width: anchos[anchos.len() / 2] as f64,
            cracks: anchos.len(),
            colors: (
                clustering.clusters[edge.left].color,
                clustering.clusters[right].color,
            ),
            jump: a.distance(&b),
        });
    }
    out
}

/// Qué fronteras son blandas, indexado por arista, que es como lo pide
/// [`crate::ramp`].
pub fn soft_edges(measures: &[Softness], edges: usize) -> Vec<bool> {
    let mut out = vec![false; edges];
    for m in measures {
        out[m.edge] = m.width >= SOFT;
    }
    out
}
