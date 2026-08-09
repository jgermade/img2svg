//! Tipos de color y reducción de paleta.

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
