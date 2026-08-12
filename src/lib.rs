//! Conversión de pixel art a SVG.
//!
//! El proceso tiene cuatro fases, cada una en su módulo: detectar la rejilla de
//! píxeles del dibujo y reducir la imagen a ella ([`grid`]), fundir los colores
//! casi idénticos ([`color`]), trazar el contorno de cada región ([`trace`]) y
//! escribir el documento ([`svg`]).
//!
//! Se parte de un búfer RGBA, que es la vía que siempre está disponible y la
//! que usa la página web. Con la característica `formats`, activa por defecto,
//! [`convert`] hace lo mismo a partir de un PNG o un JPEG ya codificados.
//!
//! ```
//! // Dos píxeles opacos arriba y dos transparentes abajo.
//! let rgba: [u8; 16] = [
//!     255, 0, 0, 255,   0, 0, 255, 255,
//!     0, 0, 0, 0,       0, 0, 0, 0,
//! ];
//! let out = img2svg::convert_rgba(2, 2, &rgba, &img2svg::Config::default()).unwrap();
//! assert_eq!(out.canvas, (2, 2));
//! assert_eq!(out.colors, 2);
//! ```

pub mod background;
#[cfg(feature = "illustration")]
pub mod boundary;
pub mod checker;
#[cfg(feature = "illustration")]
pub mod cluster;
pub mod color;
pub mod fit;
pub mod grid;
#[cfg(feature = "illustration")]
pub mod ramp;
pub mod region;
#[cfg(feature = "illustration")]
pub mod resample;
pub mod segment;
#[cfg(feature = "illustration")]
pub mod smooth;
#[cfg(feature = "illustration")]
pub mod softness;
#[cfg(feature = "illustration")]
pub mod speckle;
#[cfg(feature = "illustration")]
pub mod subpixel;
pub mod svg;
pub mod trace;
#[cfg(feature = "illustration")]
pub mod wobble;

#[cfg(feature = "wasm")]
mod wasm;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use image::RgbaImage;

use crate::color::Rgba;
use crate::grid::{Axis, PixelMap};

#[cfg(feature = "illustration")]
pub use crate::cluster::ClusterOptions;
pub use crate::fit::Fit;
pub use crate::segment::Grouping;

/// Parámetros de la conversión, en dos ejes ortogonales.
///
/// La **segmentación** decide cómo se pasa de la imagen a un conjunto de
/// regiones, y el **ajuste** cómo se pasa del contorno de una región a los datos
/// de un `<path>`. Se combinan libremente: son etapas distintas del mismo
/// proceso, no dos programas.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub segmentation: Segmentation,
    pub fit: Fit,
    /// Color de fondo opcional del SVG. No depende de ninguno de los dos ejes.
    pub background: Option<String>,
}

impl Config {
    /// Configuración de pixel art con el ajuste por defecto.
    pub fn grid(options: GridOptions) -> Self {
        Config {
            segmentation: Segmentation::Grid(options),
            ..Config::default()
        }
    }

    /// Configuración de foto con el ajuste por defecto.
    #[cfg(feature = "illustration")]
    pub fn cluster(options: ClusterOptions) -> Self {
        Config {
            segmentation: Segmentation::Cluster(options),
            ..Config::default()
        }
    }

    /// Las opciones de rejilla, para leerlas o retocarlas, o `None` si esta
    /// configuración no es de rejilla.
    pub fn grid_options(&self) -> Option<&GridOptions> {
        match &self.segmentation {
            Segmentation::Grid(options) => Some(options),
            #[cfg(feature = "illustration")]
            Segmentation::Cluster(_) => None,
        }
    }

    pub fn grid_options_mut(&mut self) -> Option<&mut GridOptions> {
        match &mut self.segmentation {
            Segmentation::Grid(options) => Some(options),
            #[cfg(feature = "illustration")]
            Segmentation::Cluster(_) => None,
        }
    }
}

/// Cómo se pasa de la imagen a un conjunto de regiones.
#[derive(Clone, Debug)]
pub enum Segmentation {
    /// Pixel art: se detecta la rejilla y se reduce la imagen a ella.
    Grid(GridOptions),
    /// Foto: se agrupan los colores en una paleta y se etiquetan las regiones
    /// conexas de cada entrada. Ver [`cluster`].
    #[cfg(feature = "illustration")]
    Cluster(ClusterOptions),
}

impl Default for Segmentation {
    fn default() -> Self {
        Segmentation::Grid(GridOptions::default())
    }
}

/// Opciones de la segmentación por rejilla.
#[derive(Clone, Debug)]
pub struct GridOptions {
    /// Tamaño de celda en píxeles reales. `None` lo detecta automáticamente.
    pub scale: Option<f64>,
    /// Desplazamiento de la rejilla. `None` usa el detectado.
    pub offset: Option<(f64, f64)>,
    /// Distancia máxima para fundir dos colores parecidos; `0` los conserva.
    pub tolerance: f64,
    /// Alfa mínimo para considerar visible un píxel.
    pub alpha_threshold: u8,
    /// Tamaño de render de cada píxel. `None` reproduce el tamaño original.
    pub pixel_size: Option<u32>,
    /// Qué cuenta como una región.
    pub grouping: Grouping,
    /// Buscar el damero de transparencia y devolverlo a transparente.
    pub remove_checkerboard: bool,
    /// Vaciar el fondo liso y recortar el resultado a lo que queda dibujado.
    pub remove_background: bool,
}

/// Estos valores son los del camino de pixel art y sólo de él: cuando exista la
/// segmentación por clustering tendrá los suyos, que son muy distintos —una foto
/// necesita bastante más tolerancia— y no deben arrastrar a estos.
impl Default for GridOptions {
    fn default() -> Self {
        GridOptions {
            scale: None,
            offset: None,
            tolerance: 12.0,
            alpha_threshold: 128,
            pixel_size: None,
            grouping: Grouping::Region,
            remove_checkerboard: true,
            remove_background: false,
        }
    }
}

/// Resultado de la conversión.
///
/// Lo que vale para cualquier segmentación va en los campos; lo que sólo
/// significa algo en una va en [`Detail`].
#[derive(Clone, Debug)]
pub struct Conversion {
    pub svg: String,
    /// Tamaño del lienzo, en las unidades del `viewBox`.
    ///
    /// No es el de la imagen de entrada: la rejilla lo da en píxeles del dibujo
    /// y no en píxeles reales, y quitar el fondo recorta por los dos caminos.
    ///
    /// Va aquí y no en `Detail` aunque el plan lo pusiera dentro: es el mismo
    /// dato para las dos segmentaciones —lo que se escribe en el `viewBox`— y
    /// duplicarlo con dos nombres obligaría a la página, al CLI y a las
    /// instantáneas a preguntar primero de qué modo vienen para leer un número
    /// que significa lo mismo en ambos.
    pub canvas: (usize, usize),
    /// Colores distintos tras fundir los parecidos.
    pub colors: usize,
    /// Elementos `<path>` del documento.
    pub paths: usize,
    /// Subtrazados emitidos, sumando todos los paths.
    pub subpaths: usize,
    /// El color de fondo retirado, si se pidió quitarlo y había uno.
    pub background: Option<Rgba>,
    /// Lo propio de cada segmentación.
    pub detail: Detail,
}

/// Los datos que sólo tienen sentido en una de las segmentaciones.
#[derive(Clone, Debug)]
pub enum Detail {
    Grid {
        /// Tamaño de celda detectado o forzado, en píxeles reales.
        cell: (f64, f64),
        /// Desplazamiento de la rejilla, en píxeles reales.
        offset: (f64, f64),
        /// El damero de transparencia encontrado, si lo había.
        checkerboard: Option<checker::Checkerboard>,
    },
    #[cfg(feature = "illustration")]
    Cluster {
        /// Regiones conexas emitidas. Es el número que hay que mirar al tocar el
        /// filtrado de motas: los colores casi no se mueven y esto sí.
        regions: usize,
        /// Degradados encontrados. Cada uno se llevó por delante un grupo de
        /// bandas, así que no se suma a `regions`: se lo resta.
        ramps: usize,
        /// Escala a la que se ha segmentado, respecto a la imagen que llegó.
        /// `1.0` es sobre su propia retícula. Ver [`resample`].
        scale: f64,
    },
}

/// Accesos a los datos de rejilla, que devuelven `None` en cualquier otro modo.
///
/// Existen para que quien sólo quiere la celda no tenga que escribir un `match`
/// con un brazo que no le importa.
impl Conversion {
    pub fn cell(&self) -> Option<(f64, f64)> {
        match self.detail {
            Detail::Grid { cell, .. } => Some(cell),
            #[cfg(feature = "illustration")]
            Detail::Cluster { .. } => None,
        }
    }

    pub fn offset(&self) -> Option<(f64, f64)> {
        match self.detail {
            Detail::Grid { offset, .. } => Some(offset),
            #[cfg(feature = "illustration")]
            Detail::Cluster { .. } => None,
        }
    }

    pub fn checkerboard(&self) -> Option<checker::Checkerboard> {
        match self.detail {
            Detail::Grid { checkerboard, .. } => checkerboard,
            #[cfg(feature = "illustration")]
            Detail::Cluster { .. } => None,
        }
    }
}

/// Las fases de una conversión, y lo que pesa cada una en el total.
///
/// Los pesos están **medidos**, no repartidos a ojo: sobre una imagen del corpus
/// de 2730x1536, de 471 ms totales, la paleta se lleva 148, el etiquetado 172,
/// el filtrado de motas 46, las fronteras 79 y el documento 26. El reparto
/// aguanta el cambio de tamaño y el de ajustador —con curvas el documento sigue
/// costando un 6%—, que es lo que hace que sirva de peso fijo.
///
/// Sin esto la barra no puede decir nada útil: el 66% del tiempo se va en dos
/// recorridos de la imagen, y una barra que sólo se moviera al acabar cada fase
/// estaría parada justo cuando hay que enseñar que algo pasa.
#[derive(Clone, Copy, Debug)]
pub enum Stage {
    /// Contar colores y construir la paleta.
    Palette,
    /// Asignar a cada píxel su entrada de la paleta.
    Regions,
    /// Regularizar esa asignación mirando el vecindario.
    Smoothing,
    /// Recorrer los tramos y etiquetar las regiones conexas.
    Runs,
    /// Fundir las motas en sus vecinas.
    Speckle,
    /// Extraer las fronteras y armar los anillos.
    Boundaries,
    /// Buscar degradados y fundir en uno cada grupo de bandas.
    Ramps,
    /// Ajustar los contornos y escribir el SVG.
    Document,
}

impl Stage {
    /// Dónde empieza y cuánto ocupa, del total.
    fn span(self) -> (f64, f64) {
        match self {
            Stage::Palette => (0.00, 0.25),
            Stage::Regions => (0.25, 0.14),
            Stage::Smoothing => (0.39, 0.18),
            Stage::Runs => (0.57, 0.10),
            Stage::Speckle => (0.67, 0.08),
            Stage::Boundaries => (0.75, 0.15),
            Stage::Ramps => (0.90, 0.05),
            Stage::Document => (0.95, 0.05),
        }
    }
}

/// A quién avisar del avance, y por dónde va.
///
/// Va como parámetro aparte y no como campo de [`Config`] a propósito: `Config`
/// es `Clone + Debug + Default` y se copia por todas partes, y meterle dentro un
/// `&mut dyn FnMut` obligaría a todo eso a arrastrar un tiempo de vida para algo
/// que sólo le importa a quien pinta una barra.
pub struct Progress<'a> {
    report: Option<&'a mut dyn FnMut(f64)>,
    /// Último tanto por ciento avisado. La limitación va aquí y no en cada
    /// llamador porque si no, uno se olvida: por fila de una imagen grande son
    /// millares de saltos al otro lado del wasm para pintar el mismo píxel.
    last: i64,
    base: f64,
    span: f64,
}

impl Default for Progress<'_> {
    fn default() -> Self {
        Progress {
            report: None,
            last: -1,
            base: 0.0,
            span: 0.0,
        }
    }
}

impl<'a> Progress<'a> {
    pub fn new(report: &'a mut dyn FnMut(f64)) -> Self {
        Progress {
            report: Some(report),
            ..Progress::default()
        }
    }

    /// Entra en una fase. Lo que se avise a partir de aquí se coloca en el hueco
    /// que le toca del total.
    pub fn stage(&mut self, stage: Stage) {
        (self.base, self.span) = stage.span();
        self.at(0, 1);
    }

    /// Avance dentro de la fase en curso.
    pub fn at(&mut self, done: usize, total: usize) {
        let Some(report) = self.report.as_mut() else {
            return;
        };
        let fraction = self.base + self.span * (done as f64 / total.max(1) as f64);
        let percent = (fraction * 100.0) as i64;
        if percent != self.last {
            self.last = percent;
            report(fraction.clamp(0.0, 1.0));
        }
    }

    /// Se acabó. Se avisa siempre, aunque el último tanto por ciento coincida.
    fn finish(&mut self) {
        if let Some(report) = self.report.as_mut() {
            report(1.0);
        }
    }
}

#[derive(Debug)]
pub enum Error {
    /// El formato no se reconoce o el fichero está corrupto.
    Decode(String),
    /// La imagen no tiene píxeles.
    EmptyImage,
    /// El búfer RGBA no cuadra con las dimensiones dadas.
    BadBufferSize { expected: usize, got: usize },
    /// La escala pedida no es utilizable.
    InvalidScale(f64),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Decode(msg) => write!(f, "no se pudo decodificar la imagen: {msg}"),
            Error::EmptyImage => write!(f, "la imagen está vacía"),
            Error::BadBufferSize { expected, got } => {
                write!(f, "se esperaban {expected} bytes de RGBA y llegaron {got}")
            }
            Error::InvalidScale(s) => write!(f, "la escala {s} debe ser mayor o igual que 1"),
        }
    }
}

impl std::error::Error for Error {}

/// Convierte una imagen codificada (PNG, JPEG, GIF, BMP o WebP).
///
/// Disponible con la característica `formats`, activa por defecto.
#[cfg(feature = "formats")]
pub fn convert(data: &[u8], config: &Config) -> Result<Conversion, Error> {
    let img = image::load_from_memory(data)
        .map_err(|e| Error::Decode(e.to_string()))?
        .to_rgba8();
    convert_image(&img, config)
}

/// Convierte un búfer RGBA sin comprimir, de `width * height * 4` bytes.
///
/// Es la vía que usa la página web: decodificar la imagen es trabajo que el
/// navegador ya sabe hacer.
pub fn convert_rgba(
    width: u32,
    height: u32,
    data: &[u8],
    config: &Config,
) -> Result<Conversion, Error> {
    convert_rgba_with(width, height, data, config, &mut Progress::default())
}

/// Lo mismo, avisando del avance.
///
/// Es otra función y no un parámetro más de [`convert_rgba`] porque casi nadie
/// lo necesita: sólo quien convierte una imagen grande delante de alguien.
pub fn convert_rgba_with(
    width: u32,
    height: u32,
    data: &[u8],
    config: &Config,
    progress: &mut Progress,
) -> Result<Conversion, Error> {
    let expected = width as usize * height as usize * 4;
    if data.len() != expected {
        return Err(Error::BadBufferSize {
            expected,
            got: data.len(),
        });
    }
    let img = RgbaImage::from_raw(width, height, data.to_vec()).ok_or(Error::EmptyImage)?;
    convert_image_with(&img, config, progress)
}

/// Convierte una imagen ya decodificada.
///
/// El proceso es `imagen -> segmentar -> regiones -> ajustar -> documento`.
pub fn convert_image(img: &RgbaImage, config: &Config) -> Result<Conversion, Error> {
    convert_image_with(img, config, &mut Progress::default())
}

pub fn convert_image_with(
    img: &RgbaImage,
    config: &Config,
    progress: &mut Progress,
) -> Result<Conversion, Error> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(Error::EmptyImage);
    }
    let out = match &config.segmentation {
        // El camino de rejilla no reparte avance: reduce la imagen a la rejilla
        // en el primer paso, y a partir de ahí trabaja sobre un dibujo de unas
        // decenas de píxeles de lado. Lo que tarda es una décima de segundo, y
        // trocearla sería inventarse un detalle que no está.
        Segmentation::Grid(options) => convert_grid(img, options, config),
        #[cfg(feature = "illustration")]
        Segmentation::Cluster(options) => Ok(convert_cluster(img, options, config, progress)),
    };
    progress.finish();
    out
}

fn convert_grid(
    img: &RgbaImage,
    options: &GridOptions,
    config: &Config,
) -> Result<Conversion, Error> {
    // El damero de transparencia es una rejilla regular más, y bien marcada:
    // hay que quitarlo de en medio antes de buscar la del dibujo.
    let mut work = Cow::Borrowed(img);
    let checkerboard = if options.remove_checkerboard {
        checker::remove(work.to_mut())
    } else {
        None
    };
    let img = work.as_ref();

    let (mut ax, mut ay) = match options.scale {
        Some(s) if s >= 1.0 => (Axis::new(s, 0.0), Axis::new(s, 0.0)),
        Some(s) => return Err(Error::InvalidScale(s)),
        None => grid::detect(img),
    };
    if let Some((ox, oy)) = options.offset {
        ax = Axis::new(ax.cell, ox);
        ay = Axis::new(ay.cell, oy);
    }

    let map = grid::downscale(img, ax, ay, options.alpha_threshold);
    let mut map = reduce_palette(map, options.tolerance);

    // Se hace después de unificar la paleta: así el fondo se retira de una pieza
    // aunque la compresión lo haya dejado con varios tonos casi iguales.
    let mut removed_background = None;
    if options.remove_background {
        removed_background = background::remove(&mut map);
        map = background::trim(map);
    }

    let regions = segment::from_pixel_map(&map, options.grouping);

    let pixel_size = options
        .pixel_size
        .unwrap_or_else(|| ax.cell.max(ay.cell).round() as u32)
        .max(1);
    let out = svg::render(
        &regions,
        &svg::Options {
            pixel_size,
            display: None,
            background: config.background.clone(),
            fit: config.fit,
        },
    );

    Ok(Conversion {
        svg: out.svg,
        canvas: (map.width, map.height),
        colors: out.colors,
        paths: out.paths,
        subpaths: out.subpaths,
        background: removed_background,
        detail: Detail::Grid {
            cell: (ax.cell, ay.cell),
            offset: (ax.offset, ay.offset),
            checkerboard,
        },
    })
}

/// La imagen a la escala a la que se va a segmentar, o la misma si no hay que
/// reescalarla.
///
/// Existe como función porque tiene dos clientes: la conversión y la medida de
/// blandura de [`softness`], que tiene que mirar exactamente los píxeles a los que
/// se refiere la geometría.
#[cfg(feature = "illustration")]
fn working<'a>(img: &'a RgbaImage, options: &ClusterOptions) -> Cow<'a, RgbaImage> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let simplify = options.simplify.unwrap_or(resample::SIMPLIFY);
    match resample::working_size(w, h, simplify) {
        Some((tw, th)) => Cow::Owned(resample::resize(img, tw, th)),
        None => Cow::Borrowed(img),
    }
}

/// La blandura de cada frontera de una imagen: cuántos píxeles tarda en pasar de un
/// color al otro. Ver [`softness`].
///
/// Es una medida y no una conversión: no escribe documento ninguno. Está aquí porque
/// necesita el trozo de delante del proceso —escala de trabajo, paleta, regiones,
/// fronteras— y ese trozo no debe estar escrito dos veces.
#[cfg(feature = "illustration")]
pub fn softness_of(img: &RgbaImage, options: &ClusterOptions) -> Vec<softness::Softness> {
    let scaled = working(img, options);
    let clustering = cluster::from_image(scaled.as_ref(), options);
    let regions = boundary::from_clustering(&clustering);
    softness::measure(&regions, &clustering, scaled.as_ref(), options.tolerance)
}

/// La otra segmentación: paleta y regiones conexas, sin rejilla ninguna.
///
/// Son dos llamadas porque el trabajo está en [`cluster`] y [`boundary`]: aquí
/// sólo se encadenan y se rellena el resultado. Nada del camino de rejilla —el
/// damero, la celda, el `pixel_size` que sigue a la escala detectada— aparece,
/// que es justo lo que quiere decir que sean dos ejes y no dos programas.
#[cfg(feature = "illustration")]
fn convert_cluster(
    img: &RgbaImage,
    options: &ClusterOptions,
    config: &Config,
    progress: &mut Progress,
) -> Conversion {
    // La escala de trabajo va **primero**, antes de que nadie mire un píxel: todo
    // lo que viene después está en píxeles absolutos, y lo que esos píxeles miden
    // se decide aquí. Ver [`resample`].
    let scaled = working(img, options);
    let scale = scaled.width() as f64 / img.width() as f64;
    let img = scaled.as_ref();

    let clustering = cluster::from_image_with(img, options, progress);
    progress.stage(Stage::Boundaries);
    let mut regions = boundary::from_clustering(&clustering);
    if options.subpixel {
        subpixel::place(&mut regions, &clustering, img);
    }
    // Después del subpíxel, porque relaja los vértices **ya colocados**: los dos
    // dicen dónde está de verdad el borde, y el orden natural es colocar y luego
    // limar. Y antes de los degradados, que sólo eligen qué tramos se dibujan.
    wobble::relax(&mut regions, options.relax);
    if options.ramps {
        // Con los tramos ya colocados, porque fundir un grupo sólo elige cuáles se
        // dibujan y no toca ninguno.
        progress.stage(Stage::Ramps);
        // La blandura de cada frontera se mide sobre la imagen a la escala de
        // trabajo, que es la que la geometría describe, y sólo cuando hay
        // degradados que buscar: es una pasada por los píxeles del borde y no se
        // paga si nadie la va a leer.
        let soft = softness::soft_edges(
            &softness::measure(&regions, &clustering, img, options.tolerance),
            regions.edges.len(),
        );
        ramp::merge(&mut regions, &clustering.labels, options.tolerance, &soft);
    }
    progress.stage(Stage::Document);

    // Una unidad del `viewBox` es un píxel **de trabajo**, así que las
    // coordenadas salen tal como se han trazado y no reescaladas una a una; el
    // documento se anuncia al tamaño de la imagen que llegó. Si el fondo se
    // retiró, el lienzo viene recortado y el anuncio se recorta con él.
    let display = (scale != 1.0).then(|| {
        let back = |v: usize| ((v as f64 / scale).round() as usize).max(1);
        (back(regions.width), back(regions.height))
    });
    let out = svg::render(
        &regions,
        &svg::Options {
            pixel_size: 1,
            display,
            background: config.background.clone(),
            fit: config.fit,
        },
    );

    Conversion {
        svg: out.svg,
        canvas: (regions.width, regions.height),
        colors: out.colors,
        paths: out.paths,
        subpaths: out.subpaths,
        background: clustering.background,
        detail: Detail::Cluster {
            regions: regions.regions.len(),
            ramps: regions.ramps.len(),
            scale,
        },
    }
}

/// Funde los colores parecidos del mapa según la tolerancia indicada.
fn reduce_palette(map: PixelMap, tolerance: f64) -> PixelMap {
    if tolerance <= 0.0 {
        return map;
    }
    let mut counts: HashMap<Rgba, usize> = HashMap::new();
    for pixel in map.pixels.iter().flatten() {
        *counts.entry(*pixel).or_insert(0) += 1;
    }
    let list: Vec<(Rgba, usize)> = counts.into_iter().collect();
    let mapping: HashMap<Rgba, Rgba> = color::build_palette(&list, tolerance).into_iter().collect();

    PixelMap {
        width: map.width,
        height: map.height,
        pixels: map
            .pixels
            .into_iter()
            .map(|p| p.map(|c| mapping[&c]))
            .collect(),
    }
}
