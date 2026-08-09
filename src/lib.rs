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
//! assert_eq!(out.grid, (2, 2));
//! assert_eq!(out.colors, 2);
//! ```

pub mod background;
#[cfg(feature = "photo")]
pub mod boundary;
pub mod checker;
#[cfg(feature = "photo")]
pub mod cluster;
pub mod color;
pub mod fit;
pub mod grid;
pub mod region;
pub mod segment;
pub mod svg;
pub mod trace;

#[cfg(feature = "wasm")]
mod wasm;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use image::RgbaImage;

use crate::color::Rgba;
use crate::grid::{Axis, PixelMap};

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

    /// Las opciones de rejilla, para leerlas o retocarlas.
    pub fn grid_options(&self) -> &GridOptions {
        let Segmentation::Grid(options) = &self.segmentation;
        options
    }

    pub fn grid_options_mut(&mut self) -> &mut GridOptions {
        let Segmentation::Grid(options) = &mut self.segmentation;
        options
    }
}

/// Cómo se pasa de la imagen a un conjunto de regiones.
#[derive(Clone, Debug)]
pub enum Segmentation {
    /// Pixel art: se detecta la rejilla y se reduce la imagen a ella.
    Grid(GridOptions),
    // Cluster(ClusterOptions) entra con la segmentación para fotos.
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

/// Resultado de la conversión, con los datos de la rejilla usada.
#[derive(Clone, Debug)]
pub struct Conversion {
    pub svg: String,
    /// Tamaño de la rejilla, en píxeles del dibujo.
    pub grid: (usize, usize),
    /// Tamaño de celda detectado o forzado, en píxeles reales.
    pub cell: (f64, f64),
    /// Desplazamiento de la rejilla, en píxeles reales.
    pub offset: (f64, f64),
    /// Colores distintos tras fundir los parecidos.
    pub colors: usize,
    /// Elementos `<path>` del documento.
    pub paths: usize,
    /// Subtrazados emitidos, sumando todos los paths.
    pub subpaths: usize,
    /// El damero de transparencia encontrado, si lo había.
    pub checkerboard: Option<checker::Checkerboard>,
    /// El color de fondo retirado, si se pidió quitarlo y había uno.
    pub background: Option<Rgba>,
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
    let expected = width as usize * height as usize * 4;
    if data.len() != expected {
        return Err(Error::BadBufferSize {
            expected,
            got: data.len(),
        });
    }
    let img = RgbaImage::from_raw(width, height, data.to_vec()).ok_or(Error::EmptyImage)?;
    convert_image(&img, config)
}

/// Convierte una imagen ya decodificada.
///
/// El proceso es `imagen -> segmentar -> regiones -> ajustar -> documento`.
pub fn convert_image(img: &RgbaImage, config: &Config) -> Result<Conversion, Error> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(Error::EmptyImage);
    }
    let Segmentation::Grid(options) = &config.segmentation;
    convert_grid(img, options, config)
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
            background: config.background.clone(),
            fit: config.fit,
        },
    );

    Ok(Conversion {
        svg: out.svg,
        grid: (map.width, map.height),
        cell: (ax.cell, ay.cell),
        offset: (ax.offset, ay.offset),
        colors: out.colors,
        paths: out.paths,
        subpaths: out.subpaths,
        checkerboard,
        background: removed_background,
    })
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
