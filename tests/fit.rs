//! Ajuste: del contorno de una región a los datos de un `<path>`.
//!
//! Se prueba **sobre el documento emitido** y no sobre las estructuras
//! internas: lo que tiene que cumplirse es una propiedad de la geometría que se
//! dibuja, y leerla del `d` de cada path comprueba de paso que lo escrito se
//! puede volver a leer.

// Sólo la comprobación de costuras necesita las tablas, y sólo existe con la
// segmentación que comparte fronteras entre regiones.
#[cfg(feature = "illustration")]
use std::collections::{HashMap, HashSet};

use img2svg::{Config, Conversion, Fit, GridOptions};

type Point = (f64, f64);

/// Un tramo dibujado: de dónde a dónde, y con qué controles si es curvo.
///
/// Leer tramos y no puntos sueltos es lo que permite comprobar una costura con
/// curvas: que las dos caras de una frontera coincidan en los extremos no dice
/// nada si por en medio cada una dibuja una curva distinta.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Seg {
    from: Point,
    to: Point,
    controls: Option<(Point, Point)>,
}

impl Seg {
    /// Puntos de la curva, para medir contra ella. Con 16 pasos el error de
    /// muestreo queda muy por debajo de las centésimas que se comparan.
    fn sample(&self) -> Vec<Point> {
        let Some((c1, c2)) = self.controls else {
            return vec![self.from, self.to];
        };
        (0..=16)
            .map(|k| {
                let u = k as f64 / 16.0;
                let v = 1.0 - u;
                let (b0, b1, b2, b3) = (v * v * v, 3.0 * u * v * v, 3.0 * u * u * v, u * u * u);
                (
                    self.from.0 * b0 + c1.0 * b1 + c2.0 * b2 + self.to.0 * b3,
                    self.from.1 * b0 + c1.1 * b1 + c2.1 * b2 + self.to.1 * b3,
                )
            })
            .collect()
    }

    /// El mismo tramo del revés, que es como lo dibuja la cara de enfrente.
    fn reversed(&self) -> Seg {
        Seg {
            from: self.to,
            to: self.from,
            controls: self.controls.map(|(c1, c2)| (c2, c1)),
        }
    }

    /// Dirección con la que sale del punto de partida.
    fn leaving(&self) -> Point {
        let to = self.controls.map_or(self.to, |(c1, _)| c1);
        unit((to.0 - self.from.0, to.1 - self.from.1))
    }

    /// Dirección con la que llega al punto final.
    fn arriving(&self) -> Point {
        let from = self.controls.map_or(self.from, |(_, c2)| c2);
        unit((self.to.0 - from.0, self.to.1 - from.1))
    }
}

fn unit(v: Point) -> Point {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    if len == 0.0 {
        (0.0, 0.0)
    } else {
        (v.0 / len, v.1 / len)
    }
}

/// Colores bien separados: la tolerancia va a 0, así que cualquier par distinto
/// vale, pero mirando el SVG se distinguen. El punto es transparente, para poder
/// dibujar una figura suelta y que no salga también el contorno de su fondo.
const NEGRO: [u8; 4] = [0, 0, 0, 255];
const BLANCO: [u8; 4] = [255, 255, 255, 255];
const ROJO: [u8; 4] = [220, 40, 40, 255];
const NADA: [u8; 4] = [0, 0, 0, 0];

fn pixels(rows: &[&str]) -> (u32, u32, Vec<u8>) {
    let (w, h) = (rows[0].len() as u32, rows.len() as u32);
    let mut buf = Vec::with_capacity((w * h) as usize * 4);
    for row in rows {
        assert_eq!(row.len(), w as usize, "las filas no miden lo mismo");
        for c in row.chars() {
            buf.extend_from_slice(match c {
                '#' => &NEGRO,
                'o' => &BLANCO,
                'r' => &ROJO,
                '.' => &NADA,
                other => panic!("carácter {other:?} sin color"),
            });
        }
    }
    (w, h, buf)
}

/// Convierte un dibujo por el camino de rejilla, un píxel por celda.
///
/// El ajuste es un eje aparte de la segmentación, así que se prueba en la más
/// sencilla de las dos: contornos que se pueden contar a mano.
fn convert(rows: &[&str], fit: Fit) -> Conversion {
    let (w, h, buf) = pixels(rows);
    let config = Config {
        fit,
        ..Config::grid(GridOptions {
            // Sin detección ni fusión de colores: aquí se mira el contorno, y
            // que la rejilla o la paleta opinasen lo enturbiaría.
            scale: Some(1.0),
            tolerance: 0.0,
            remove_checkerboard: false,
            ..GridOptions::default()
        })
    };
    img2svg::convert_rgba(w, h, &buf, &config).expect("la conversión no debe fallar")
}

/* ------------------------------------------------------------- el documento --- */

/// Los atributos `d` del documento, en orden.
fn path_data(svg: &str) -> Vec<&str> {
    svg.split("d=\"")
        .skip(1)
        .map(|rest| &rest[..rest.find('"').expect("un atributo d sin cerrar")])
        .collect()
}

/// Los subtrazados del documento, como listas de tramos.
fn subpaths(svg: &str) -> Vec<Vec<Seg>> {
    path_data(svg).iter().flat_map(|d| parse(d)).collect()
}

/// Los vértices de cada subtrazado, que es lo que basta cuando no hay curvas.
fn corners(svg: &str) -> Vec<Vec<Point>> {
    subpaths(svg)
        .iter()
        .map(|path| path.iter().map(|s| s.from).collect())
        .collect()
}

/// Lee un `d` de los que emite el ajuste: `M`, `h`, `v`, `l`, `c` y `z`, todos
/// relativos menos el primero. Cualquier otro comando hace fallar el test, que
/// es lo que se quiere si un día se emite algo que no se pretendía.
fn parse(d: &str) -> Vec<Vec<Seg>> {
    let chars: Vec<char> = d.chars().collect();
    let mut paths = Vec::new();
    let mut segs: Vec<Seg> = Vec::new();
    let mut at = (0.0, 0.0);
    let mut start = at;
    let mut i = 0;

    fn line(segs: &mut Vec<Seg>, at: &mut Point, to: Point) {
        segs.push(Seg {
            from: *at,
            to,
            controls: None,
        });
        *at = to;
    }

    while i < chars.len() {
        let command = chars[i];
        i += 1;
        match command {
            'M' => {
                at = (number(&chars, &mut i), number(&chars, &mut i));
                start = at;
            }
            'h' => {
                let to = (at.0 + number(&chars, &mut i), at.1);
                line(&mut segs, &mut at, to);
            }
            'v' => {
                let to = (at.0, at.1 + number(&chars, &mut i));
                line(&mut segs, &mut at, to);
            }
            'l' => {
                let to = (at.0 + number(&chars, &mut i), at.1 + number(&chars, &mut i));
                line(&mut segs, &mut at, to);
            }
            'c' => {
                let c1 = (at.0 + number(&chars, &mut i), at.1 + number(&chars, &mut i));
                let c2 = (at.0 + number(&chars, &mut i), at.1 + number(&chars, &mut i));
                let to = (at.0 + number(&chars, &mut i), at.1 + number(&chars, &mut i));
                segs.push(Seg {
                    from: at,
                    to,
                    controls: Some((c1, c2)),
                });
                at = to;
            }
            // La `z` cierra con una recta, y sólo hay que escribirla si el
            // último tramo no acababa ya en el punto de partida.
            'z' => {
                if at != start {
                    line(&mut segs, &mut at, start);
                }
                at = start;
                paths.push(std::mem::take(&mut segs));
            }
            other => panic!("comando {other:?} inesperado en {d:?}"),
        }
    }
    assert!(segs.is_empty(), "un subtrazado sin cerrar en {d:?}");
    paths
}

fn number(chars: &[char], i: &mut usize) -> f64 {
    let start = *i;
    if chars[*i] == '-' {
        *i += 1;
    }
    while *i < chars.len() && (chars[*i].is_ascii_digit() || chars[*i] == '.') {
        *i += 1;
    }
    let n = chars[start..*i]
        .iter()
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("número ilegible en la posición {start}"));
    // El separador es un espacio, o el signo del número siguiente.
    while *i < chars.len() && chars[*i] == ' ' {
        *i += 1;
    }
    n
}

/* ---------------------------------------------------------------- costuras --- */

/// La comprobación que justifica toda la estructura: **cada segmento interior
/// lo dibujan exactamente dos caras**, y con los mismos extremos.
///
/// Si las dos caras de una frontera compartida se ajustaran por separado, una
/// podría quitar un vértice que la otra conserva; entre las dos líneas quedaría
/// una franja de fondo a la vista, y aquí el segmento largo de una cara y los
/// dos cortos de la otra aparecerían una sola vez cada uno.
///
/// Antes de contar hay que partir cada segmento por los vértices que caigan
/// dentro de él: una junta colineal en un nodo se puede fundir en una cara y no
/// en la otra sin mover la línea ni un pelo, y lo que tiene que coincidir es la
/// línea, no en cuántos comandos se escribió.
/// Y con curvas hay que comparar **la curva entera**, controles incluidos: dos
/// cúbicas que empiezan y acaban donde mismo pueden ir por sitios distintos, y
/// entre las dos quedaría el mismo pelo de fondo que con dos rectas.
#[cfg(feature = "illustration")]
fn comprueba_costuras(out: &Conversion) {
    let (w, h) = (out.canvas.0 as f64, out.canvas.1 as f64);
    let paths = subpaths(&out.svg);
    let vertices: HashSet<Key> = paths.iter().flatten().map(|s| key(s.from)).collect();

    // Las curvas van por su cuenta: no se parten por vértices intermedios
    // —ninguno cae encima— y se cuentan tal cual, orientadas siempre igual.
    let mut curvas: HashMap<[Key; 4], usize> = HashMap::new();
    let mut rectas: HashMap<(Key, Key), usize> = HashMap::new();

    for seg in paths.iter().flatten() {
        match seg.controls {
            Some(_) => *curvas.entry(canonical(seg)).or_default() += 1,
            None => {
                for (a, b) in split((seg.from, seg.to), &vertices) {
                    *rectas
                        .entry(if a < b { (a, b) } else { (b, a) })
                        .or_default() += 1;
                }
            }
        }
    }

    // El dibujo del test tapa el lienzo entero, así que lo único que dibuja una
    // sola cara es el borde, y el borde siempre es recto: una curva que
    // aparezca una sola vez es una costura rota.
    for (curva, n) in &curvas {
        assert_eq!(n, &2, "la curva {curva:?} la dibujan {n} caras, no dos");
    }

    let mut interiores = 0;
    for (&(a, b), &n) in &rectas {
        // El borde del lienzo lo dibuja una sola cara: al otro lado no hay
        // región ninguna.
        let (w, h) = (key((w, 0.0)).0, key((0.0, h)).1);
        let borde = (a.0 == 0 && b.0 == 0)
            || (a.1 == 0 && b.1 == 0)
            || (a.0 == w && b.0 == w)
            || (a.1 == h && b.1 == h);
        if borde {
            continue;
        }
        interiores += 1;
        assert_eq!(n, 2, "el segmento {a:?}-{b:?} lo dibujan {n} caras, no dos");
    }
    assert!(
        interiores > 0,
        "el dibujo no tiene ninguna frontera interior"
    );
}

/// Una coordenada como entero de centésimas, que es la precisión con la que se
/// escribe. Hace falta para poder meterlas en una tabla, y de paso dice que lo
/// que se compara es lo escrito y no lo que se tenía en memoria.
#[cfg(feature = "illustration")]
type Key = (i64, i64);

#[cfg(feature = "illustration")]
fn key(p: Point) -> Key {
    ((p.0 * 100.0).round() as i64, (p.1 * 100.0).round() as i64)
}

/// Los cuatro puntos de una curva, orientados siempre igual, para que la misma
/// curva dibujada en los dos sentidos dé la misma clave.
#[cfg(feature = "illustration")]
fn canonical(seg: &Seg) -> [Key; 4] {
    let seg = if key(seg.from) <= key(seg.to) {
        *seg
    } else {
        seg.reversed()
    };
    let (c1, c2) = seg.controls.expect("sólo se llama con curvas");
    [key(seg.from), key(c1), key(c2), key(seg.to)]
}

/// Parte un segmento por los vértices que caen dentro de él.
#[cfg(feature = "illustration")]
fn split((a, b): (Point, Point), vertices: &HashSet<Key>) -> Vec<(Key, Key)> {
    let (a, b) = (key(a), key(b));
    let d = (b.0 - a.0, b.1 - a.1);
    let largo = d.0 * d.0 + d.1 * d.1;
    let mut dentro: Vec<(i64, Key)> = vertices
        .iter()
        .filter_map(|&v| {
            let p = (v.0 - a.0, v.1 - a.1);
            let alineado = d.0 * p.1 - d.1 * p.0 == 0;
            let avance = d.0 * p.0 + d.1 * p.1;
            (alineado && avance > 0 && avance < largo).then_some((avance, v))
        })
        .collect();
    dentro.sort();

    let mut out = Vec::with_capacity(dentro.len() + 1);
    let mut from = a;
    for (_, v) in dentro {
        out.push((from, v));
        from = v;
    }
    out.push((from, b));
    out
}

/* -------------------------------------------------------------------- casos --- */

/// Una diagonal a 45° es una recta, y el ajuste de polígono la escribe como
/// tal. Es la ganancia que justifica el ajustador entero.
#[test]
fn una_diagonal_sale_recta() {
    let escalera = &["#....", "##...", "###..", "####.", "#####"];

    let escalones = corners(&convert(escalera, Fit::Pixel).svg);
    let recta = corners(&convert(escalera, Fit::polygon()).svg);

    assert_eq!(escalones.len(), 1);
    assert_eq!(recta.len(), 1, "el ajuste no debe partir el anillo");
    // El triángulo tiene doce vértices en escalera —dos por escalón— y cuatro
    // cuando la hipotenusa es un solo tramo: los tres del triángulo y el borde
    // de arriba, que mide un píxel porque la fila de arriba tiene un píxel.
    assert_eq!(escalones[0].len(), 12);
    assert_eq!(recta[0].len(), 4);
    // Y los cuatro son vértices que ya estaban: RDP elige, no inventa.
    let mut triangulo = recta[0].clone();
    triangulo.sort_by(|a, b| a.partial_cmp(b).expect("sin NaN"));
    assert_eq!(
        triangulo,
        vec![(0.0, 0.0), (0.0, 5.0), (1.0, 0.0), (5.0, 5.0)]
    );
}

/// Con tolerancia 0 el polígono dibuja exactamente la escalera: RDP sólo quita
/// lo que se aparta de la cuerda, y de una escalera no se aparta nada.
///
/// Fija el otro extremo del rango, que es lo que dice que la tolerancia es lo
/// único que decide cuánto se simplifica.
#[test]
fn sin_tolerancia_el_poligono_es_la_escalera() {
    let dibujo = &["#..r.", "##rr.", "###..", "#..##", "....#"];
    let pixel = convert(dibujo, Fit::Pixel);
    let poligono = convert(dibujo, Fit::Polygon { tolerance: 0.0 });

    assert_eq!(path_data(&poligono.svg), path_data(&pixel.svg));
}

/// Lo que la tolerancia promete, y es lo único que promete: **ningún vértice
/// del contorno queda a más de esa distancia de la línea que se dibuja**.
///
/// Conviene ser preciso aquí, porque lo evidente es falso. RDP no conserva un
/// detalle por ser más alto que la tolerancia: mide contra la cuerda que tenga
/// en ese momento, no contra los vecinos del vértice, y una cuerda que venga de
/// lejos se traga un píxel que sobresale aunque suelto se apartara 1.0. Lo que
/// no puede es apartarse de la figura más de lo pedido, y eso sí se comprueba.
#[test]
fn la_tolerancia_acota_lo_que_se_aparta() {
    // Diagonales, escalones tendidos, un píxel suelto que sobresale y un
    // entrante: cada uno se aparta de su cuerda una cantidad distinta.
    let dibujo = &[
        "..#####..",
        ".#######.",
        "#########",
        "####.####",
        "#########",
        ".#######.",
        "..##.##..",
    ];
    let contorno: Vec<Point> = corners(&convert(dibujo, Fit::Pixel).svg)
        .into_iter()
        .flatten()
        .collect();

    // Los dos ajustadores prometen lo mismo, así que se les pide lo mismo. El
    // margen extra es el redondeo del escritor, que trabaja en centésimas.
    for fit in [
        Fit::Polygon { tolerance: 0.0 },
        Fit::Polygon { tolerance: 0.75 },
        Fit::Polygon { tolerance: 1.5 },
        Fit::Polygon { tolerance: 3.0 },
        Fit::Spline { tolerance: 1.0 },
        Fit::Spline { tolerance: 1.5 },
        Fit::Spline { tolerance: 3.0 },
    ] {
        let tolerance = match fit {
            Fit::Polygon { tolerance } | Fit::Spline { tolerance } => tolerance,
            Fit::Pixel => 0.0,
        };
        let ajustado = subpaths(&convert(dibujo, fit).svg);
        for &p in &contorno {
            let d = distancia(p, &ajustado);
            assert!(
                d <= tolerance + 0.01,
                "con {fit:?} el vértice {p:?} se queda a {d:.3} de lo dibujado"
            );
        }
    }
}

/// Distancia de un punto a lo que se dibuja, curvas incluidas: se muestrean y
/// se mide contra la poligonal de las muestras.
fn distancia(p: Point, paths: &[Vec<Seg>]) -> f64 {
    paths
        .iter()
        .flatten()
        .flat_map(|seg| {
            let pts = seg.sample();
            (0..pts.len() - 1)
                .map(|i| (pts[i], pts[i + 1]))
                .collect::<Vec<_>>()
        })
        .map(|(a, b)| al_segmento(p, a, b))
        .fold(f64::INFINITY, f64::min)
}

fn al_segmento(p: Point, a: Point, b: Point) -> f64 {
    let (px, py) = (p.0, p.1);
    let (ax, ay) = (a.0, a.1);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    // Proyección recortada al segmento: fuera de él, el punto más cercano es un
    // extremo, y medir contra la recta infinita daría de menos.
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    ((px - ax - t * dx).powi(2) + (py - ay - t * dy).powi(2)).sqrt()
}

/// `crispEdges` apaga el suavizado, que es lo correcto para una escalera sobre
/// coordenadas enteras y lo contrario de lo que quiere una oblicua: dejaría
/// escalonada justo la diagonal que el ajuste acaba de enderezar.
#[test]
fn el_suavizado_depende_del_ajuste() {
    let dibujo = &["#..", "##.", "###"];
    assert!(convert(dibujo, Fit::Pixel).svg.contains("crispEdges"));
    assert!(!convert(dibujo, Fit::polygon()).svg.contains("crispEdges"));
    assert!(!convert(dibujo, Fit::spline()).svg.contains("crispEdges"));
}

/// Las dos caras de cada frontera coinciden, con el ajuste que sea.
///
/// El dibujo lleva diagonales, tramos rectos y nodos donde se juntan tres
/// colores —incluidos nodos por los que la frontera pasa de largo, que son los
/// que un ajuste por anillo simplificaría de forma distinta en cada cara.
#[test]
#[cfg(feature = "illustration")]
fn las_dos_caras_de_una_frontera_se_ajustan_igual() {
    let dibujo = &[
        "rrrrr####",
        "rrrr#####",
        "rrr######",
        "rrooo####",
        "roooo####",
        "rooooo###",
        "rroooo###",
        "rrrooo###",
    ];
    for fit in [
        Fit::Pixel,
        Fit::polygon(),
        Fit::Polygon { tolerance: 2.5 },
        Fit::spline(),
        Fit::Spline { tolerance: 2.5 },
    ] {
        let out = convert_cluster(dibujo, fit);
        comprueba_costuras(&out);
    }
}

/// Una esquina recta sigue siendo una esquina: el ajuste de curvas no redondea
/// un rectángulo.
///
/// Es lo que separa un ajustador de curvas utilizable de uno que convierte todo
/// dibujo en una mancha, y lo que decide la detección de esquinas.
#[test]
fn un_rectangulo_no_se_curva() {
    let dibujo = &["....", ".##.", ".##.", "....."];
    let dibujo = &dibujo[..3];
    let out = convert(dibujo, Fit::spline());
    let curvas = subpaths(&out.svg)
        .iter()
        .flatten()
        .filter(|s| s.controls.is_some())
        .count();
    assert_eq!(
        curvas, 0,
        "un rectángulo no lleva ni una curva: {}",
        out.svg
    );
}

/// Un círculo sale liso, **y la costura no se nota**.
///
/// Un bucle que no pasa por ningún nodo se parte por donde caiga
/// ([`img2svg::boundary`]), y eso cae en medio del contorno más liso que suele
/// haber en la imagen. Si ese corte se tratara como esquina, todas las manchas
/// sueltas de todas las fotos saldrían con un pico en un sitio distinto cada
/// vez. Aquí se comprueba justo eso: en cada junta entre dos curvas, la
/// dirección con la que se llega y con la que se sale son la misma.
#[test]
fn un_circulo_sale_liso() {
    let (w, h) = (81u32, 81u32);
    let mut buf = Vec::with_capacity((w * h) as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f64 - 40.0).powi(2) + (y as f64 - 40.0).powi(2)).sqrt();
            buf.extend_from_slice(if d <= 30.0 { &NEGRO } else { &NADA });
        }
    }
    let config = Config {
        fit: Fit::spline(),
        ..Config::grid(GridOptions {
            scale: Some(1.0),
            tolerance: 0.0,
            remove_checkerboard: false,
            ..GridOptions::default()
        })
    };
    let out = img2svg::convert_rgba(w, h, &buf, &config).expect("la conversión no debe fallar");
    let paths = subpaths(&out.svg);

    assert_eq!(paths.len(), 1, "el círculo es un solo subtrazado");
    let circulo = &paths[0];
    assert!(
        circulo.len() <= 12,
        "un círculo debería salir en pocos tramos, y salen {}: {}",
        circulo.len(),
        out.svg
    );

    // En **todas** las juntas, y no sólo entre curvas: la primera versión de
    // esto se saltaba las de curva con recta —que en un círculo digitalizado son
    // todas— y pasaba sin llegar a comprobar nada. Medido: bien cerrado da 0.00
    // en todas, y tratando la costura como esquina sale un 0.60.
    for i in 0..circulo.len() {
        let (antes, despues) = (circulo[i], circulo[(i + 1) % circulo.len()]);
        let (llega, sale) = (antes.arriving(), despues.leaving());
        let giro = (llega.0 * sale.1 - llega.1 * sale.0).abs();
        assert!(
            giro < 0.1,
            "la junta en {:?} hace un pico: llega {llega:?} y sale {sale:?}",
            antes.to
        );
    }
}

/// Y con fronteras compartidas **curvas**, que es el caso que el dibujo de
/// arriba no llega a tener: mide nueve píxeles de ancho, y en cadenas tan cortas
/// no se emite ni una curva.
///
/// Dos discos planos que se solapan sobre un fondo liso. La frontera entre los
/// dos discos es un arco compartido por dos regiones, y es exactamente donde una
/// cúbica mal invertida deja de coincidir consigo misma: mismos extremos, otros
/// controles, y entre las dos caras asoma el fondo.
#[test]
#[cfg(feature = "illustration")]
fn las_dos_caras_de_una_frontera_curva_se_ajustan_igual() {
    let (w, h) = (120u32, 120u32);
    let discos = [(45.0f64, 60.0f64, 34.0f64, NEGRO), (75.0, 60.0, 34.0, ROJO)];
    let mut buf = Vec::with_capacity((w * h) as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let mut color = BLANCO;
            for &(cx, cy, r, c) in &discos {
                if ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt() <= r {
                    color = c;
                }
            }
            buf.extend_from_slice(&color);
        }
    }

    for fit in [Fit::spline(), Fit::Spline { tolerance: 2.5 }] {
        let out = convert_cluster_buf(w, h, &buf, fit);
        let curvas = subpaths(&out.svg)
            .iter()
            .flatten()
            .filter(|s| s.controls.is_some())
            .count();
        assert!(
            curvas > 0,
            "sin curvas no se está comprobando nada con {fit:?}"
        );
        comprueba_costuras(&out);
    }
}

/// El mismo dibujo por la segmentación de clustering, que es la que comparte
/// fronteras entre regiones. La de rejilla traza cada una por su cuenta.
#[cfg(feature = "illustration")]
fn convert_cluster(rows: &[&str], fit: Fit) -> Conversion {
    let (w, h, buf) = pixels(rows);
    convert_cluster_buf(w, h, &buf, fit)
}

#[cfg(feature = "illustration")]
fn convert_cluster_buf(w: u32, h: u32, buf: &[u8], fit: Fit) -> Conversion {
    use img2svg::ClusterOptions;

    let config = Config {
        fit,
        ..Config::cluster(ClusterOptions {
            // Sin filtrar: aquí se mira el ajuste, y con el umbral por defecto
            // un dibujo de este tamaño es todo motas.
            filter_speckle: 0,
            min_thickness: 0.0,
            // Y sobre su propia retícula: estos dibujos se escriben fila a fila
            // para que el contorno sea exactamente el que se quiere ajustar, así
            // que elegir escala de trabajo mediría otro contorno.
            simplify: Some(0.0),
            ..ClusterOptions::default()
        })
    };
    img2svg::convert_rgba(w, h, buf, &config).expect("la conversión no debe fallar")
}
