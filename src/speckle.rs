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
//! # El grosor no basta, y por poco no se lleva el dibujo
//!
//! Un reborde de antialias es delgado, pero **un trazo de tinta también**: en una
//! ilustración de 300 px de lado los trazos miden uno o dos píxeles, así que el
//! umbral de grosor que quita los rebordes se lleva las gafas, la boca y las
//! cejas. Medido en la portada de un disco: con el umbral puesto los aros de las
//! gafas salen a guiones; sin él vuelven, y vuelve con ellos una orla marrón
//! alrededor de cada trazo negro.
//!
//! Lo que separa las dos cosas no es la geometría, es el **color**, y es la misma
//! cuenta que hace [`crate::subpixel`] un nivel más abajo:
//!
//! > un reborde es una **mezcla** de sus dos vecinas —su color cae sobre el
//! > segmento que las une—, y un trazo no: el negro entre una piel y un fondo
//! > amarillo no se parece a ninguna mezcla de piel y amarillo.
//!
//! Así que el grosor sólo **propone** y la mezcla decide. Y el caso de una vecina
//! sola sale sin ponerle una regla aparte: el segmento degenera en un punto, la
//! distancia es al color de esa vecina, y una línea fina dentro de una zona lisa
//! —la boca sobre la piel— está lejísimos de él y se queda.
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
use crate::color::Oklab;

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

/// Cuánto puede apartarse del segmento entre sus dos vecinas el color de una
/// región para seguir siendo una mezcla de ellas, en múltiplos de `tolerance`.
///
/// Una tolerancia, que es lo que la paleta promete: ningún píxel se pinta a más de
/// `tolerance` de su color, así que la entrada de un reborde de verdad no puede
/// estar más lejos que eso de la mezcla que lo generó. Más ancho empieza a admitir
/// tinta —un gris medio entre negro y blanco *es* una mezcla— y más estrecho deja
/// pasar la orla, que es lo que hay que quitar.
const MIX_CEILING: f64 = 3.0;

/// Funde las motas de un etiquetado, dejándolo listo para extraer contornos.
///
/// Una región se funde si su área no pasa de `max_area`, o si es delgada —grosor
/// por debajo de `min_thickness`— **y** su color es una mezcla de sus dos vecinas
/// principales; con `max_area` y `min_thickness` a cero no hace nada. Lo segundo es
/// lo que distingue un reborde de antialias de un trazo fino, que mide lo mismo:
/// ver la cabecera del módulo.
///
/// La vecina elegida es la que **comparte más frontera**, no la más grande: para
/// una banda de un píxel, la vecina grande de la que es reborde puede tocarla por
/// un solo lado mientras la comparte casi entera con otra. En un reborde va a la
/// más parecida en color de las dos, que es la que menos error mete.
///
/// Lo que **no** hace: una mota sin ninguna vecina visible se queda como está. Es
/// el caso de un punto suelto sobre fondo transparente, y la alternativa sería
/// borrarlo, que abre un agujero donde había dibujo.
pub fn filter(clustering: &mut Clustering, max_area: usize, min_thickness: f64, tolerance: f64) {
    let n = clustering.clusters.len();
    if n == 0 || (max_area == 0 && min_thickness <= 0.0) {
        return;
    }

    let perimeter = perimeters(clustering);
    // Primero los candidatos, que es geometría y no depende de nadie; el test de
    // mezcla necesita saber quiénes son las vecinas, y saberlo pide esta lista.
    let small: Vec<bool> = (0..n)
        .map(|i| clustering.clusters[i].area <= max_area)
        .collect();
    let candidate: Vec<bool> = (0..n)
        .map(|i| {
            let area = clustering.clusters[i].area;
            small[i] || thickness(area, perimeter[i]) < min_thickness
        })
        .collect();
    if !candidate.iter().any(|&d| d) {
        return;
    }

    let sides = best_neighbours(clustering, &candidate);
    let lab: Vec<Oklab> = clustering
        .clusters
        .iter()
        .map(|k| Oklab::from_rgba(k.color))
        .collect();

    // El área condena por sí sola; el grosor sólo propone, y decide la mezcla.
    let mut best: Vec<Option<u32>> = vec![None; n];
    let doomed: Vec<bool> = (0..n)
        .map(|i| {
            let Some(first) = sides[i][0] else {
                return false;
            };
            if small[i] {
                best[i] = Some(first);
                return true;
            }
            let second = sides[i][1].unwrap_or(first);
            match mixture(lab[i], lab[first as usize], lab[second as usize]) {
                Some(mix) if mix.gap <= MIX_CEILING * tolerance => {
                    // A la más parecida de las dos: fundir un reborde en la vecina
                    // de la que está más lejos en color sería meter el error entero
                    // teniendo la mitad al lado.
                    let closer = if lab[i].distance(&lab[first as usize])
                        <= lab[i].distance(&lab[second as usize])
                    {
                        first
                    } else {
                        second
                    };
                    best[i] = Some(closer);
                    true
                }
                _ => false,
            }
        })
        .collect();
    if !doomed.iter().any(|&d| d) {
        return;
    }

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

/// Dónde cae un color respecto del segmento que une a otros dos.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mix {
    /// Distancia al segmento: cuánto le falta para ser una mezcla de los dos.
    pub gap: f64,
    /// Qué parte del camino lleva del primero al segundo, recortada a `0..1`. Cerca
    /// de un extremo el color **es** ese extremo y no una mezcla de nada.
    pub t: f64,
}

/// La distancia del color de una región al segmento que une a sus dos vecinas, o
/// `None` si las tres son el mismo color y no hay nada que decidir.
///
/// Es la proyección de [`crate::subpixel`] pedida de una región en vez de de un
/// píxel, y en Oklab en vez de en sRGB: allí se deshace una mezcla concreta —hecha
/// sobre los números del fichero— y aquí se pregunta si dos colores *podrían*
/// haberla hecho, que es una pregunta perceptual y se mide donde una tolerancia
/// quiere decir lo mismo en todo el espacio.
///
/// Con las dos vecinas iguales el segmento degenera en un punto y esto es la
/// distancia a ese punto, que es exactamente lo que hay que preguntar de una línea
/// fina dentro de una zona lisa.
pub(crate) fn mixture(p: Oklab, a: Oklab, b: Oklab) -> Option<Mix> {
    let (ab, ap) = (
        [b.l - a.l, b.a - a.a, b.b - a.b],
        [p.l - a.l, p.a - a.a, p.b - a.b],
    );
    let len2: f32 = ab.iter().map(|v| v * v).sum();
    // Fuera del segmento el punto más cercano es el extremo, así que el parámetro
    // se recorta: un color que se pasa de largo de una vecina no es más mezcla por
    // estar alineado con ellas.
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (ap.iter().zip(ab).map(|(u, v)| u * v).sum::<f32>() / len2).clamp(0.0, 1.0)
    };
    let d2: f32 = ap
        .iter()
        .zip(ab)
        .map(|(u, v)| {
            let d = u - t * v;
            d * d
        })
        .sum();
    // El alfa no entra en la proyección —Oklab no lo cubre— pero sí decide: una
    // región semitransparente no es mezcla de dos opacas.
    if (p.alpha - a.alpha).abs() > f32::EPSILON && (p.alpha - b.alpha).abs() > f32::EPSILON {
        return None;
    }
    Some(Mix {
        gap: d2.sqrt() as f64,
        t: t as f64,
    })
}

/// Para cada mota, las dos vecinas con las que comparte más frontera.
///
/// Dos y no una porque el test de mezcla necesita el segmento: una sola vecina no
/// dice si el color de la región está *entre* algo. La primera es además la que se
/// queda con ella cuando se funde por área.
///
/// Se acumula sólo lo que toca a una mota: en una imagen grande la mayoría de las
/// parejas de vecinas no interesan, y guardarlas todas es memoria a cambio de
/// nada.
fn best_neighbours(c: &Clustering, doomed: &[bool]) -> Vec<[Option<u32>; 2]> {
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

    // Las dos mejores por mota: (frontera compartida, área de la vecina, vecina).
    let mut best: Vec<[Option<(usize, usize, u32)>; 2]> = vec![[None; 2]; c.clusters.len()];
    let mut consider = |mota: u32, vecina: u32, frontera: usize| {
        if !doomed[mota as usize] {
            return;
        }
        // Más frontera compartida gana; a igualdad, la vecina más grande, y a
        // igualdad de las dos, la de menor índice: sin este desempate el
        // resultado dependería del recorrido de la tabla hash.
        let candidata = (frontera, c.clusters[vecina as usize].area, vecina);
        let mejor_que = |otra: &Option<(usize, usize, u32)>| match otra {
            None => true,
            Some((f, a, v)) => {
                (candidata.0, candidata.1) > (*f, *a)
                    || ((candidata.0, candidata.1) == (*f, *a) && vecina < *v)
            }
        };
        let entry = &mut best[mota as usize];
        if mejor_que(&entry[0]) {
            entry[1] = entry[0];
            entry[0] = Some(candidata);
        } else if mejor_que(&entry[1]) {
            entry[1] = Some(candidata);
        }
    };
    for (&(a, b), &frontera) in &shared {
        consider(a, b, frontera);
        consider(b, a, frontera);
    }

    best.into_iter()
        .map(|pair| pair.map(|b| b.map(|(_, _, v)| v)))
        .collect()
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
