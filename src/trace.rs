//! Trazado de contornos: convierte una máscara de píxeles en polígonos cerrados.
//!
//! Cada píxel `(x, y)` ocupa el cuadrado `[x, x+1] x [y, y+1]`. Se emiten los
//! lados que separan un píxel de la máscara de uno que no lo está, orientados de
//! forma coherente (el interior siempre queda a la izquierda del avance), y
//! luego se encadenan por sus extremos hasta cerrar los bucles.

use std::collections::HashMap;

pub type Point = (i32, i32);

struct Edge {
    from: Point,
    to: Point,
    used: bool,
}

/// Devuelve los bucles cerrados del contorno de la máscara, **densos**: un
/// punto por cada esquina de píxel recorrida, sin colapsar los tramos rectos.
/// Colapsarlos es cosa del ajustador de píxel ([`crate::fit`]); los de curvas
/// necesitan los puntos intermedios para estimar tangentes.
///
/// Incluye contornos exteriores y agujeros; se rellenan con
/// `fill-rule="evenodd"`.
pub fn trace(mask: &[bool], w: usize, h: usize) -> Vec<Vec<Point>> {
    let at = |x: i64, y: i64| -> bool {
        x >= 0 && y >= 0 && x < w as i64 && y < h as i64 && mask[y as usize * w + x as usize]
    };

    let mut edges: Vec<Edge> = Vec::new();
    let mut outgoing: HashMap<Point, Vec<usize>> = HashMap::new();
    let push = |edges: &mut Vec<Edge>,
                outgoing: &mut HashMap<Point, Vec<usize>>,
                from: Point,
                to: Point| {
        outgoing.entry(from).or_default().push(edges.len());
        edges.push(Edge {
            from,
            to,
            used: false,
        });
    };

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if !at(x as i64, y as i64) {
                continue;
            }
            let (x0, y0, x1, y1) = (x, y, x + 1, y + 1);
            if !at(x as i64, y as i64 - 1) {
                push(&mut edges, &mut outgoing, (x1, y0), (x0, y0));
            }
            if !at(x as i64 - 1, y as i64) {
                push(&mut edges, &mut outgoing, (x0, y0), (x0, y1));
            }
            if !at(x as i64, y as i64 + 1) {
                push(&mut edges, &mut outgoing, (x0, y1), (x1, y1));
            }
            if !at(x as i64 + 1, y as i64) {
                push(&mut edges, &mut outgoing, (x1, y1), (x1, y0));
            }
        }
    }

    let mut loops = Vec::new();
    for start in 0..edges.len() {
        if edges[start].used {
            continue;
        }
        let mut points = Vec::new();
        let mut current = start;
        loop {
            edges[current].used = true;
            points.push(edges[current].from);
            let dir = delta(edges[current].from, edges[current].to);
            let next = match pick_next(&edges, &outgoing, edges[current].to, dir) {
                Some(n) => n,
                None => break,
            };
            if next == start {
                break;
            }
            current = next;
        }
        if points.len() >= 4 {
            loops.push(points);
        }
    }
    loops
}

/// Reparte los píxeles de la máscara en regiones conexas, devolviendo los
/// índices de cada una.
///
/// La vecindad es de 8: dos píxeles que sólo se tocan por la esquina forman una
/// sola región, que es lo que uno espera de una diagonal de pixel art. El
/// trazado sí las separa en bucles distintos, pero acaban en el mismo `<path>`.
pub fn components(mask: &[bool], w: usize, h: usize) -> Vec<Vec<usize>> {
    let mut seen = vec![false; mask.len()];
    let mut regions = Vec::new();
    let mut stack = Vec::new();

    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let mut region = Vec::new();

        while let Some(i) = stack.pop() {
            region.push(i);
            let (x, y) = ((i % w) as i64, (i / w) as i64);
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx == 0 && dy == 0) || nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64
                    {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if mask[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        region.sort_unstable();
        regions.push(region);
    }
    regions
}

fn delta(a: Point, b: Point) -> Point {
    (b.0 - a.0, b.1 - a.1)
}

/// Elige la continuación del contorno. En los cruces en diagonal hay dos
/// candidatas: se prefiere el giro más cerrado hacia la izquierda, lo que separa
/// los píxeles diagonales en bucles distintos (conectividad-4).
fn pick_next(
    edges: &[Edge],
    outgoing: &HashMap<Point, Vec<usize>>,
    at: Point,
    incoming: Point,
) -> Option<usize> {
    let left = (incoming.1, -incoming.0);
    let right = (-incoming.1, incoming.0);
    outgoing
        .get(&at)?
        .iter()
        .copied()
        .filter(|&i| !edges[i].used)
        .min_by_key(|&i| {
            let d = delta(edges[i].from, edges[i].to);
            if d == left {
                0
            } else if d == incoming {
                1
            } else if d == right {
                2
            } else {
                3
            }
        })
}
