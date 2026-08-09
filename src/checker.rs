//! Detección de la cuadrícula de transparencia.
//!
//! Cuando alguien captura la pantalla de un editor, el damero blanco/gris con
//! el que se dibuja el fondo transparente se queda pegado en la imagen como
//! píxeles opacos. Aquí se busca ese patrón —dos grises alternándose en un
//! tablero regular— y se devuelve a transparencia.
//!
//! El reconocimiento es exigente a propósito: sólo se borran los píxeles que
//! caen en casillas *enteras* del damero, así que un blanco suelto del dibujo
//! que coincida con el color y la paridad de la cuadrícula sobrevive.

use std::collections::HashMap;

use image::RgbaImage;

use crate::color::Rgba;

/// Diferencia máxima entre canales para considerar un color un gris.
const MAX_SATURATION: i32 = 24;
/// Los dos tonos del damero se parecen, pero se distinguen.
const CONTRAST: std::ops::Range<i32> = 4..140;
/// Margen con el que se reconoce cada tono. Absorbe el ruido de compresión sin
/// llegar a tragarse los planos vecinos del dibujo.
const MATCH: f64 = 12.0;
/// Colores más frecuentes entre los que se busca la pareja.
const CANDIDATES: usize = 8;
/// Parejas que se llegan a analizar a fondo, de más a menos frecuentes.
const PAIRS: usize = 4;
/// Tiras mínimas por eje para fiarse de la medida.
const MIN_RUNS: usize = 24;
/// Proporción de tiras que debe medir lo mismo. Es el criterio flojo: el dibujo
/// suele traer blancos que se funden con los del damero y parten las tiras.
const UNIFORMITY: f64 = 0.7;
/// Proporción de tiras completas que debe arrancar en la misma fase. Este es el
/// criterio duro: una rejilla de verdad las alinea todas.
const ALIGNMENT: f64 = 0.9;
/// Proporción de la casilla que debe cuadrar para darla por parte del damero.
const CELL_AGREEMENT: f64 = 0.9;
/// Por debajo de esta fracción de imagen, la detección se descarta.
const MIN_COVERAGE: f64 = 0.05;

#[derive(Clone, Copy, Debug)]
pub struct Checkerboard {
    /// Lado de la casilla por eje, en píxeles reales. No tiene por qué ser
    /// entero: basta con que la imagen se haya reescalado alguna vez.
    pub cell: (f64, f64),
    /// Los dos tonos del damero.
    pub colors: (Rgba, Rgba),
    /// Fracción de la imagen devuelta a transparencia.
    pub coverage: f64,
}

/// Parámetros geométricos del damero, una vez encajado.
#[derive(Clone, Copy)]
struct Lattice {
    cell: (f64, f64),
    offset: (f64, f64),
    /// Qué tono ocupa las casillas de paridad par.
    flip: bool,
}

impl Lattice {
    /// Coordenadas de la casilla que contiene el píxel.
    fn cell_at(&self, x: usize, y: usize) -> (i64, i64) {
        (
            ((x as f64 - self.offset.0) / self.cell.0).floor() as i64,
            ((y as f64 - self.offset.1) / self.cell.1).floor() as i64,
        )
    }
}

/// Busca la cuadrícula y, si la encuentra, deja transparentes sus píxeles.
///
/// Se analizan las parejas de grises más frecuentes y gana la que más imagen
/// cubre: un dibujo puede tener dos grises que se alternen por casualidad en
/// algún rincón, pero el fondo transparente ocupa mucho más.
pub fn remove(img: &mut RgbaImage) -> Option<Checkerboard> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let candidates = frequent_colors(img);

    let mut pairs = Vec::new();
    for i in 0..candidates.len() {
        for j in i + 1..candidates.len() {
            let ((a, na), (b, nb)) = (candidates[i], candidates[j]);
            if plausible_pair(a, b) {
                pairs.push((na + nb, a, b));
            }
        }
    }
    pairs.sort_by(|x, y| y.0.cmp(&x.0));

    let mut best: Option<Candidate> = None;
    for &(_, a, b) in pairs.iter().take(PAIRS) {
        let labels = label(img, a, b, MATCH);
        let Some(lattice) = fit(&labels, w, h) else {
            continue;
        };
        let (cells, matching) = qualifying_cells(&labels, w, h, lattice);
        if matching as f64 / (w * h) as f64 >= MIN_COVERAGE
            && best.as_ref().is_none_or(|found| matching > found.matching)
        {
            best = Some(Candidate {
                colors: (a, b),
                lattice,
                labels,
                cells,
                matching,
            });
        }
    }

    let found = best?;
    let erased = erase(img, &found.labels, w, h, found.lattice, &found.cells);
    Some(Checkerboard {
        cell: found.lattice.cell,
        colors: found.colors,
        coverage: erased as f64 / (w * h) as f64,
    })
}

/// Una pareja de tonos ya analizada, lista para aplicar.
struct Candidate {
    colors: (Rgba, Rgba),
    lattice: Lattice,
    labels: Vec<u8>,
    /// Casillas que son damero de principio a fin.
    cells: Vec<bool>,
    /// Píxeles que cuadran dentro de esas casillas; mide lo buena que es.
    matching: usize,
}

/// Los colores opacos más repetidos de la imagen, con su recuento.
fn frequent_colors(img: &RgbaImage) -> Vec<(Rgba, usize)> {
    let mut counts: HashMap<[u8; 4], usize> = HashMap::new();
    for pixel in img.pixels() {
        if pixel.0[3] == 255 {
            *counts.entry(pixel.0).or_insert(0) += 1;
        }
    }
    let mut list: Vec<([u8; 4], usize)> = counts.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    list.into_iter()
        .take(CANDIDATES)
        .map(|(c, n)| (Rgba::new(c[0], c[1], c[2], c[3]), n))
        .collect()
}

/// Dos grises parecidos pero distinguibles: el perfil de un damero.
fn plausible_pair(a: Rgba, b: Rgba) -> bool {
    let gray = |c: Rgba| {
        let (max, min) = (c.r.max(c.g).max(c.b) as i32, c.r.min(c.g).min(c.b) as i32);
        max - min <= MAX_SATURATION
    };
    let luma = |c: Rgba| (c.r as i32 * 30 + c.g as i32 * 59 + c.b as i32 * 11) / 100;
    gray(a) && gray(b) && CONTRAST.contains(&(luma(a) - luma(b)).abs())
}

/// Marca cada píxel con el tono del damero al que se parece: 0, 1 o 2 (ninguno).
fn label(img: &RgbaImage, a: Rgba, b: Rgba, margin: f64) -> Vec<u8> {
    img.pixels()
        .map(|p| {
            if p.0[3] != 255 {
                return 2;
            }
            let c = Rgba::new(p.0[0], p.0[1], p.0[2], p.0[3]);
            let (da, db) = (c.distance(&a), c.distance(&b));
            if da <= margin && da <= db {
                0
            } else if db <= margin {
                1
            } else {
                2
            }
        })
        .collect()
}

/// Tiras de color uniforme rodeadas por el otro tono. Las de los extremos están
/// cortadas y no miden la casilla, así que se descartan.
fn runs(labels: &[u8], w: usize, h: usize, horizontal: bool) -> Vec<(u32, u32)> {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let at = |line: usize, i: usize| -> u8 {
        labels[if horizontal {
            line * w + i
        } else {
            i * w + line
        }]
    };

    let mut out = Vec::new();
    for line in 0..lines {
        // Segmentos (etiqueta, inicio, largo) de la línea.
        let mut segments: Vec<(u8, u32, u32)> = Vec::new();
        let mut start = 0;
        for i in 1..=len {
            if i == len || at(line, i) != at(line, start) {
                segments.push((at(line, start), start as u32, (i - start) as u32));
                start = i;
            }
        }
        for k in 1..segments.len().saturating_sub(1) {
            let (label, from, length) = segments[k];
            let other = 1 - label;
            if label < 2 && segments[k - 1].0 == other && segments[k + 1].0 == other {
                out.push((from, length));
            }
        }
    }
    out
}

/// Valor más repetido, con su recuento.
fn mode(values: impl Iterator<Item = u32>) -> Option<(u32, usize, usize)> {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    let mut total = 0;
    for v in values {
        *counts.entry(v).or_insert(0) += 1;
        total += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(value, count)| (count, std::cmp::Reverse(value)))
        .map(|(value, count)| (value, count, total))
}

/// Deduce el tamaño de casilla y el encaje de la rejilla a partir de las tiras.
///
/// Un damero de verdad da tiras del mismo largo, en ambos ejes y todas alineadas
/// a la misma rejilla. Cualquier grieta en esas tres condiciones descarta la
/// pareja de colores: dos tonos del dibujo que se alternen sin más no las
/// cumplen, y colarlos costaría borrar parte del dibujo.
fn fit(labels: &[u8], w: usize, h: usize) -> Option<Lattice> {
    let rows = runs(labels, w, h, true);
    let cols = runs(labels, w, h, false);
    if rows.len() < MIN_RUNS || cols.len() < MIN_RUNS {
        return None;
    }
    let (cell_x, offset_x) = axis_fit(&rows)?;
    let (cell_y, offset_y) = axis_fit(&cols)?;

    // Las casillas son cuadradas; un reescalado las deforma un poco, no más.
    if (cell_x - cell_y).abs() > cell_x.max(cell_y) * 0.1 {
        return None;
    }
    let (cell, offset) = ((cell_x, cell_y), (offset_x, offset_y));

    // La paridad de las casillas puede empezar por cualquiera de los dos tonos.
    let score = |flip: bool| {
        let lattice = Lattice { cell, offset, flip };
        labels
            .iter()
            .enumerate()
            .filter(|&(i, &label)| label < 2 && label == expected(&lattice, i % w, i / w))
            .count()
    };
    Some(Lattice {
        cell,
        offset,
        flip: score(true) > score(false),
    })
}

/// Tamaño de casilla y fase de un eje, a partir de las tiras que mide.
///
/// El tamaño puede salir decimal, así que se admiten tiras de un píxel más o
/// menos que la moda y se promedian. La fase se saca en círculo (sumando cada
/// arranque como un ángulo), que es lo que tolera esos redondeos: su módulo
/// mide de paso lo bien alineadas que están.
fn axis_fit(runs: &[(u32, u32)]) -> Option<(f64, f64)> {
    let (peak, _, total) = mode(runs.iter().map(|r| r.1))?;
    let accepted: Vec<(u32, u32)> = runs
        .iter()
        .copied()
        .filter(|&(_, len)| len + 1 >= peak && len <= peak + 1)
        .collect();
    if (accepted.len() as f64) < total as f64 * UNIFORMITY {
        return None;
    }

    let n = accepted.len() as f64;
    let starts: Vec<f64> = accepted.iter().map(|&(start, _)| start as f64).collect();
    let coarse = accepted.iter().map(|&(_, len)| len as f64).sum::<f64>() / n;
    if coarse < 2.0 {
        return None;
    }

    // Promediar los largos deja el tamaño algo sesgado, y unas centésimas bastan
    // para que la fase se vaya al otro extremo de la imagen. Se afina buscando el
    // tamaño que más concentra los arranques.
    let (cell, (re, im)) = (0..=100)
        .map(|k| {
            let candidate = coarse - 0.5 + k as f64 / 100.0;
            (candidate, phase(&starts, candidate))
        })
        .max_by(|a, b| {
            let concentration = |(re, im): (f64, f64)| re.hypot(im);
            concentration(a.1).total_cmp(&concentration(b.1))
        })?;

    if re.hypot(im) / n < ALIGNMENT {
        return None;
    }
    let offset = (im.atan2(re) / std::f64::consts::TAU * cell).rem_euclid(cell);
    Some((cell, offset))
}

/// Suma de los arranques vistos como ángulos sobre un periodo. Su módulo mide lo
/// alineados que están, y su argumento dónde empieza la rejilla.
fn phase(starts: &[f64], cell: f64) -> (f64, f64) {
    let (mut re, mut im) = (0.0, 0.0);
    for start in starts {
        let angle = std::f64::consts::TAU * start / cell;
        re += angle.cos();
        im += angle.sin();
    }
    (re, im)
}

/// Tono que le toca a la casilla de esa posición.
fn expected(lattice: &Lattice, x: usize, y: usize) -> u8 {
    let (cx, cy) = lattice.cell_at(x, y);
    (((cx + cy) & 1) as u8) ^ u8::from(lattice.flip)
}

/// Índice de casilla de cada píxel, y las dimensiones del tablero.
fn cell_index(
    w: usize,
    h: usize,
    lattice: Lattice,
) -> (impl Fn(usize, usize) -> usize, usize, usize) {
    let origin = lattice.cell_at(0, 0);
    let last = lattice.cell_at(w - 1, h - 1);
    let cells_x = (last.0 - origin.0 + 1) as usize;
    let cells_y = (last.1 - origin.1 + 1) as usize;

    let index = move |x: usize, y: usize| -> usize {
        let (cx, cy) = lattice.cell_at(x, y);
        (cy - origin.1) as usize * cells_x + (cx - origin.0) as usize
    };
    (index, cells_x, cells_y)
}

/// Casillas que son damero de principio a fin, y cuántos píxeles suman.
///
/// Donde el dibujo pisa la cuadrícula, la casilla deja de cuadrar y se queda
/// entera: así un blanco suelto del dibujo no se lleva por delante.
///
/// No basta con que la casilla cuadre por su cuenta: un plano blanco del dibujo
/// —el ojo de un personaje, sin ir más lejos— cuadra perfectamente con las
/// casillas claras. Lo que distingue al fondo es la alternancia, así que se
/// exige que las vecinas, que esperan el otro tono, cuadren también.
fn qualifying_cells(labels: &[u8], w: usize, h: usize, lattice: Lattice) -> (Vec<bool>, usize) {
    let (index, cells_x, cells_y) = cell_index(w, h, lattice);
    let cells = cells_x * cells_y;
    let mut total = vec![0u32; cells];
    let mut hits = vec![0u32; cells];

    for y in 0..h {
        for x in 0..w {
            let i = index(x, y);
            total[i] += 1;
            if labels[y * w + x] == expected(&lattice, x, y) {
                hits[i] += 1;
            }
        }
    }

    let agrees: Vec<bool> = (0..cells)
        .map(|i| total[i] > 0 && hits[i] as f64 >= total[i] as f64 * CELL_AGREEMENT)
        .collect();

    let ok: Vec<bool> = (0..cells)
        .map(|i| {
            if !agrees[i] {
                return false;
            }
            let (cx, cy) = ((i % cells_x) as i64, (i / cells_x) as i64);
            let neighbours = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .filter(|&&(dx, dy)| {
                    let (nx, ny) = (cx + dx, cy + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < cells_x as i64
                        && ny < cells_y as i64
                        && agrees[ny as usize * cells_x + nx as usize]
                })
                .count();
            neighbours >= 2
        })
        .collect();

    let matching = (0..cells)
        .filter(|&i| ok[i])
        .map(|i| hits[i] as usize)
        .sum();
    (ok, matching)
}

/// Vacía el damero y devuelve cuántos píxeles ha tocado.
///
/// Se parte de las casillas confirmadas y se extiende por contigüidad a todo
/// píxel que siga el patrón. Borrar sólo las casillas confirmadas dejaría un
/// residuo con el periodo del damero, que luego despista a la detección de la
/// rejilla del dibujo; y lo que queda suelto —un blanco del dibujo que no toca
/// el fondo— se conserva.
fn erase(
    img: &mut RgbaImage,
    labels: &[u8],
    w: usize,
    h: usize,
    lattice: Lattice,
    cells: &[bool],
) -> usize {
    let (index, ..) = cell_index(w, h, lattice);
    let matches = |x: usize, y: usize| labels[y * w + x] == expected(&lattice, x, y);

    let mut seen = vec![false; w * h];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if cells[index(x, y)] && matches(x, y) && !seen[y * w + x] {
                seen[y * w + x] = true;
                stack.push((x, y));
            }
        }
    }

    let mut erased = 0;
    while let Some((x, y)) = stack.pop() {
        img.get_pixel_mut(x as u32, y as u32).0[3] = 0;
        erased += 1;
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < w && ny < h && !seen[ny * w + nx] && matches(nx, ny) {
                seen[ny * w + nx] = true;
                stack.push((nx, ny));
            }
        }
    }
    erased
}
