//! Enlace con JavaScript. Se compila sólo con `--features wasm`.
//!
//! La API que ve la página es `convert(bytes, options)`, donde `options` es un
//! objeto plano con las mismas claves que [`crate::Config`] (todas opcionales).

use js_sys::Reflect;
use wasm_bindgen::prelude::*;

use crate::{Config, Grouping};

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
        self.inner.grid.0
    }

    /// Alto de la rejilla, en píxeles del dibujo.
    #[wasm_bindgen(getter, js_name = gridHeight)]
    pub fn grid_height(&self) -> usize {
        self.inner.grid.1
    }

    /// Tamaño de celda detectado en el eje X, en píxeles reales.
    #[wasm_bindgen(getter, js_name = cellWidth)]
    pub fn cell_width(&self) -> f64 {
        self.inner.cell.0
    }

    /// Tamaño de celda detectado en el eje Y, en píxeles reales.
    #[wasm_bindgen(getter, js_name = cellHeight)]
    pub fn cell_height(&self) -> f64 {
        self.inner.cell.1
    }

    #[wasm_bindgen(getter, js_name = offsetX)]
    pub fn offset_x(&self) -> f64 {
        self.inner.offset.0
    }

    #[wasm_bindgen(getter, js_name = offsetY)]
    pub fn offset_y(&self) -> f64 {
        self.inner.offset.1
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
        self.inner.checkerboard.map(|c| c.cell.0)
    }

    /// Fracción de imagen devuelta a transparente al quitar el damero.
    #[wasm_bindgen(getter, js_name = checkerCoverage)]
    pub fn checker_coverage(&self) -> Option<f64> {
        self.inner.checkerboard.map(|c| c.coverage)
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

fn read_config(options: &JsValue) -> Config {
    let default = Config::default();
    if options.is_falsy() {
        return default;
    }
    let number = |key: &str| -> Option<f64> {
        Reflect::get(options, &JsValue::from_str(key))
            .ok()
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
    };
    let text = |key: &str| -> Option<String> {
        Reflect::get(options, &JsValue::from_str(key))
            .ok()
            .and_then(|v| v.as_string())
            .filter(|v| !v.is_empty())
    };
    let flag = |key: &str| -> Option<bool> {
        Reflect::get(options, &JsValue::from_str(key))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null())
            .map(|v| v.is_truthy())
    };

    Config {
        scale: number("scale"),
        offset: number("offsetX").zip(number("offsetY")),
        tolerance: number("tolerance").unwrap_or(default.tolerance),
        alpha_threshold: number("alphaThreshold")
            .map(|v| v.clamp(0.0, 255.0) as u8)
            .unwrap_or(default.alpha_threshold),
        pixel_size: number("pixelSize").map(|v| v.max(1.0) as u32),
        background: text("background"),
        grouping: if flag("mergeColors").unwrap_or(false) {
            Grouping::Color
        } else {
            Grouping::Region
        },
        remove_checkerboard: flag("removeCheckerboard").unwrap_or(default.remove_checkerboard),
        remove_background: flag("removeBackground").unwrap_or(default.remove_background),
    }
}
