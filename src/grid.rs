//! Detección de la rejilla de píxeles lógicos y reducción de la imagen.
//!
//! El pixel art suele llegar escalado (un píxel "de arte" ocupa NxN píxeles
//! reales). Aquí se estima ese factor por eje, junto con el desplazamiento de
//! la rejilla, y se reduce la imagen a un píxel por celda.

use image::RgbaImage;

use crate::color::Rgba;

#[derive(Clone, Copy, Debug)]
pub struct Axis {
    /// Tamaño de celda en píxeles reales (puede no ser entero si la imagen se
    /// reescaló con un factor arbitrario).
    pub cell: f64,
    /// Posición de la primera línea de rejilla, en `0..cell`.
    pub offset: f64,
}

impl Axis {
    pub fn new(cell: f64, offset: f64) -> Self {
        let cell = cell.max(1.0);
        Axis {
            cell,
            offset: offset.rem_euclid(cell),
        }
    }
}

/// Mapa de píxeles lógicos: `None` es transparente.
pub struct PixelMap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Option<Rgba>>,
}

/// Diferencia media entre columnas (o filas) consecutivas.
///
/// Sólo cuentan los pares en los que ambos píxeles son visibles. Lo transparente
/// no aporta: ni el color basura que suele haber debajo, ni el canto del recorte
/// —que, si la imagen traía un damero de transparencia ya retirado, seguiría el
/// paso del damero y no el del dibujo.
fn axis_gradient(img: &RgbaImage, horizontal: bool) -> Vec<f64> {
    let (w, h) = img.dimensions();
    let (len, cross) = if horizontal { (w, h) } else { (h, w) };
    let mut out = vec![0.0; len as usize];

    for i in 1..len {
        let mut acc = 0.0;
        for j in 0..cross {
            let (xa, ya, xb, yb) = if horizontal {
                (i - 1, j, i, j)
            } else {
                (j, i - 1, j, i)
            };
            let a = img.get_pixel(xa, ya).0;
            let b = img.get_pixel(xb, yb).0;
            if a[3] == 0 || b[3] == 0 {
                continue;
            }
            acc += (0..4)
                .map(|c| (a[c] as f64 - b[c] as f64).abs())
                .fold(0.0, f64::max);
        }
        // Se normaliza por la línea entera, no por los pares que han contado:
        // así una franja casi transparente no infla su gradiente.
        out[i as usize] = acc / cross as f64;
    }
    out
}

/// Coherencia mínima para dar por buena una rejilla; por debajo se asume que la
/// imagen ya está a escala 1:1.
const MIN_SCORE: f64 = 0.35;
/// Un pico se considera equivalente al mejor si llega a esta fracción de él.
/// Sirve para quedarse con la frecuencia fundamental en vez de sus armónicos.
const PEAK_RATIO: f64 = 0.75;

/// Coherencia de la señal con un periodo `1/f`, en `0..1`, junto con su fase.
fn coherence(grad: &[f64], total: f64, f: f64) -> (f64, f64) {
    let (mut re, mut im) = (0.0, 0.0);
    for (x, g) in grad.iter().enumerate() {
        if *g == 0.0 {
            continue;
        }
        let angle = std::f64::consts::TAU * f * x as f64;
        re += g * angle.cos();
        im -= g * angle.sin();
    }
    (re.hypot(im) / total, im.atan2(re))
}

/// Afina la posición de un pico dentro del intervalo `(lo, hi)`.
fn refine_peak(grad: &[f64], total: f64, lo: f64, hi: f64) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    for _ in 0..48 {
        let a = lo + (hi - lo) / 3.0;
        let b = hi - (hi - lo) / 3.0;
        if coherence(grad, total, a).0 < coherence(grad, total, b).0 {
            lo = a;
        } else {
            hi = b;
        }
    }
    (lo + hi) / 2.0
}

/// Estima el tamaño de celda de un eje.
///
/// Los saltos de color de un pixel art escalado caen sobre una rejilla regular,
/// así que el gradiente es una señal periódica. Para cada periodo candidato `c`
/// se mide cuánta de su energía se concentra en esa frecuencia
/// (`|Σ g(x)·e^(-2πix/c)| / Σ g(x)`, entre 0 y 1) y se elige el mayor periodo
/// que supere el umbral. La fase de esa suma da directamente el desplazamiento.
fn detect_axis(grad: &[f64], max_cell: f64) -> Axis {
    let total: f64 = grad.iter().sum();
    if total <= 0.0 || max_cell < 2.0 {
        return Axis::new(1.0, 0.0);
    }
    let len = grad.len() as f64;
    let tau = std::f64::consts::TAU;

    // Se barre en frecuencia (no en periodo) para muestrear por igual todas las
    // escalas; el paso resuelve un cuarto de la anchura de un lóbulo.
    let step = 1.0 / (4.0 * len);
    let f_min = 1.0 / max_cell;
    let steps = (((0.5 - f_min) / step).ceil() as usize).max(1);

    let scores: Vec<f64> = (0..=steps)
        .map(|k| coherence(grad, total, f_min + k as f64 * step).0)
        .collect();

    // Máximos locales, de menor a mayor frecuencia.
    let peaks: Vec<usize> = (0..scores.len())
        .filter(|&i| {
            (i == 0 || scores[i - 1] <= scores[i])
                && (i + 1 == scores.len() || scores[i + 1] <= scores[i])
        })
        .collect();
    let best = peaks.iter().fold(0.0_f64, |m, &i| m.max(scores[i]));
    if best < MIN_SCORE {
        return Axis::new(1.0, 0.0);
    }

    // Los divisores de la celda real describen la misma rejilla partida y
    // puntúan casi igual de bien, así que nos quedamos con la celda más grande
    // (la frecuencia fundamental) de entre los picos equivalentes al mejor.
    let peak = peaks
        .iter()
        .copied()
        .find(|&i| scores[i] >= best * PEAK_RATIO)
        .unwrap();

    // El barrido es grueso para lo que necesitamos: un error de centésimas en
    // el periodo desalinea la rejilla al otro extremo de la imagen.
    let f = refine_peak(
        grad,
        total,
        f_min + peak.saturating_sub(1) as f64 * step,
        f_min + (peak + 1) as f64 * step,
    );
    let (_, phase) = coherence(grad, total, f);

    let cell = 1.0 / f;
    // Un periodo casi entero casi siempre lo es.
    let cell = if (cell - cell.round()).abs() < 0.02 {
        cell.round()
    } else {
        cell
    };
    Axis::new(cell, -phase * cell / tau)
}

/// Detecta el factor de escala en ambos ejes.
pub fn detect(img: &RgbaImage) -> (Axis, Axis) {
    let (w, h) = img.dimensions();
    let max_cell = (w.min(h) as f64 / 4.0).max(1.0);
    let mut x = detect_axis(&axis_gradient(img, true), max_cell);
    let mut y = detect_axis(&axis_gradient(img, false), max_cell);

    // En la práctica los dos ejes comparten escala: si en uno falla la
    // detección, se hereda la del otro.
    if x.cell == 1.0 && y.cell > 1.0 {
        x = Axis::new(y.cell, 0.0);
    } else if y.cell == 1.0 && x.cell > 1.0 {
        y = Axis::new(x.cell, 0.0);
    }
    (x, y)
}

/// Límites de celda de un eje: la rejilla se ancla en `offset` y se extiende
/// hasta el final. Las celdas de los extremos que quedan por debajo de un tercio
/// del ancho normal son restos del recorte y se descartan.
fn cell_bounds(len: u32, axis: Axis) -> Vec<(u32, u32)> {
    let cell = axis.cell.max(1.0);
    let mut lines = vec![0u32];
    let mut k = 0.0;
    loop {
        let pos = (axis.offset + k * cell).round();
        if pos >= len as f64 {
            break;
        }
        let pos = pos as u32;
        if pos > *lines.last().unwrap() {
            lines.push(pos);
        }
        k += 1.0;
    }
    lines.push(len);

    let min_width = (cell / 3.0).ceil() as u32;
    let mut bounds: Vec<(u32, u32)> = lines.windows(2).map(|w| (w[0], w[1])).collect();
    if bounds.len() > 1 && bounds[0].1 - bounds[0].0 < min_width {
        bounds.remove(0);
    }
    if bounds.len() > 1 {
        let last = bounds.len() - 1;
        if bounds[last].1 - bounds[last].0 < min_width {
            bounds.remove(last);
        }
    }
    bounds
}

/// Color representativo de una celda: se muestrea sólo su parte central (para
/// esquivar el antialiasing de los bordes) y se toma el tono mayoritario.
fn cell_color(
    img: &RgbaImage,
    xr: (u32, u32),
    yr: (u32, u32),
    alpha_threshold: u8,
) -> Option<Rgba> {
    let inset = |(a, b): (u32, u32)| -> (u32, u32) {
        let len = b - a;
        if len <= 2 {
            (a, b)
        } else {
            let pad = len / 4;
            (a + pad, b - pad)
        }
    };
    let (x0, x1) = inset(xr);
    let (y0, y1) = inset(yr);

    // Histograma sobre cubos de 8x8x8x8 para tolerar ruido de compresión.
    let mut buckets: Vec<(u32, [u64; 4], usize)> = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let p = img.get_pixel(x, y).0;
            let key = ((p[0] as u32 >> 3) << 15)
                | ((p[1] as u32 >> 3) << 10)
                | ((p[2] as u32 >> 3) << 5)
                | (p[3] as u32 >> 3);
            match buckets.iter_mut().find(|b| b.0 == key) {
                Some(b) => {
                    for (sum, channel) in b.1.iter_mut().zip(p) {
                        *sum += channel as u64;
                    }
                    b.2 += 1;
                }
                None => {
                    buckets.push((key, [p[0] as u64, p[1] as u64, p[2] as u64, p[3] as u64], 1))
                }
            }
        }
    }

    let (_, sums, n) = buckets.into_iter().max_by_key(|b| b.2)?;
    let avg = |i: usize| (sums[i] as f64 / n as f64).round() as u8;
    let a = avg(3);
    if a < alpha_threshold {
        return None;
    }
    Some(Rgba::new(avg(0), avg(1), avg(2), a))
}

/// Reduce la imagen a un píxel lógico por celda de la rejilla.
pub fn downscale(img: &RgbaImage, ax: Axis, ay: Axis, alpha_threshold: u8) -> PixelMap {
    let (w, h) = img.dimensions();
    let xs = cell_bounds(w, ax);
    let ys = cell_bounds(h, ay);

    let mut pixels = Vec::with_capacity(xs.len() * ys.len());
    for yr in &ys {
        for xr in &xs {
            pixels.push(cell_color(img, *xr, *yr, alpha_threshold));
        }
    }

    PixelMap {
        width: xs.len(),
        height: ys.len(),
        pixels,
    }
}
