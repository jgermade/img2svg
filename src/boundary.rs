//! De la imagen etiquetada a la representación intermedia de medias aristas.
//!
//! El clustering ([`crate::cluster`]) deja una etiqueta de región por píxel. Esto
//! saca de ahí los contornos, con **cada frontera una sola vez** y sabiendo qué
//! región queda a cada lado, que es lo que pide [`crate::region`].
//!
//! # Por qué no sirve el trazado de la rejilla
//!
//! [`crate::trace::trace`] traza una máscara: se le da un conjunto de píxeles y
//! devuelve sus bucles. Para usarlo hay que llamarlo una vez por región, con una
//! máscara del tamaño de la imagen cada vez —que es lo que hace
//! [`crate::segment::from_pixel_map`]—, y con decenas de miles de regiones eso no
//! termina. Y aunque terminara, cada región se trazaría por su cuenta: la
//! frontera entre dos vecinas saldría dos veces, y no habría dónde anotar quién
//! está al otro lado.
//!
//! # Grietas, nodos y cadenas
//!
//! Se mira la retícula de esquinas de píxel, `(w+1) x (h+1)`. Cada **grieta** es
//! un segmento unidad entre dos esquinas contiguas, y separa dos píxeles; es
//! frontera si los dos no tienen la misma etiqueta. El exterior y lo
//! transparente cuentan como una etiqueta más, así que el borde de la imagen sale
//! solo.
//!
//! Las grietas de frontera forman un grafo plano sobre la retícula, donde cada
//! esquina tiene grado 0, 2, 3 o 4 —nunca 1, porque las diferencias de etiqueta
//! forman curvas cerradas—. Las de grado 3 o 4 son **nodos**: ahí se juntan tres
//! o cuatro regiones y la frontera se bifurca. Entre nodo y nodo va una
//! **cadena**, y una cadena es exactamente una media arista.
//!
//! Lo que hace que esto funcione es que **en una esquina de grado 2 las dos
//! grietas separan el mismo par de regiones**, sigan recto o giren. Se comprueba
//! por casos: si de las cuatro grietas de la esquina sólo dos son frontera, las
//! otras dos tienen iguales sus dos píxeles, y eso obliga a que los tres o cuatro
//! píxeles de alrededor sean sólo dos valores distintos, repartidos justo de la
//! forma que deja el mismo par a un lado y a otro. Por eso una cadena tiene un
//! par `(left, right)` bien definido en toda su longitud, y por eso se puede
//! ajustar una sola vez para las dos caras. En depuración hay una comprobación
//! que lo verifica grieta a grieta, para que sea un hecho y no un argumento.

use std::collections::HashMap;

use crate::cluster::{Clustering, NONE};
use crate::region::{EdgeId, HalfEdge, Region, Regions, Ring};
use crate::trace::Point;

/// Extrae los contornos de una imagen ya etiquetada.
///
/// Las regiones salen en el mismo orden que traían las del clustering, que es el
/// de emisión: los colores más presentes primero y los de un color seguidos.
pub fn from_clustering(clustering: &Clustering) -> Regions {
    let mut cracks = Cracks::new(clustering);
    let edges = cracks.chains();
    let regions = assemble(clustering, &edges);
    Regions {
        width: clustering.width,
        height: clustering.height,
        colors: clustering.colors,
        regions,
        edges,
    }
}

/// Una grieta, por la esquina en la que empieza.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Crack {
    /// De `(x, y)` a `(x+1, y)`. Separa el píxel de arriba del de abajo.
    H(usize, usize),
    /// De `(x, y)` a `(x, y+1)`. Separa el de la izquierda del de la derecha.
    V(usize, usize),
}

struct Cracks<'a> {
    w: usize,
    h: usize,
    labels: &'a [u32],
    /// Las horizontales primero y las verticales después, en un solo vector.
    used: Vec<bool>,
    /// Esquinas donde la frontera se bifurca, es decir de grado 3 o 4.
    node: Vec<bool>,
}

impl<'a> Cracks<'a> {
    fn new(clustering: &'a Clustering) -> Self {
        let (w, h) = (clustering.width, clustering.height);
        let mut cracks = Cracks {
            w,
            h,
            labels: &clustering.labels,
            used: vec![false; w * (h + 1) + (w + 1) * h],
            node: vec![false; (w + 1) * (h + 1)],
        };
        let mut buf = [Crack::H(0, 0); 4];
        for ly in 0..=h {
            for lx in 0..=w {
                let degree = cracks.incident(lx, ly, &mut buf);
                cracks.node[ly * (w + 1) + lx] = degree > 2;
            }
        }
        cracks
    }

    /// La etiqueta de un píxel. Fuera de la imagen es [`NONE`], igual que lo
    /// transparente: para el contorno el exterior no es un caso aparte.
    fn label(&self, x: i64, y: i64) -> u32 {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            NONE
        } else {
            self.labels[y as usize * self.w + x as usize]
        }
    }

    /// Las etiquetas a izquierda y derecha de una grieta recorrida en su sentido
    /// creciente.
    ///
    /// Con la `y` hacia abajo, la izquierda de un avance `(dx, dy)` es `(dy, -dx)`:
    /// yendo en `+x` es el píxel de arriba, y yendo en `+y` el de la derecha.
    fn sides(&self, crack: Crack) -> (u32, u32) {
        match crack {
            Crack::H(x, y) => (
                self.label(x as i64, y as i64 - 1),
                self.label(x as i64, y as i64),
            ),
            Crack::V(x, y) => (
                self.label(x as i64, y as i64),
                self.label(x as i64 - 1, y as i64),
            ),
        }
    }

    fn is_boundary(&self, crack: Crack) -> bool {
        let (left, right) = self.sides(crack);
        left != right
    }

    fn ends(&self, crack: Crack) -> (Point, Point) {
        match crack {
            Crack::H(x, y) => ((x as i32, y as i32), (x as i32 + 1, y as i32)),
            Crack::V(x, y) => ((x as i32, y as i32), (x as i32, y as i32 + 1)),
        }
    }

    fn index(&self, crack: Crack) -> usize {
        match crack {
            Crack::H(x, y) => y * self.w + x,
            Crack::V(x, y) => self.w * (self.h + 1) + y * (self.w + 1) + x,
        }
    }

    /// Las grietas de frontera que tocan una esquina, y cuántas son.
    fn incident(&self, lx: usize, ly: usize, out: &mut [Crack; 4]) -> usize {
        let mut n = 0;
        let consider = |cracks: &Self, crack: Crack, out: &mut [Crack; 4], n: &mut usize| {
            if cracks.is_boundary(crack) {
                out[*n] = crack;
                *n += 1;
            }
        };
        if lx > 0 {
            consider(self, Crack::H(lx - 1, ly), out, &mut n);
        }
        if lx < self.w {
            consider(self, Crack::H(lx, ly), out, &mut n);
        }
        if ly > 0 {
            consider(self, Crack::V(lx, ly - 1), out, &mut n);
        }
        if ly < self.h {
            consider(self, Crack::V(lx, ly), out, &mut n);
        }
        n
    }

    fn is_node(&self, at: Point) -> bool {
        self.node[at.1 as usize * (self.w + 1) + at.0 as usize]
    }

    /// Todas las medias aristas de la imagen.
    fn chains(&mut self) -> Vec<HalfEdge> {
        let mut edges = Vec::new();
        // Primero las cadenas que van de nodo a nodo, porque es donde tienen que
        // partirse: sólo ahí cambia el par de regiones. Después lo que quede son
        // bucles que no pasan por ningún nodo —una región suelta sobre un fondo
        // uniforme no tiene por dónde bifurcarse— y se parten por donde caiga.
        self.sweep(true, &mut edges);
        self.sweep(false, &mut edges);
        edges
    }

    fn sweep(&mut self, only_nodes: bool, edges: &mut Vec<HalfEdge>) {
        let mut buf = [Crack::H(0, 0); 4];
        for ly in 0..=self.h {
            for lx in 0..=self.w {
                if only_nodes && !self.node[ly * (self.w + 1) + lx] {
                    continue;
                }
                let n = self.incident(lx, ly, &mut buf);
                for &crack in &buf[..n] {
                    if !self.used[self.index(crack)] {
                        let edge = self.walk((lx as i32, ly as i32), crack);
                        edges.push(edge);
                    }
                }
            }
        }
    }

    /// Sigue una cadena desde una esquina hasta el siguiente nodo, o hasta
    /// cerrarse sobre el punto de partida.
    fn walk(&mut self, start: Point, first: Crack) -> HalfEdge {
        // El par de regiones se fija con la primera grieta y vale para toda la
        // cadena. Si se entra por el extremo final de la grieta, los lados van
        // al revés que en su sentido creciente.
        let (left, right) = self.oriented(first, start);

        let mut points = vec![start];
        let mut buf = [Crack::H(0, 0); 4];
        let mut crack = first;
        let mut at = start;
        loop {
            debug_assert_eq!(
                self.oriented(crack, at),
                (left, right),
                "la cadena ha cambiado de par de regiones en {at:?}"
            );
            let index = self.index(crack);
            self.used[index] = true;
            let (a, b) = self.ends(crack);
            let next = if at == a { b } else { a };
            points.push(next);
            if next == start || self.is_node(next) {
                break;
            }
            // Grado 2: hay exactamente otra grieta, y es la continuación.
            let n = self.incident(next.0 as usize, next.1 as usize, &mut buf);
            debug_assert_eq!(n, 2, "esquina de grado {n} que no es nodo, en {next:?}");
            crack = if buf[0] == crack { buf[1] } else { buf[0] };
            at = next;
        }

        // `right` es el exterior, así que si lo transparente ha caído a la
        // izquierda hay que dar la vuelta a la cadena.
        let (left, right) = if left == NONE {
            points.reverse();
            (right, left)
        } else {
            (left, right)
        };
        HalfEdge {
            points,
            // Los pone [`crate::subpixel`] después, si se piden: aquí sólo se
            // sabe de grietas, y dónde cae el borde dentro de un píxel es una
            // pregunta para la imagen.
            offsets: Vec::new(),
            left: left as usize,
            right: (right != NONE).then_some(right as usize),
        }
    }

    /// Los lados de una grieta recorrida saliendo de `from`.
    fn oriented(&self, crack: Crack, from: Point) -> (u32, u32) {
        let (left, right) = self.sides(crack);
        if from == self.ends(crack).0 {
            (left, right)
        } else {
            (right, left)
        }
    }
}

/// Encadena las medias aristas de cada región en sus anillos.
///
/// Una región aparece como `left` de unas y como `right` de otras; en las
/// segundas su contorno recorre el tramo al revés, que es lo que marca el `bool`
/// del anillo. Al recorrerlas siempre en el sentido en que la región queda a la
/// izquierda, los anillos salen todos con la misma orientación.
fn assemble(clustering: &Clustering, edges: &[HalfEdge]) -> Vec<Region> {
    let mut uses: Vec<Vec<(EdgeId, bool)>> = vec![Vec::new(); clustering.clusters.len()];
    for (id, edge) in edges.iter().enumerate() {
        uses[edge.left].push((id, false));
        if let Some(right) = edge.right {
            uses[right].push((id, true));
        }
    }

    uses.iter()
        .zip(&clustering.clusters)
        .map(|(uses, cluster)| Region {
            color: cluster.color,
            area: cluster.area,
            rings: rings(edges, uses),
        })
        .collect()
}

/// Reparte los tramos de una cara en anillos cerrados.
fn rings(edges: &[HalfEdge], uses: &[(EdgeId, bool)]) -> Vec<Ring> {
    let mut by_start: HashMap<Point, Vec<usize>> = HashMap::new();
    for (i, &u) in uses.iter().enumerate() {
        by_start.entry(start_of(edges, u)).or_default().push(i);
    }

    let mut done = vec![false; uses.len()];
    let mut out = Vec::new();
    for seed in 0..uses.len() {
        if done[seed] {
            continue;
        }
        let mut ring: Ring = Vec::new();
        let opening = start_of(edges, uses[seed]);
        let mut i = seed;
        loop {
            done[i] = true;
            ring.push(uses[i]);
            let end = end_of(edges, uses[i]);
            // El anillo se cierra al volver a donde empezó, y ahí se corta aunque
            // queden tramos sin usar que salgan de este mismo punto. Seguir
            // enganchándolos daría un anillo en forma de ocho: es lo que pasa con
            // dos trozos de la misma región que sólo se tocan por una esquina, que
            // son dos cadenas cerradas distintas y las dos empiezan y acaban ahí.
            // El relleno par-impar pintaría igual el ocho, pero cada trozo tiene
            // que ser su propio anillo para que un ajustador de curvas pueda
            // cerrar cada uno por su cuenta.
            if end == opening {
                break;
            }
            let incoming = last_step(edges, uses[i]);
            // En una esquina donde la región se toca consigo misma salen dos
            // continuaciones. Se prefiere el giro más cerrado a la izquierda,
            // igual que en `trace::pick_next`: eso separa los píxeles que sólo se
            // tocan en diagonal en anillos distintos, que es lo que hace que el
            // relleno par-impar los pinte bien.
            let next = by_start.get(&end).and_then(|candidates| {
                candidates
                    .iter()
                    .copied()
                    .filter(|&c| !done[c])
                    .min_by_key(|&c| turn(incoming, first_step(edges, uses[c])))
            });
            match next {
                Some(next) => i = next,
                // No debería pasar: el contorno de una cara son curvas cerradas,
                // así que desde cualquier tramo se vuelve al principio.
                None => {
                    debug_assert!(false, "anillo abierto en {end:?}");
                    break;
                }
            }
        }
        out.push(ring);
    }
    out
}

fn start_of(edges: &[HalfEdge], (edge, reversed): (EdgeId, bool)) -> Point {
    let points = &edges[edge].points;
    if reversed {
        points[points.len() - 1]
    } else {
        points[0]
    }
}

fn end_of(edges: &[HalfEdge], (edge, reversed): (EdgeId, bool)) -> Point {
    let points = &edges[edge].points;
    if reversed {
        points[0]
    } else {
        points[points.len() - 1]
    }
}

/// El primer paso del tramo tal como se recorre.
fn first_step(edges: &[HalfEdge], (edge, reversed): (EdgeId, bool)) -> Point {
    let points = &edges[edge].points;
    let n = points.len();
    if reversed {
        delta(points[n - 1], points[n - 2])
    } else {
        delta(points[0], points[1])
    }
}

/// El último paso del tramo tal como se recorre.
fn last_step(edges: &[HalfEdge], (edge, reversed): (EdgeId, bool)) -> Point {
    let points = &edges[edge].points;
    let n = points.len();
    if reversed {
        delta(points[1], points[0])
    } else {
        delta(points[n - 2], points[n - 1])
    }
}

fn delta(a: Point, b: Point) -> Point {
    (b.0 - a.0, b.1 - a.1)
}

/// Cuánto gira una continuación respecto a lo que se traía: la izquierda
/// primero. Mismo criterio que [`crate::trace`].
fn turn(incoming: Point, outgoing: Point) -> u8 {
    let left = (incoming.1, -incoming.0);
    let right = (-incoming.1, incoming.0);
    if outgoing == left {
        0
    } else if outgoing == incoming {
        1
    } else if outgoing == right {
        2
    } else {
        3
    }
}
