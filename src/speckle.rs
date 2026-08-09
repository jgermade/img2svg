//! Filtrado de motas: funde en una vecina las regiones que no aportan dibujo.
//!
//! Sin esto la salida no es utilizable. Una imagen real de 4 Mpx sale del
//! clustering con entre 18.000 y 31.000 regiones, y de ellas **el 68% son motas
//! de cuatro píxeles o menos** que suman el 0,5% del área. No es que abulten: es
//! que son un `<path>` cada una, y ningún editor vectorial abre eso.
//!
//! # Dos clases de mota, y una no la ve el área
//!
//! Mirando una conversión ampliada aparecen dos cosas distintas. Puntos sueltos,
//! que un umbral de área quita. Y **bandas de un píxel de ancho** siguiendo cada
//! borde de color, que son el reborde de antialias del original: una banda de
//! `1x8` tiene ocho píxeles, así que el umbral de área —lo que hace
//! `--filter-speckle` de VTracer— no la toca. Contado sobre el corpus: el 85% de
//! las regiones tienen un píxel de ancho o de alto, y **el 20% son delgadas y de
//! más de cuatro píxeles**. Filtrando sólo por área a `<=4`, de las ~4.100
//! regiones que sobreviven casi la mitad serían rebordes, que son justo los paths
//! más inútiles del documento.
//!
//! De ahí los dos criterios: área y grosor. Ver [`thickness`] para el segundo,
//! que es el que no existía en el plan.
//!
//! # Por qué antes de extraer las fronteras
//!
//! Fundir después obligaría a operar sobre la representación intermedia:
//! disolver la media arista que separa la mota de su vecina y reencadenar los
//! anillos de las dos caras, con el caso interesante de una mota cuya
//! desaparición junta dos anillos de su vecina en uno. Hacerlo sobre las
//! etiquetas es el mismo resultado sin nada de eso, y las fronteras se extraen
//! una sola vez, ya de etiquetas definitivas.

use std::collections::HashMap;

use crate::cluster::{self, Clustering, NONE};

/// Grosor estimado de una región, en píxeles: `2 * área / perímetro`.
///
/// Sale de que el perímetro de una banda de `1xL` es `2L+2`, así que la razón
/// vale ~0,5 sea la banda todo lo larga que sea, mientras que para un bloque
/// compacto de lado `s` vale `s/2` y crece con el tamaño. Da igual la
/// orientación, que es lo que le falta a medir la caja envolvente: una diagonal
/// de un píxel de ancho tiene la caja tan alta como larga.
///
/// | región | área | perímetro | grosor |
/// | --- | --- | --- | --- |
/// | un píxel | 1 | 4 | 0,5 |
/// | banda `1x8` | 8 | 18 | 0,9 |
/// | bloque `2x2` | 4 | 8 | 1,0 |
/// | bloque `3x3` | 9 | 12 | 1,5 |
/// | bloque `10x10` | 100 | 40 | 5,0 |
pub fn thickness(area: usize, perimeter: usize) -> f64 {
    if perimeter == 0 {
        return 0.0;
    }
    2.0 * area as f64 / perimeter as f64
}

/// Funde las motas de un etiquetado, dejándolo listo para extraer contornos.
///
/// Una región se funde si su área no pasa de `max_area` o si su grosor no llega a
/// `min_thickness`; con los dos a cero no hace nada. La vecina elegida es la que
/// **comparte más frontera** con ella, no la más grande: para una banda de un
/// píxel, la vecina grande de la que es reborde puede tocarla por un solo lado
/// mientras la comparte casi entera con otra.
///
/// Lo que **no** hace: una mota sin ninguna vecina visible se queda como está. Es
/// el caso de un punto suelto sobre fondo transparente, y la alternativa sería
/// borrarlo, que en una foto abre un agujero donde había dibujo.
pub fn filter(clustering: &mut Clustering, max_area: usize, min_thickness: f64) {
    let n = clustering.clusters.len();
    if n == 0 || (max_area == 0 && min_thickness <= 0.0) {
        return;
    }

    let perimeter = perimeters(clustering);
    let doomed: Vec<bool> = (0..n)
        .map(|i| {
            let area = clustering.clusters[i].area;
            area <= max_area || thickness(area, perimeter[i]) < min_thickness
        })
        .collect();
    if !doomed.iter().any(|&d| d) {
        return;
    }

    let best = best_neighbours(clustering, &doomed);

    // De menor a mayor área: así una mota rodeada de motas se pega a la más
    // grande de ellas y viaja con ella cuando esa encuentre una superviviente,
    // en vez de hacer falta repetir pasadas.
    let mut order: Vec<u32> = (0..n as u32).filter(|&i| doomed[i as usize]).collect();
    order.sort_by_key(|&i| (clustering.clusters[i as usize].area, i));

    let mut sets = Sets::new(n);
    for &i in &order {
        let root = sets.find(i);
        // Si la raíz ya no es una mota, este grupo ya está fundido en una
        // superviviente y no hay que arrastrarlo a ningún otro sitio.
        if !doomed[root as usize] {
            continue;
        }
        let Some(target) = best[i as usize] else {
            continue;
        };
        let target = sets.find(target);
        if target != root {
            sets.merge_into(root, target);
        }
    }

    let root_of: Vec<u32> = (0..n as u32).map(|i| sets.find(i)).collect();
    // El color de la región resultante es el de su raíz, y la raíz es siempre la
    // superviviente porque las uniones van en ese sentido.
    let color_of: Vec<crate::color::Rgba> = clustering.clusters.iter().map(|k| k.color).collect();

    let labels = std::mem::take(&mut clustering.labels);
    *clustering = cluster::gather(
        labels,
        clustering.width,
        clustering.height,
        &root_of,
        &color_of,
    );
}

/// El perímetro de cada región: sus lados de píxel que dan a otra etiqueta o al
/// borde de la imagen.
fn perimeters(c: &Clustering) -> Vec<usize> {
    let (w, h) = (c.width, c.height);
    let mut out = vec![0usize; c.clusters.len()];
    for y in 0..h {
        for x in 0..w {
            let label = c.labels[y * w + x];
            if label == NONE {
                continue;
            }
            let mut sides = 0;
            if y == 0 || c.labels[(y - 1) * w + x] != label {
                sides += 1;
            }
            if x == 0 || c.labels[y * w + x - 1] != label {
                sides += 1;
            }
            if y + 1 == h || c.labels[(y + 1) * w + x] != label {
                sides += 1;
            }
            if x + 1 == w || c.labels[y * w + x + 1] != label {
                sides += 1;
            }
            out[label as usize] += sides;
        }
    }
    out
}

/// Para cada mota, la vecina con la que comparte más frontera.
///
/// Se acumula sólo lo que toca a una mota: en una imagen grande la mayoría de las
/// parejas de vecinas no interesan, y guardarlas todas es memoria a cambio de
/// nada.
fn best_neighbours(c: &Clustering, doomed: &[bool]) -> Vec<Option<u32>> {
    let (w, h) = (c.width, c.height);
    let mut shared: HashMap<(u32, u32), usize> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let a = c.labels[y * w + x];
            if a == NONE {
                continue;
            }
            // Sólo hacia la derecha y hacia abajo, para no contar cada frontera
            // dos veces.
            for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                if nx == w || ny == h {
                    continue;
                }
                let b = c.labels[ny * w + nx];
                if b == NONE || b == a {
                    continue;
                }
                if !doomed[a as usize] && !doomed[b as usize] {
                    continue;
                }
                let key = if a < b { (a, b) } else { (b, a) };
                *shared.entry(key).or_insert(0) += 1;
            }
        }
    }

    // (frontera compartida, área de la vecina, vecina) del mejor candidato.
    let mut best: Vec<Option<(usize, usize, u32)>> = vec![None; c.clusters.len()];
    let mut consider = |mota: u32, vecina: u32, frontera: usize| {
        if !doomed[mota as usize] {
            return;
        }
        // Más frontera compartida gana; a igualdad, la vecina más grande, y a
        // igualdad de las dos, la de menor índice: sin este desempate el
        // resultado dependería del recorrido de la tabla hash.
        let candidata = (frontera, c.clusters[vecina as usize].area);
        let entry = &mut best[mota as usize];
        let gana = match entry {
            None => true,
            Some((f, a, v)) => candidata > (*f, *a) || (candidata == (*f, *a) && vecina < *v),
        };
        if gana {
            *entry = Some((candidata.0, candidata.1, vecina));
        }
    };
    for (&(a, b), &frontera) in &shared {
        consider(a, b, frontera);
        consider(b, a, frontera);
    }

    best.into_iter().map(|b| b.map(|(_, _, v)| v)).collect()
}

/// Conjuntos disjuntos con la raíz **impuesta**, que es lo que distingue esto del
/// union-find del etiquetado: ahí gana el conjunto más grande, y aquí tiene que
/// ganar la vecina, porque su color es el que se queda. Sin la heurística de
/// tamaño los árboles pueden crecer más, y de eso se ocupa la compresión de
/// caminos.
struct Sets {
    parent: Vec<u32>,
}

impl Sets {
    fn new(n: usize) -> Self {
        Sets {
            parent: (0..n as u32).collect(),
        }
    }

    fn find(&mut self, mut x: u32) -> u32 {
        while self.parent[x as usize] != x {
            let parent = self.parent[x as usize];
            self.parent[x as usize] = self.parent[parent as usize];
            x = self.parent[x as usize];
        }
        x
    }

    /// Cuelga la raíz `loser` de la raíz `winner`.
    fn merge_into(&mut self, loser: u32, winner: u32) {
        debug_assert_eq!(self.parent[loser as usize], loser);
        debug_assert_eq!(self.parent[winner as usize], winner);
        self.parent[loser as usize] = winner;
    }
}
