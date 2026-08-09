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

use crate::background;
use crate::color::{Oklab, Rgba};
use crate::speckle;

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
    /// Área hasta la que una región se funde con una vecina. `0` no funde nada.
    /// Ver [`crate::speckle`].
    pub filter_speckle: usize,
    /// Grosor por debajo del cual una región se funde con una vecina, aunque su
    /// área sea grande. `0` no funde nada. Ver [`crate::speckle::thickness`].
    ///
    /// El valor por defecto, `1.0`, es exactamente el grosor de un bloque de
    /// `2x2`, así que se lleva **todo** lo que mida un píxel de ancho: los
    /// rebordes de antialias, que es para lo que está, y también una línea fina
    /// que sí fuese dibujo. En una foto los primeros son órdenes de magnitud más
    /// numerosos; en un dibujo de línea fina, esto va a `0`.
    pub min_thickness: f64,
    /// Hasta cuánta diferencia **de luz** se funden dos colores que por lo demás
    /// son el mismo, aunque pasen de `tolerance`. `0` no relaja nada.
    ///
    /// Es la respuesta al degradado. Un SVG no tiene degradado por región, así que
    /// una rampa suave sale por fuerza a escalones, y lo que se elige aquí es lo
    /// anchos que son. Subir `tolerance` también los ensancharía, pero de paso
    /// fundiría tonos distintos que están cerca; esto ensancha **sólo** a lo largo
    /// del eje de la luz y deja el tono donde estaba. Ver
    /// [`Oklab::chroma_distance`].
    pub gradient_step: f64,
    /// Entradas máximas de la paleta. `0` no pone tope.
    ///
    /// Con tope, los colores que sobran van a la entrada más cercana **sin límite
    /// de distancia**: el orden de la agrupación es por frecuencia, así que las
    /// entradas que se quedan son las de los colores más presentes.
    pub max_colors: usize,
    /// Paleta impuesta. Si no está vacía es exactamente la paleta que se usa: no
    /// se crea ninguna entrada más y cada color va a la más cercana, también sin
    /// límite de distancia.
    pub palette: Vec<Rgba>,
    /// Vaciar el fondo liso y recortar a lo que queda dibujado.
    /// Ver [`crate::background::remove_clustered`].
    pub remove_background: bool,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        ClusterOptions {
            color_precision: 5,
            tolerance: 0.045,
            alpha_threshold: 128,
            filter_speckle: 4,
            min_thickness: 1.0,
            // Apagado: bandear es una decisión de estilo, y de las que se toman
            // mirando el resultado. Lo que hace por defecto la tolerancia ya
            // reparte una rampa en escalones; esto los ensancha a propósito.
            gradient_step: 0.0,
            max_colors: 0,
            palette: Vec::new(),
            remove_background: false,
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
    /// El color de fondo retirado, si se pidió quitarlo y había uno.
    pub background: Option<Rgba>,
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

    let mut clustering = finish(labels, w, h, &mut sets, &run_color);

    // Las motas primero y el fondo después, y no al revés: una mota que estaba
    // sobre el fondo se funde con él y desaparece con él. Quitando el fondo antes,
    // esa misma mota se queda rodeada de transparencia, sin vecina en la que
    // fundirse, y sobrevive como un punto flotando en el vacío.
    speckle::filter(
        &mut clustering,
        options.filter_speckle,
        options.min_thickness,
    );
    if options.remove_background {
        background::remove_clustered(&mut clustering);
        background::trim_clustered(&mut clustering);
    }
    clustering
}

/// Un tramo horizontal de igual representante. `end` es inclusivo.
struct Run {
    start: usize,
    end: usize,
    id: u32,
}

/// Cierra el etiquetado: resuelve cada tramo a su raíz y arma el resultado.
fn finish(
    labels: Vec<u32>,
    width: usize,
    height: usize,
    sets: &mut Sets,
    run_color: &[Rgba],
) -> Clustering {
    let root_of: Vec<u32> = (0..sets.len() as u32).map(|id| sets.find(id)).collect();
    gather(labels, width, height, &root_of, run_color)
}

/// Ordena las regiones para la emisión y reescribe las etiquetas con el índice
/// definitivo.
///
/// Vive aquí y lo usan dos etapas —el etiquetado y el filtrado de motas— porque el
/// orden que produce no es cosmético: [`crate::svg::render`] recorre tramos
/// contiguos de igual color y abre un `<g>` por cada tramo, así que si las
/// regiones de un color dejan de ir seguidas, el documento pasa de un grupo por
/// color a un grupo por región. Con la regla en un solo sitio, eso no se puede
/// romper a medias.
///
/// - `root_of` lleva de cada etiqueta actual a su raíz, que es lo que agrupa.
/// - `color_of` da el color de cada raíz, indexado igual que `root_of`.
pub(crate) fn gather(
    mut labels: Vec<u32>,
    width: usize,
    height: usize,
    root_of: &[u32],
    color_of: &[Rgba],
) -> Clustering {
    let n = root_of.len();
    let mut area = vec![0usize; n];
    let mut first = vec![usize::MAX; n];
    for (i, &label) in labels.iter().enumerate() {
        if label == NONE {
            continue;
        }
        let root = root_of[label as usize] as usize;
        area[root] += 1;
        if first[root] == usize::MAX {
            first[root] = i;
        }
    }

    // Sólo las raíces con píxeles son regiones; lo demás se ha unido a alguna.
    let mut roots: Vec<u32> = (0..n as u32).filter(|&r| area[r as usize] > 0).collect();

    // Los colores más presentes primero, para que los paths grandes queden al
    // fondo del documento. El peso de un color es lo que suman sus regiones.
    let mut weight: HashMap<Rgba, usize> = HashMap::new();
    for &r in &roots {
        *weight.entry(color_of[r as usize]).or_insert(0) += area[r as usize];
    }
    roots.sort_by(|&a, &b| {
        let (ca, cb) = (color_of[a as usize], color_of[b as usize]);
        weight[&cb]
            .cmp(&weight[&ca])
            .then(order_key(ca).cmp(&order_key(cb)))
            .then(first[a as usize].cmp(&first[b as usize]))
    });

    let mut label_of = vec![NONE; n];
    for (id, &root) in roots.iter().enumerate() {
        label_of[root as usize] = id as u32;
    }
    for label in labels.iter_mut() {
        if *label != NONE {
            *label = label_of[root_of[*label as usize] as usize];
        }
    }

    let clusters = roots
        .iter()
        .map(|&r| Cluster {
            color: color_of[r as usize],
            area: area[r as usize],
        })
        .collect();

    Clustering {
        width,
        height,
        labels,
        clusters,
        colors: weight.len(),
        background: None,
    }
}

/// Desempate estable entre dos colores igual de frecuentes, para que la salida
/// no dependa del orden en que los haya recorrido una tabla hash.
pub(crate) fn order_key(c: Rgba) -> (u8, u8, u8, u8) {
    (c.r, c.g, c.b, c.a)
}

/// La paleta: de color cuantizado a color con el que se va a pintar.
struct Palette {
    bits: u8,
    representative: HashMap<Rgba, Rgba>,
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

        // Con paleta impuesta las entradas están dadas y no se crea ninguna más;
        // si no, se van fundando por el camino.
        let fixed = !options.palette.is_empty();
        let mut entries: Vec<(Rgba, Oklab)> = options
            .palette
            .iter()
            .map(|&color| (color, Oklab::from(color)))
            .collect();
        let mut representative = HashMap::with_capacity(distinct.len());

        for (color, _) in distinct {
            let lab = Oklab::from(color);
            // Se puede fundar una entrada nueva mientras no haya paleta impuesta
            // ni se haya llegado al tope.
            let can_add = !fixed && (options.max_colors == 0 || entries.len() < options.max_colors);
            let near = entries
                .iter()
                .map(|&(entry, entry_lab)| (entry, lab.distance(&entry_lab), entry_lab))
                .filter(|&(_, d, entry_lab)| {
                    // Dentro de la tolerancia, o sólo más lejos en luz de lo que
                    // `gradient_step` permite ensanchar la banda.
                    !can_add
                        || d <= options.tolerance
                        || (lab.chroma_distance(&entry_lab) <= options.tolerance
                            && lab.lightness_gap(&entry_lab) <= options.gradient_step)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(entry, _, _)| entry);

            let entry = match near {
                Some(entry) => entry,
                None => {
                    // Sin candidata y sin poder fundar: sólo pasa si la paleta
                    // impuesta está vacía, y entonces `fixed` es falso.
                    debug_assert!(can_add, "ningún destino para {color:?}");
                    entries.push((color, lab));
                    color
                }
            };
            representative.insert(color, entry);
        }

        Palette {
            bits,
            representative,
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
