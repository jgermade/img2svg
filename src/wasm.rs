//! Enlace con JavaScript. Se compila sólo con `--features wasm`.
//!
//! La API que ve la página es `convert(bytes, options)`, donde `options` es un
//! objeto plano con las mismas claves que [`crate::Config`] (todas opcionales).

use js_sys::Reflect;
use wasm_bindgen::prelude::*;

#[cfg(feature = "photo")]
use crate::ClusterOptions;
use crate::{Config, GridOptions, Grouping};

/// Resultado de la conversión, con los datos de la rejilla empleada.
#[wasm_bindgen]
pub struct Conversion {
    inner: crate::Conversion,
}

#[wasm_bindgen]
impl Conversion {
    #[wasm_bindgen(getter)]
    pub fn svg(&self) -> String {
        self.inner.svg.clone()
    }

    /// Ancho de la rejilla, en píxeles del dibujo.
    #[wasm_bindgen(getter, js_name = gridWidth)]
    pub fn grid_width(&self) -> usize {
        self.inner.canvas.0
    }

    /// Alto de la rejilla, en píxeles del dibujo.
    #[wasm_bindgen(getter, js_name = gridHeight)]
    pub fn grid_height(&self) -> usize {
        self.inner.canvas.1
    }

    /// Tamaño de celda detectado en el eje X, en píxeles reales.
    #[wasm_bindgen(getter, js_name = cellWidth)]
    pub fn cell_width(&self) -> f64 {
        self.inner.cell().map_or(0.0, |c| c.0)
    }

    /// Tamaño de celda detectado en el eje Y, en píxeles reales.
    #[wasm_bindgen(getter, js_name = cellHeight)]
    pub fn cell_height(&self) -> f64 {
        self.inner.cell().map_or(0.0, |c| c.1)
    }

    #[wasm_bindgen(getter, js_name = offsetX)]
    pub fn offset_x(&self) -> f64 {
        self.inner.offset().map_or(0.0, |o| o.0)
    }

    #[wasm_bindgen(getter, js_name = offsetY)]
    pub fn offset_y(&self) -> f64 {
        self.inner.offset().map_or(0.0, |o| o.1)
    }

    /// Colores distintos del SVG, uno por `<path>`.
    #[wasm_bindgen(getter)]
    pub fn colors(&self) -> usize {
        self.inner.colors
    }

    /// Elementos `<path>` del documento.
    #[wasm_bindgen(getter)]
    pub fn paths(&self) -> usize {
        self.inner.paths
    }

    /// Subtrazados emitidos, sumando todos los paths.
    #[wasm_bindgen(getter)]
    pub fn subpaths(&self) -> usize {
        self.inner.subpaths
    }

    /// Lado de la casilla del damero de transparencia encontrado, o `undefined`.
    #[wasm_bindgen(getter, js_name = checkerCell)]
    pub fn checker_cell(&self) -> Option<f64> {
        self.inner.checkerboard().map(|c| c.cell.0)
    }

    /// Fracción de imagen devuelta a transparente al quitar el damero.
    #[wasm_bindgen(getter, js_name = checkerCoverage)]
    pub fn checker_coverage(&self) -> Option<f64> {
        self.inner.checkerboard().map(|c| c.coverage)
    }

    /// Color de fondo retirado, en hexadecimal, o `undefined`.
    #[wasm_bindgen(getter)]
    pub fn background(&self) -> Option<String> {
        self.inner.background.map(|c| c.to_hex())
    }
}

/// Convierte un búfer RGBA (el que devuelve `ctx.getImageData()`) en SVG.
#[wasm_bindgen(js_name = convertRgba)]
pub fn convert_rgba(
    width: u32,
    height: u32,
    data: &[u8],
    options: &JsValue,
) -> Result<Conversion, JsError> {
    let config = read_config(options);
    crate::convert_rgba(width, height, data, &config)
        .map(|inner| Conversion { inner })
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Resultado de una conversión de foto.
///
/// Es un tipo aparte y no unos cuantos `undefined` más en [`Conversion`]: los
/// dos caminos no comparten casi ninguna cifra —no hay rejilla, ni celda, ni
/// damero, y sí un recuento de regiones— y así el `.d.ts` **gana** un tipo en
/// vez de que el que ya consume la página se llene de campos opcionales.
#[cfg(feature = "photo")]
#[wasm_bindgen]
pub struct PhotoConversion {
    inner: crate::Conversion,
}

#[cfg(feature = "photo")]
#[wasm_bindgen]
impl PhotoConversion {
    #[wasm_bindgen(getter)]
    pub fn svg(&self) -> String {
        self.inner.svg.clone()
    }

    /// Ancho del lienzo, que es el del `viewBox`. No tiene por qué ser el de la
    /// imagen: quitar el fondo recorta.
    #[wasm_bindgen(getter, js_name = canvasWidth)]
    pub fn canvas_width(&self) -> usize {
        self.inner.canvas.0
    }

    #[wasm_bindgen(getter, js_name = canvasHeight)]
    pub fn canvas_height(&self) -> usize {
        self.inner.canvas.1
    }

    /// Entradas de la paleta.
    #[wasm_bindgen(getter)]
    pub fn colors(&self) -> usize {
        self.inner.colors
    }

    #[wasm_bindgen(getter)]
    pub fn paths(&self) -> usize {
        self.inner.paths
    }

    #[wasm_bindgen(getter)]
    pub fn subpaths(&self) -> usize {
        self.inner.subpaths
    }

    /// Regiones conexas emitidas. Es la cifra que se mueve al tocar el filtrado
    /// de motas, y la que dice si el SVG se puede abrir en un editor.
    #[wasm_bindgen(getter)]
    pub fn regions(&self) -> usize {
        match self.inner.detail {
            crate::Detail::Cluster { regions } => regions,
            _ => 0,
        }
    }

    /// Color de fondo retirado, en hexadecimal, o `undefined`.
    #[wasm_bindgen(getter)]
    pub fn background(&self) -> Option<String> {
        self.inner.background.map(|c| c.to_hex())
    }
}

/// Convierte un búfer RGBA por el camino de foto.
///
/// Va aparte de [`convert_rgba`] en vez de mirar una clave `mode` dentro de las
/// opciones porque son dos juegos de ajustes que no se solapan: una función por
/// segmentación deja que cada una lea sólo lo suyo, y que el `.d.ts` diga cuál
/// devuelve qué.
#[cfg(feature = "photo")]
#[wasm_bindgen(js_name = convertPhoto)]
pub fn convert_photo(
    width: u32,
    height: u32,
    data: &[u8],
    options: &JsValue,
) -> Result<PhotoConversion, JsError> {
    let config = read_cluster_config(options);
    crate::convert_rgba(width, height, data, &config)
        .map(|inner| PhotoConversion { inner })
        .map_err(|e| JsError::new(&e.to_string()))
}

#[cfg(feature = "photo")]
fn read_cluster_config(options: &JsValue) -> Config {
    let default = ClusterOptions::default();
    if options.is_falsy() {
        return Config::cluster(default);
    }
    let o = Options(options);

    let cluster = ClusterOptions {
        color_precision: o
            .byte("colorPrecision")
            .unwrap_or(default.color_precision)
            .clamp(1, 8),
        tolerance: o.number("tolerance").unwrap_or(default.tolerance),
        alpha_threshold: o.byte("alphaThreshold").unwrap_or(default.alpha_threshold),
        filter_speckle: o.count("filterSpeckle").unwrap_or(default.filter_speckle),
        min_thickness: o.number("minThickness").unwrap_or(default.min_thickness),
        gradient_step: o.number("gradientStep").unwrap_or(default.gradient_step),
        max_colors: o.count("maxColors").unwrap_or(default.max_colors),
        // Igual que en el CLI: una paleta impuesta pide una sintaxis que la
        // página no tiene dónde poner. Queda en la biblioteca.
        palette: Vec::new(),
        remove_background: o
            .flag("removeBackground")
            .unwrap_or(default.remove_background),
    };

    Config {
        background: o.text("background"),
        ..Config::cluster(cluster)
    }
}

/// Lector del objeto plano que manda la página.
///
/// Las claves son deliberadamente **planas** y no reflejan la partición interna
/// en segmentación y ajuste: son la API pública que consume `web/app.js`, y
/// cambiarlas rompería la página sin avisar en tiempo de compilación.
struct Options<'a>(&'a JsValue);

impl Options<'_> {
    fn get(&self, key: &str) -> Option<JsValue> {
        Reflect::get(self.0, &JsValue::from_str(key)).ok()
    }

    fn number(&self, key: &str) -> Option<f64> {
        self.get(key)
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
    }

    /// Un entero no negativo, que es lo que son todos los contadores de la API.
    /// Sólo lo usa el camino de foto, que puede estar compilado fuera.
    #[cfg(feature = "photo")]
    fn count(&self, key: &str) -> Option<usize> {
        self.number(key).map(|v| v.max(0.0) as usize)
    }

    fn byte(&self, key: &str) -> Option<u8> {
        self.number(key).map(|v| v.clamp(0.0, 255.0) as u8)
    }

    fn text(&self, key: &str) -> Option<String> {
        self.get(key)
            .and_then(|v| v.as_string())
            .filter(|v| !v.is_empty())
    }

    fn flag(&self, key: &str) -> Option<bool> {
        self.get(key)
            .filter(|v| !v.is_undefined() && !v.is_null())
            .map(|v| v.is_truthy())
    }
}

fn read_config(options: &JsValue) -> Config {
    let default = GridOptions::default();
    if options.is_falsy() {
        return Config::default();
    }
    let o = Options(options);

    let grid = GridOptions {
        scale: o.number("scale"),
        offset: o.number("offsetX").zip(o.number("offsetY")),
        tolerance: o.number("tolerance").unwrap_or(default.tolerance),
        alpha_threshold: o.byte("alphaThreshold").unwrap_or(default.alpha_threshold),
        pixel_size: o.number("pixelSize").map(|v| v.max(1.0) as u32),
        grouping: if o.flag("mergeColors").unwrap_or(false) {
            Grouping::Color
        } else {
            Grouping::Region
        },
        remove_checkerboard: o
            .flag("removeCheckerboard")
            .unwrap_or(default.remove_checkerboard),
        remove_background: o
            .flag("removeBackground")
            .unwrap_or(default.remove_background),
    };

    Config {
        background: o.text("background"),
        ..Config::grid(grid)
    }
}
