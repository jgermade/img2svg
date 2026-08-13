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
//! # El orden de las cuatro etapas
//!
//! 1. **Cuantizar** cada píxel a `2^bits` niveles por canal
//!    ([`Rgba::quantize`]). Cuesta lo mismo por píxel sea la imagen que sea y
//!    deja el ruido del último bit fuera de la ecuación.
//! 2. **Construir la paleta** agrupando los colores distintos por cercanía en
//!    Oklab, del más frecuente al menos. Cada color queda asignado a un
//!    representante a menos de `tolerance` de él.
//! 3. **Regularizar** la asignación mirando el vecindario de cada píxel
//!    ([`crate::smooth`]), porque los dos pasos anteriores tratan cada píxel
//!    como si estuviera solo en el mundo y el ruido del original decide por
//!    ellos en cuanto se acerca a la tolerancia.
//! 4. **Etiquetar las componentes conexas** de igual representante.
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
//! Esa cota es de la paleta con todo lo demás apagado, y las etapas que **funden**
//! son las que la gastan, cada una con su precio dicho:
//!
//! | etapa | qué puede empeorar el color de un píxel |
//! | --- | --- |
//! | `min_color_share` | hasta [`SNAP_CEILING`] veces la tolerancia, y de ellas [`SNAP_HUE`] en tono |
//! | `gradient_step` | lo que diga, y sólo a lo largo del eje de la luz |
//! | [`crate::smooth`] | hasta [`crate::smooth::CEILING`] veces la tolerancia, o hasta donde ya estuviera; y sólo a un color que ya se pintaba pegado a él |
//! | [`crate::speckle`] | sin cota: una mota se va con su vecina, sea del color que sea |
//!
//! `min_color_share` y el suavizado componen sin aflojarse: `4x` sigue siendo la
//! cota con los dos puestos, porque el suavizado nunca empeora un píxel más allá
//! de donde ya estaba. Así que **con los valores por defecto ningún píxel se pinta
//! a más de `SNAP_CEILING * tolerance` de su color, ni a más de `SNAP_HUE *
//! tolerance` de él en tono**, más lo que `gradient_step` permita **en luz**, que es
//! un eje aparte y por eso se dice aparte. Las dos cotas del arrastre hacen falta
//! porque una sola sobre la distancia total deja pasar un cambio de color entero:
//! ver [`SNAP_HUE`], que es un defecto medido y no una precaución. Apagando las cuatro —`min_color_share: 0`, `smoothing: 0`,
//! `filter_speckle: 0`, `min_thickness: 0`, `gradient_step: 0`— vale la cota
//! estrecha tal cual está escrita arriba. `tests/cluster.rs` comprueba las dos.
//!
//! Fuera de aquí queda una etapa más que también gasta: [`crate::ramp`] añade
//! hasta [`crate::ramp::CEILING`] tolerancias **encima** de lo que la región ya
//! traía. No entra en la tabla porque no es de esta fase —la segmentación acaba
//! con cada región de un color, y el degradado se decide al escribir el
//! documento—, pero sí en la cuenta de quien mire un píxel del resultado.
//!
//! # Por qué por tramos y no por píxel
//!
//! La cuarta etapa va por **tramos** —secuencias horizontales de igual
//! representante— y no píxel a píxel. Un relleno por inundación sobre 4 millones
//! de píxeles son millones de apilamientos sin localidad ninguna; una foto se
//! reduce a bastantes menos tramos que píxeles, y unir los de dos filas
//! contiguas es entonces un recorrido lineal de dos punteros con conjuntos
//! disjuntos.

use std::collections::HashMap;

use image::RgbaImage;

use crate::background;
use crate::color::{Oklab, Rgba};
use crate::{smooth, speckle};
use crate::{Progress, Stage};

/// Etiqueta de un píxel que no pertenece a ninguna región por ser transparente.
///
/// Un `Option<u32>` costaría el doble de memoria, y sobre 4 Mpx son 16 MB de más
/// para distinguir un caso que ya tiene un valor imposible libre.
pub const NONE: u32 = u32::MAX;

/// Hasta dónde puede arrastrarse un color que no se ha ganado entrada propia,
/// en múltiplos de `tolerance`. Más lejos que esto funda entrada por raro que
/// sea.
///
/// Sin él, `min_color_share` sale sin cota ninguna: el criterio es *píxeles por
/// distancia*, así que a un color con pocos píxeles se le puede pedir una
/// distancia enorme, y en una imagen de 5 Mpx un lunar saturado de 30x30 se
/// quedaría sin su color. Con él, lo que está lejos de todo siempre se lleva
/// entrada —que es el caso que hay que proteger— y lo que está pegado a un color
/// que ya existe se absorbe —que es el ruido de los bordes.
///
/// Dónde ponerlo, medido en entradas de la paleta:
///
/// | techo | cover.jpg | Sonic1.png |
/// | --- | --- | --- |
/// | 2x | 30 | 37 |
/// | 3x | 20 | 24 |
/// | **4x** | **20** | **21** |
/// | 8x | 20 | 20 |
/// | sin techo | 20 | 18 |
///
/// A `4x` está prácticamente todo lo que hay que ganar, y a cambio queda una cota
/// que se puede escribir: **ningún píxel se pinta a más de `4 * tolerance` de su
/// color**.
pub const SNAP_CEILING: f64 = 4.0;

/// Y cuánto puede empeorar **de tono**, en múltiplos de `tolerance`.
///
/// El techo de arriba mira la distancia entera, y por eso deja pasar un cambio de
/// tono: la entrada más cercana a un color puede estar cerca por la luz y ser de otro
/// color. Medido en la portada de un disco, es de donde sale su defecto más visible.
/// El canto de una letra blanca sobre el panel verde es una rampa de seis píxeles
/// —doce a la escala de trabajo— cuyos píxeles de en medio son mezcla limpia de sus
/// dos lados; ninguno se gana entrada propia, y el más cercano a `(186,209,183)` no
/// es un verde claro, que no está en la paleta, sino un tono de piel a 1,4
/// tolerancias. Así que la rampa entera se pinta de beige y el rótulo sale con un
/// reborde ocre que no está en la imagen.
///
/// Una tolerancia, que es lo que la paleta promete de por sí: absorber puede costar
/// luz —hasta [`SNAP_CEILING`]— y no puede costar tono. Lo que el atajo tenía que
/// tragarse sigue tragándose, porque el *ringing* alrededor de un trazo negro
/// comparte tono con el negro que lo absorbe; lo que deja de poder es cambiar de
/// color.
///
/// Es lo simétrico de `gradient_step`, que da holgura en luz a propósito y ninguna en
/// tono. Las dos salen de tener los dos ejes separados, que es para lo que está Oklab.
pub const SNAP_HUE: f64 = 2.0;

/// Opciones de la segmentación por clustering.
///
/// Nada que ver con las de rejilla, que hablan de celdas y de damero: son dos
/// interpretaciones distintas de qué es la imagen.
#[derive(Clone, Debug)]
pub struct ClusterOptions {
    /// El rasgo más pequeño que sobrevive, en tantos por mil del lado largo.
    /// `None` usa [`crate::resample::SIMPLIFY`] y `Some(0.0)` no reescala nada.
    ///
    /// No es una opción de esta etapa sino de la anterior: dice a qué resolución
    /// se segmenta, y de ahí sale lo que significan todas las demás, que están en
    /// píxeles absolutos. Ver [`crate::resample`], que es donde está el argumento.
    pub simplify: Option<f64>,
    /// Bits por canal a los que se recorta el color antes de agrupar.
    pub color_precision: u8,
    /// Distancia máxima en Oklab entre un color y su representante en la paleta.
    /// Ver [`Oklab::distance`] para la escala: `1.0` es de negro a blanco.
    pub tolerance: f64,
    /// Colocar cada vértice del contorno donde la imagen dice que está el borde,
    /// en vez de en la retícula entera. Ver [`crate::subpixel`].
    ///
    /// Sólo lo leen los ajustes que pueden dibujar fuera de la retícula: `pixel`
    /// es la escalera literal por definición y lo ignora.
    pub subpixel: bool,
    /// Pasadas de regularización espacial sobre la asignación de paleta. `0` deja
    /// cada píxel donde lo puso la paleta. Ver [`crate::smooth`].
    ///
    /// Es un número de pasadas y no un umbral porque lo que se raspa es grosor:
    /// una mota compacta de ruido se erosiona una corona por pasada, mientras que
    /// un detalle que sí es dibujo no se mueve por muchas que se den.
    pub smoothing: usize,
    /// Cuánto puede moverse un vértice del contorno para quitarle el temblor de
    /// la escalera, en píxeles de trabajo. `0` lo deja como sale del trazado.
    ///
    /// No es un suavizado: el tope es lo que garantiza que un vértice no acabe
    /// lejos de donde la imagen dice que está el borde, y las esquinas no se
    /// tocan. Ver [`crate::wobble`].
    pub relax: f64,
    /// Buscar grupos de bandas que sean una rampa y fundir cada uno en una sola
    /// figura con `<linearGradient>`. Ver [`crate::ramp`].
    ///
    /// Es lo único de aquí que baja las tres cifras a la vez —colores, figuras y
    /// anclas—, porque las fronteras entre bandas de un degradado son contornos
    /// que no dibujan nada: sólo marcan por dónde cruzó la rampa un umbral.
    pub ramps: bool,
    /// Alfa mínimo para considerar visible un píxel.
    pub alpha_threshold: u8,
    /// Área hasta la que una región se funde con una vecina. `0` no funde nada.
    /// Ver [`crate::speckle`].
    pub filter_speckle: usize,
    /// Grosor por debajo del cual una región **puede** fundirse con una vecina,
    /// aunque su área sea grande. `0` no funde nada. Ver
    /// [`crate::speckle::thickness`].
    ///
    /// Es un candidato, no una condena: de las regiones delgadas sólo se funden
    /// las que son una **mezcla** de sus dos vecinas, que es lo que separa el
    /// reborde de antialias de un trazo de tinta, porque los dos miden lo mismo.
    /// Ver [`crate::speckle`], que es donde está el argumento y la medida.
    ///
    /// Por defecto, el grosor del rasgo más pequeño que la escala de trabajo
    /// promete conservar: nada más delgado que eso *y* además explicable como
    /// mezcla tiene por qué llegar al documento.
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
    /// Lo que un color tiene que **valer** para llevarse una entrada propia, como
    /// fracción de la imagen. `0` se la da a cualquiera, que era lo de antes.
    ///
    /// Sin esto la paleta crece con el ruido: la agrupación va por frecuencia,
    /// pero la frecuencia sólo *ordena*, nunca frena, así que un color que
    /// aparece treinta veces en toda la imagen funda entrada igual que el fondo.
    /// Medido en la portada de un disco —un dibujo de cuatro planos de color y
    /// trazo negro—, la paleta salía con 65 entradas de las que **42 pintaban
    /// menos del 0,2% cada una y el 1,45% entre todas**: el *ringing* del JPEG
    /// alrededor de los trazos, una entrada por escalón.
    ///
    /// El criterio no es el recuento a secas, porque no distingue dos cosas que
    /// se parecen en el recuento y en nada más: cuarenta píxeles de *ringing*
    /// repartidos por los bordes, y un lunar rojo de cuarenta píxeles. Lo que las
    /// separa es **cuánto error se ahorra** teniendo la entrada: el *ringing* está
    /// pegado a un color que ya existe y no ahorra casi nada; el lunar está lejos
    /// de todo y ahorra mucho. Así que una entrada se gana el sitio cuando
    ///
    /// ```text
    ///     píxeles del color * distancia a la entrada más cercana
    ///         >= min_color_share * píxeles visibles * tolerance
    /// ```
    ///
    /// que se lee: *la entrada nueva tiene que quitar al menos tanto error como
    /// el de tener esta fracción de la imagen desviada una tolerancia*. Un color
    /// que está al doble de la tolerancia necesita la mitad de píxeles; uno que
    /// está a diez veces, la décima parte.
    pub min_color_share: f64,
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
            simplify: None,
            color_precision: 5,
            tolerance: 0.045,
            subpixel: true,
            relax: 0.75,
            smoothing: 2,
            ramps: true,
            alpha_threshold: 128,
            // Los dos umbrales de motas salen del mismo sitio, que es el mando de
            // simplificar: el área y el grosor del rasgo más pequeño que la escala
            // de trabajo promete conservar, `resample::FEATURE` de lado. Un `4`
            // absoluto era lo justo en un dibujo de 300 px y nada en uno de 5 Mpx,
            // y ese desajuste es lo que la escala de trabajo quita de en medio.
            filter_speckle: (crate::resample::FEATURE * crate::resample::FEATURE) as usize,
            min_thickness: crate::resample::FEATURE,
            // Poco, pero no nada, y el motivo no es bandear: es la **tinta
            // partida**. Un trazo de dos píxeles nunca llega a tinta plena, así
            // que el trazo entero es una mezcla y uno fino sale más claro que uno
            // gordo; la paleta los separa y el mismo trazo aparece a trozos en dos
            // tonos, casi negro y gris oscuro. Son el mismo color con distinta
            // dilución, que es exactamente lo que esto funde.
            //
            // Medido, en la portada y en el aerógrafo:
            //
            // | escalón | portada | aerógrafo | qué se ve |
            // | --- | --- | --- | --- |
            // | 0 | 19 colores, 662 paths, 119 KB | 21, 131, 40 KB | trazos a trozos de dos tonos |
            // | **0,05** | **16, 450, 94 KB** | **19, 137, 40 KB** | el pelo sale macizo; el sombreado sigue |
            // | 0,10 | 14, 497, 116 KB | 13, 97, 33 KB | más paths en la portada: bandas nuevas |
            // | 0,15 | 13, 283, 90 KB | | fronteras moteadas, y aplana el volumen |
            //
            // Y no estorba a los degradados, que era la duda: el cielo sintético
            // sigue saliendo de una pieza con un solo `<linearGradient>`.
            gradient_step: 0.05,
            // Medido sobre tres imágenes que no se parecen en nada —la portada de
            // un JPEG con trazo, un aerógrafo escaneado de 5 Mpx y un pixel art
            // reescalado—, y las tres coinciden en el mismo sitio: es donde la
            // paleta deja de tener entradas que no pintan nada y todavía no ha
            // empezado a perder ninguna que sí.
            //
            // | imagen | antes | 0,001 | **0,002** | 0,005 |
            // | --- | --- | --- | --- | --- |
            // | cover.jpg | 68 | 27 | **20** | 12 |
            // | Sonic1.png | 86 | 24 | **18** | 16 |
            // | pixel art | 80 | 18 | **16** | 13 |
            //
            // A 0,005 la portada baja a doce y sigue estando bien —es un cartel de
            // cuatro planos—, pero eso es una decisión de estilo y ésta es la que
            // se toma sin mirar la imagen.
            min_color_share: 0.002,
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
    from_image_with(img, options, &mut Progress::default())
}

/// Lo mismo, avisando del avance.
///
/// Las dos pasadas por la imagen —contar colores y etiquetar— son dos tercios
/// del tiempo de una conversión de foto, así que son las dos que avisan por
/// fila. Lo de después va por fases enteras: son tres saltos, pero cortos.
pub fn from_image_with(
    img: &RgbaImage,
    options: &ClusterOptions,
    progress: &mut Progress,
) -> Clustering {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let palette = Palette::build(img, options, progress);
    let raw = img.as_raw();

    // La entrada de paleta de cada píxel, antes de mirar a nadie. Se materializa
    // entera y no fila a fila como antes porque la regularización necesita ver el
    // vecindario, y eso incluye la fila de abajo.
    progress.stage(Stage::Regions);
    let mut field = vec![NONE; w * h];
    for y in 0..h {
        progress.at(y, h);
        let base = y * w * 4;
        for (x, px) in raw[base..base + w * 4].chunks_exact(4).enumerate() {
            if let Some((entry, _)) = palette.lookup(px, options) {
                field[y * w + x] = entry;
            }
        }
    }

    progress.stage(Stage::Smoothing);
    smooth::regularize(
        &mut field,
        w,
        h,
        |i| palette.lab_at(&raw[i * 4..i * 4 + 4]),
        &palette.entry_lab,
        smooth::beta(options.tolerance),
        options.tolerance,
        options.smoothing,
    );

    progress.stage(Stage::Runs);
    let mut labels = vec![NONE; w * h];
    let mut sets = Sets::default();
    // El color de cada tramo. Todos los de una región comparten el mismo, porque
    // sólo se unen tramos de igual representante.
    let mut run_color: Vec<Rgba> = Vec::new();

    let mut prev: Vec<Run> = Vec::new();
    let mut cur: Vec<Run> = Vec::new();

    for y in 0..h {
        progress.at(y, h);

        cur.clear();
        let mut x = 0;
        while x < w {
            let entry = field[y * w + x];
            if entry == NONE {
                x += 1;
                continue;
            }
            let start = x;
            while x + 1 < w && field[y * w + x + 1] == entry {
                x += 1;
            }
            let id = sets.push();
            run_color.push(palette.entries[entry as usize]);
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
    progress.stage(Stage::Speckle);

    // Las motas primero y el fondo después, y no al revés: una mota que estaba
    // sobre el fondo se funde con él y desaparece con él. Quitando el fondo antes,
    // esa misma mota se queda rodeada de transparencia, sin vecina en la que
    // fundirse, y sobrevive como un punto flotando en el vacío.
    speckle::filter(
        &mut clustering,
        options.filter_speckle,
        options.min_thickness,
        options.tolerance,
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

/// La paleta: de color cuantizado a la entrada con la que se va a pintar.
///
/// Guarda índices y no colores porque el campo de píxeles se regulariza antes de
/// etiquetar ([`crate::smooth`]): comparar dos vecinos tiene que costar una
/// comparación de enteros, y el término de color del criterio necesita el Oklab
/// de cada entrada a mano, no recalculado por píxel.
struct Palette {
    bits: u8,
    /// De color cuantizado a (índice de su entrada, su propio Oklab). El Oklab
    /// que se guarda es el del color **cuantizado**, que es sobre el que se
    /// decidió la paleta: usar otro haría que el criterio de regularización y el
    /// de agrupación midieran cosas distintas.
    assignment: HashMap<Rgba, (u32, Oklab)>,
    /// Las entradas, en el orden en que se fundaron.
    entries: Vec<Rgba>,
    /// Su color, indexado igual. Va aparte de `entries` porque la
    /// regularización lo recorre por índice y no quiere arrastrar el `Rgba`.
    entry_lab: Vec<Oklab>,
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
    fn build(img: &RgbaImage, options: &ClusterOptions, progress: &mut Progress) -> Self {
        progress.stage(Stage::Palette);
        let bits = options.color_precision;
        let mut counts: HashMap<Rgba, usize> = HashMap::new();
        // Por filas, y no de un tirón sobre todos los píxeles, sólo para poder
        // decir por dónde va: es el 30% del tiempo de la conversión.
        let h = img.height() as usize;
        for (y, row) in img
            .as_raw()
            .chunks_exact(img.width() as usize * 4)
            .enumerate()
        {
            progress.at(y, h);
            for px in row.chunks_exact(4) {
                if px[3] < options.alpha_threshold {
                    continue;
                }
                *counts
                    .entry(Rgba::new(px[0], px[1], px[2], px[3]).quantize(bits))
                    .or_insert(0) += 1;
            }
        }

        let mut distinct: Vec<(Rgba, usize)> = counts.into_iter().collect();
        distinct.sort_by(|a, b| b.1.cmp(&a.1).then(order_key(a.0).cmp(&order_key(b.0))));
        // Los píxeles visibles, que es contra lo que se mide si un color da para
        // entrada propia. Sale de la misma cuenta y no de `w*h`: lo transparente
        // no se pinta.
        let visible: usize = distinct.iter().map(|&(_, n)| n).sum();

        // Con paleta impuesta las entradas están dadas y no se crea ninguna más;
        // si no, se van fundando por el camino.
        let fixed = !options.palette.is_empty();
        let mut entries: Vec<(Rgba, Oklab)> = options
            .palette
            .iter()
            .map(|&color| (color, Oklab::from(color)))
            .collect();
        let mut assignment = HashMap::with_capacity(distinct.len());

        // Lo que tiene que ahorrar una entrada nueva para ganarse el sitio, en
        // error de color acumulado. Ver `min_color_share`.
        let budget = options.min_color_share * visible as f64 * options.tolerance;

        for (color, count) in distinct {
            let lab = Oklab::from(color);
            // Se puede fundar una entrada nueva mientras no haya paleta impuesta
            // ni se haya llegado al tope.
            let can_add = !fixed && (options.max_colors == 0 || entries.len() < options.max_colors);
            // Dos mínimos, y hacen falta los dos por separado. `serviria` es la
            // más cercana **de las que le valen**, que es a la que se asigna; y
            // `absorbe` es la más cercana **de las que pueden tragárselo** si no se
            // gana entrada propia. No se puede sacar la segunda de la primera ni al
            // revés: con `gradient_step` la entrada más cercana puede no valer
            // mientras que otra un poco más lejos sí.
            let mut serviria: Option<(usize, f64)> = None;
            let mut absorbe: Option<(usize, f64)> = None;
            for (i, &(_, entry_lab)) in entries.iter().enumerate() {
                let d = lab.distance(&entry_lab);
                // Estrictamente menor: a igualdad se queda la primera, que es la
                // que fundó antes y por tanto la del color más presente. Y sólo se
                // mira a las que le quedan cerca en tono: el atajo puede empeorar la
                // luz de un color, no cambiarle el color. Ver [`SNAP_HUE`].
                if lab.chroma_distance(&entry_lab) <= SNAP_HUE * options.tolerance
                    && absorbe.is_none_or(|(_, best)| d < best)
                {
                    absorbe = Some((i, d));
                }
                // Dentro de la tolerancia, o sólo más lejos en luz de lo que
                // `gradient_step` permite ensanchar la banda.
                let vale = !can_add
                    || d <= options.tolerance
                    || (lab.chroma_distance(&entry_lab) <= options.tolerance
                        && lab.lightness_gap(&entry_lab) <= options.gradient_step);
                if vale && serviria.is_none_or(|(_, best)| d < best) {
                    serviria = Some((i, d));
                }
            }

            let index = match serviria {
                Some((i, _)) => i,
                // Ninguna entrada le vale. Aquí `can_add` es siempre cierto: con
                // la paleta llena el filtro de arriba deja pasar todo. Así que o
                // se gana una entrada nueva, o se va con la más cercana de su tono
                // aunque quede lejos —que es lo que hace que la paleta deje de
                // crecer con el ruido de los bordes.
                None => match absorbe {
                    Some((i, d))
                        if d <= options.tolerance * SNAP_CEILING && (count as f64) * d < budget =>
                    {
                        i
                    }
                    _ => {
                        debug_assert!(can_add, "ningún destino para {color:?}");
                        entries.push((color, lab));
                        entries.len() - 1
                    }
                },
            };
            assignment.insert(color, (index as u32, lab));
        }

        Palette {
            bits,
            assignment,
            entries: entries.iter().map(|&(color, _)| color).collect(),
            entry_lab: entries.iter().map(|&(_, lab)| lab).collect(),
        }
    }

    /// La entrada de un píxel crudo y su color, o `None` si no es visible.
    fn lookup(&self, px: &[u8], options: &ClusterOptions) -> Option<(u32, Oklab)> {
        if px[3] < options.alpha_threshold {
            return None;
        }
        Some(self.quantized(px))
    }

    /// El color de un píxel que ya se sabe visible.
    ///
    /// Existe aparte de [`Palette::lookup`] para la regularización, que ya ha
    /// mirado el alfa —está en el campo de entradas— y sólo quiere el color.
    fn lab_at(&self, px: &[u8]) -> Oklab {
        self.quantized(px).1
    }

    fn quantized(&self, px: &[u8]) -> (u32, Oklab) {
        let quantized = Rgba::new(px[0], px[1], px[2], px[3]).quantize(self.bits);
        // Está siempre: la paleta se construyó sobre estos mismos píxeles, con
        // esta misma cuantización y este mismo umbral de alfa.
        self.assignment[&quantized]
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
