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
//! let out = px2svg::convert_rgba(2, 2, &rgba, &px2svg::Config::default()).unwrap();
//! assert_eq!(out.grid, (2, 2));
//! assert_eq!(out.colors, 2);
//! ```

pub mod background;
pub mod checker;
pub mod color;
pub mod grid;
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

pub use crate::svg::Grouping;

/// Parámetros de la conversión.
#[derive(Clone, Debug)]
pub struct Config {
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
    /// Color de fondo opcional del SVG.
    pub background: Option<String>,
    /// Cómo se reparten los píxeles entre los `<path>`.
    pub grouping: Grouping,
    /// Buscar el damero de transparencia y devolverlo a transparente.
    pub remove_checkerboard: bool,
    /// Vaciar el fondo liso y recortar el resultado a lo que queda dibujado.
    pub remove_background: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            scale: None,
            offset: None,
            tolerance: 12.0,
            alpha_threshold: 128,
            pixel_size: None,
            background: None,
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
pub fn convert_image(img: &RgbaImage, config: &Config) -> Result<Conversion, Error> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(Error::EmptyImage);
    }

    // El damero de transparencia es una rejilla regular más, y bien marcada:
    // hay que quitarlo de en medio antes de buscar la del dibujo.
    let mut work = Cow::Borrowed(img);
    let checkerboard = if config.remove_checkerboard {
        checker::remove(work.to_mut())
    } else {
        None
    };
    let img = work.as_ref();

    let (mut ax, mut ay) = match config.scale {
        Some(s) if s >= 1.0 => (Axis::new(s, 0.0), Axis::new(s, 0.0)),
        Some(s) => return Err(Error::InvalidScale(s)),
        None => grid::detect(img),
    };
    if let Some((ox, oy)) = config.offset {
        ax = Axis::new(ax.cell, ox);
        ay = Axis::new(ay.cell, oy);
    }

    let map = grid::downscale(img, ax, ay, config.alpha_threshold);
    let mut map = reduce_palette(map, config.tolerance);

    // Se hace después de unificar la paleta: así el fondo se retira de una pieza
    // aunque la compresión lo haya dejado con varios tonos casi iguales.
    let mut removed_background = None;
    if config.remove_background {
        removed_background = background::remove(&mut map);
        map = background::trim(map);
    }

    let pixel_size = config
        .pixel_size
        .unwrap_or_else(|| ax.cell.max(ay.cell).round() as u32)
        .max(1);
    let out = svg::render(
        &map,
        &svg::Options {
            pixel_size,
            background: config.background.clone(),
            grouping: config.grouping,
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
