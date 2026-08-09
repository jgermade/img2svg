//! Segmentación por clustering: de una imagen cualquiera a regiones de color
//! casi uniforme.
//!
//! Es el otro eje de segmentación, en paralelo al de rejilla ([`crate::segment`])
//! y para lo que ese no sabe hacer: una foto no está sobre una cuadrícula, no
//! tiene una paleta discreta y no se puede reducir a un píxel por celda.
//!
//! # Por qué no vale el camino de la rejilla
//!
//! [`crate::segment::from_pixel_map`] recorre los colores y por cada uno arma
//! una máscara de toda la imagen para pasarla por [`crate::trace::components`],
//! que es un relleno por inundación píxel a píxel. Con 40 colores sobre una
//! rejilla de 80x126 eso es gratis. Con 200 colores sobre 4 Mpx son 800 millones
//! de escrituras de máscara y 200 inundaciones sobre la imagen entera: no es que
//! vaya lento, es que no termina.
//!
//! # El orden de las tres etapas
//!
//! 1. **Cuantizar** cada píxel a `2^bits` niveles por canal
//!    ([`Rgba::quantize`]). Cuesta lo mismo por píxel sea la imagen que sea y
//!    deja el ruido del último bit fuera de la ecuación.
//! 2. **Construir la paleta** agrupando los colores distintos por cercanía en
//!    Oklab, del más frecuente al menos. Cada color queda asignado a un
//!    representante a menos de `tolerance` de él.
//! 3. **Etiquetar las componentes conexas** de igual representante.
//!
//! Que la paleta se decida *antes* de recorrer la imagen es lo que da la
//! garantía que importa: **ningún píxel queda a más de `tolerance` del color con
//! el que se va a pintar**. Un clustering que fuese fundiendo regiones vecinas
//! mientras avanza no puede prometer eso —cada fusión mueve el color del grupo, y
//! en un degradado suave la cadena de fusiones se lleva por delante todo el
//! cielo, que acaba siendo una sola región de un color plano que no se parece a
//! ninguno de sus extremos—. Con la paleta fija de antemano el error está acotado
//! por construcción y no depende de por dónde se haya empezado a recorrer.
//!
//! # Por qué por tramos y no por píxel
//!
//! La tercera etapa va por **tramos** —secuencias horizontales de igual
//! representante— y no píxel a píxel. Un relleno por inundación sobre 4 millones
//! de píxeles son millones de apilamientos sin localidad ninguna; una foto se
//! reduce a bastantes menos tramos que píxeles, y unir los de dos filas
//! contiguas es entonces un recorrido lineal de dos punteros con conjuntos
//! disjuntos.

use std::collections::HashMap;

use image::RgbaImage;

use crate::color::{Oklab, Rgba};

/// Etiqueta de un píxel que no pertenece a ninguna región por ser transparente.
///
/// Un `Option<u32>` costaría el doble de memoria, y sobre 4 Mpx son 16 MB de más
/// para distinguir un caso que ya tiene un valor imposible libre.
pub const NONE: u32 = u32::MAX;

/// Opciones de la segmentación por clustering.
///
/// Nada que ver con las de rejilla, que hablan de celdas y de damero: son dos
/// interpretaciones distintas de qué es la imagen.
#[derive(Clone, Debug)]
pub struct ClusterOptions {
    /// Bits por canal a los que se recorta el color antes de agrupar.
    pub color_precision: u8,
    /// Distancia máxima en Oklab entre un color y su representante en la paleta.
    /// Ver [`Oklab::distance`] para la escala: `1.0` es de negro a blanco.
    pub tolerance: f64,
    /// Alfa mínimo para considerar visible un píxel.
    pub alpha_threshold: u8,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        ClusterOptions {
            color_precision: 5,
            tolerance: 0.045,
            alpha_threshold: 128,
        }
    }
}

/// Una región conexa de un mismo color de la paleta.
#[derive(Clone, Debug)]
pub struct Cluster {
    pub color: Rgba,
    /// Píxeles que ocupa. El filtrado de motas se apoya en esto.
    pub area: usize,
}

/// La imagen repartida en regiones.
#[derive(Clone, Debug)]
pub struct Clustering {
    pub width: usize,
    pub height: usize,
    /// Región de cada píxel, en orden de filas, o [`NONE`] si es transparente.
    pub labels: Vec<u32>,
    /// Las regiones, **en orden de emisión**: los colores más presentes primero
    /// y las regiones de un mismo color seguidas, que es lo que espera
    /// [`crate::svg::render`] para envolverlas en un solo `<g>`. Dentro de un
    /// color van por posición de su primer píxel.
    pub clusters: Vec<Cluster>,
    /// Entradas de la paleta, que no tiene por qué coincidir con el número de
    /// regiones: un color suele aparecer en varias partes de la imagen.
    pub colors: usize,
}

/// Segmenta una imagen ya decodificada.
pub fn from_image(img: &RgbaImage, options: &ClusterOptions) -> Clustering {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let palette = Palette::build(img, options);

    let mut labels = vec![NONE; w * h];
    let mut sets = Sets::default();
    // El color de cada tramo. Todos los de una región comparten el mismo, porque
    // sólo se unen tramos de igual representante.
    let mut run_color: Vec<Rgba> = Vec::new();

    // Los representantes de la fila en curso, para no repetir la búsqueda en la
    // paleta al comparar un píxel con el siguiente.
    let mut row: Vec<Option<Rgba>> = vec![None; w];
    let mut prev: Vec<Run> = Vec::new();
    let mut cur: Vec<Run> = Vec::new();
    let raw = img.as_raw();

    for y in 0..h {
        let base = y * w * 4;
        for (x, px) in raw[base..base + w * 4].chunks_exact(4).enumerate() {
            row[x] = palette.lookup(px, options);
        }

        cur.clear();
        let mut x = 0;
        while x < w {
            let Some(color) = row[x] else {
                x += 1;
                continue;
            };
            let start = x;
            while x + 1 < w && row[x + 1] == Some(color) {
                x += 1;
            }
            let id = sets.push();
            run_color.push(color);
            labels[y * w + start..=y * w + x].fill(id);
            cur.push(Run { start, end: x, id });
            x += 1;
        }

        // Unión con la fila de arriba. Las dos listas van ordenadas por posición,
        // así que basta avanzar un puntero por cada una en vez de comparar todos
        // los tramos con todos.
        //
        // La vecindad es de 8, como en el camino de la rejilla
        // ([`crate::trace::components`]): dos tramos que sólo se tocan por la
        // esquina son la misma región, que es lo que uno espera de una diagonal.
        // Sale gratis, con extender una columna el solape.
        let mut i = 0;
        for r in &cur {
            while i < prev.len() && prev[i].end + 1 < r.start {
                i += 1;
            }
            let mut j = i;
            while j < prev.len() && prev[j].start <= r.end + 1 {
                if run_color[prev[j].id as usize] == run_color[r.id as usize] {
                    sets.union(prev[j].id, r.id);
                }
                j += 1;
            }
        }
        std::mem::swap(&mut prev, &mut cur);
    }

    finish(labels, w, h, &mut sets, &run_color, &palette)
}

/// Un tramo horizontal de igual representante. `end` es inclusivo.
struct Run {
    start: usize,
    end: usize,
    id: u32,
}

/// Cierra el etiquetado: resuelve cada tramo a su raíz, ordena las regiones para
/// la emisión y reescribe las etiquetas con el índice definitivo.
fn finish(
    mut labels: Vec<u32>,
    width: usize,
    height: usize,
    sets: &mut Sets,
    run_color: &[Rgba],
    palette: &Palette,
) -> Clustering {
    let n = sets.len();
    let mut area = vec![0usize; n];
    let mut first = vec![usize::MAX; n];
    for (i, &label) in labels.iter().enumerate() {
        if label == NONE {
            continue;
        }
        let root = sets.find(label) as usize;
        area[root] += 1;
        if first[root] == usize::MAX {
            first[root] = i;
        }
    }

    // Sólo las raíces con píxeles son regiones; el resto de tramos se han unido
    // a alguna de ellas.
    let mut roots: Vec<u32> = (0..n as u32).filter(|&r| area[r as usize] > 0).collect();
    roots.sort_by(|&a, &b| {
        let (ca, cb) = (run_color[a as usize], run_color[b as usize]);
        palette
            .weight(cb)
            .cmp(&palette.weight(ca))
            .then(order_key(ca).cmp(&order_key(cb)))
            .then(first[a as usize].cmp(&first[b as usize]))
    });

    let mut label_of = vec![NONE; n];
    for (id, &root) in roots.iter().enumerate() {
        label_of[root as usize] = id as u32;
    }
    for label in labels.iter_mut() {
        if *label != NONE {
            *label = label_of[sets.find(*label) as usize];
        }
    }

    let clusters = roots
        .iter()
        .map(|&r| Cluster {
            color: run_color[r as usize],
            area: area[r as usize],
        })
        .collect();

    Clustering {
        width,
        height,
        labels,
        clusters,
        colors: palette.colors(),
    }
}

/// Desempate estable entre dos colores igual de frecuentes, para que la salida
/// no dependa del orden en que los haya recorrido una tabla hash.
fn order_key(c: Rgba) -> (u8, u8, u8, u8) {
    (c.r, c.g, c.b, c.a)
}

/// La paleta: de color cuantizado a color con el que se va a pintar.
struct Palette {
    bits: u8,
    representative: HashMap<Rgba, Rgba>,
    /// Píxeles que suman todos los colores de cada representante. Fija el orden
    /// de emisión.
    weight: HashMap<Rgba, usize>,
}

impl Palette {
    /// Agrupación voraz por el más frecuente: se recorren los colores distintos
    /// de más a menos presente y cada uno se queda con el representante más
    /// cercano que esté dentro de `tolerance`, o funda uno nuevo si no hay
    /// ninguno. Es la misma idea que [`crate::color::build_palette`] usa en el
    /// camino de la rejilla, con dos diferencias que piden código propio: la
    /// distancia es la de Oklab, y la conversión de cada color se saca fuera del
    /// bucle —que es de colores por entradas— en vez de repetirla en cada
    /// comparación.
    fn build(img: &RgbaImage, options: &ClusterOptions) -> Self {
        let bits = options.color_precision;
        let mut counts: HashMap<Rgba, usize> = HashMap::new();
        for px in img.as_raw().chunks_exact(4) {
            if px[3] < options.alpha_threshold {
                continue;
            }
            *counts
                .entry(Rgba::new(px[0], px[1], px[2], px[3]).quantize(bits))
                .or_insert(0) += 1;
        }

        let mut distinct: Vec<(Rgba, usize)> = counts.into_iter().collect();
        distinct.sort_by(|a, b| b.1.cmp(&a.1).then(order_key(a.0).cmp(&order_key(b.0))));

        let mut entries: Vec<(Rgba, Oklab)> = Vec::new();
        let mut representative = HashMap::with_capacity(distinct.len());
        let mut weight: HashMap<Rgba, usize> = HashMap::new();

        for (color, count) in distinct {
            let lab = Oklab::from(color);
            let near = entries
                .iter()
                .map(|&(entry, entry_lab)| (entry, lab.distance(&entry_lab)))
                .filter(|&(_, d)| d <= options.tolerance)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(entry, _)| entry);

            let entry = match near {
                Some(entry) => entry,
                None => {
                    entries.push((color, lab));
                    color
                }
            };
            representative.insert(color, entry);
            *weight.entry(entry).or_insert(0) += count;
        }

        Palette {
            bits,
            representative,
            weight,
        }
    }

    /// El representante de un píxel crudo, o `None` si no es visible.
    fn lookup(&self, px: &[u8], options: &ClusterOptions) -> Option<Rgba> {
        if px[3] < options.alpha_threshold {
            return None;
        }
        let quantized = Rgba::new(px[0], px[1], px[2], px[3]).quantize(self.bits);
        // Está siempre: la paleta se construyó sobre estos mismos píxeles, con
        // esta misma cuantización y este mismo umbral de alfa.
        Some(self.representative[&quantized])
    }

    fn weight(&self, entry: Rgba) -> usize {
        self.weight.get(&entry).copied().unwrap_or(0)
    }

    fn colors(&self) -> usize {
        self.weight.len()
    }
}

/// Conjuntos disjuntos sobre los tramos, con unión por tamaño y compresión de
/// caminos.
#[derive(Default)]
struct Sets {
    parent: Vec<u32>,
    size: Vec<u32>,
}

impl Sets {
    fn len(&self) -> usize {
        self.parent.len()
    }

    fn push(&mut self) -> u32 {
        let id = self.parent.len() as u32;
        self.parent.push(id);
        self.size.push(1);
        id
    }

    fn find(&mut self, mut x: u32) -> u32 {
        while self.parent[x as usize] != x {
            let parent = self.parent[x as usize];
            // Compresión a medias: cada nodo pasa a colgar de su abuelo, que
            // aplana el árbol igual de bien y sin segunda pasada.
            self.parent[x as usize] = self.parent[parent as usize];
            x = self.parent[x as usize];
        }
        x
    }

    fn union(&mut self, a: u32, b: u32) {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b {
            return;
        }
        if self.size[a as usize] < self.size[b as usize] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parent[b as usize] = a;
        self.size[a as usize] += self.size[b as usize];
    }
}
