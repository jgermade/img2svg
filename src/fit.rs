//! Ajuste: del contorno de una región a los datos de un `<path>`.
//!
//! Es el otro eje de la conversión, ortogonal a la segmentación: cualquier
//! ajustador sirve para cualquier segmentación. Son tres: [`Fit::Pixel`], la
//! escalera literal; [`Fit::Polygon`], que la simplifica a segmentos rectos; y
//! [`Fit::Spline`], que la ajusta con Béziers cúbicas ([`spline`]).
//!
//! Los tres prometen lo mismo y sólo eso: **ningún punto del contorno acaba a
//! más de la tolerancia de lo que se dibuja**. Ninguno promete un fichero más
//! pequeño; de hecho el de curvas sale más grande que el de polígono con la
//! misma tolerancia, porque una cúbica cuesta seis números y una recta dos. Lo
//! que compra es que el contorno siga siendo liso por mucho que se amplíe, que
//! es otra cosa.
//!
//! # Se ajusta por media arista, y sólo después se ensamblan los anillos
//!
//! El orden importa y no es el evidente. La representación intermedia extrae
//! **cada frontera una sola vez** ([`crate::region`]) precisamente para que las
//! dos caras que la comparten reciban la misma geometría; si el ajuste
//! trabajara sobre el anillo ya ensamblado, esa garantía se perdería en el
//! último paso: un nodo donde se juntan tres regiones no es más que un vértice
//! interior para el simplificador, que lo descartaría por recto, y la misma
//! frontera se simplificaría dos veces —dentro de dos anillos distintos, con
//! vecinos distintos a cada lado del nodo— saliendo distinta. Entre las dos
//! asomaría el pelo de fondo que toda esta estructura existe para evitar.
//!
//! Así que [`Fitted`] ajusta cada `EdgeId` una vez, y [`Fitted::ring_data`]
//! ensambla el anillo a partir de tramos ya ajustados. Con `Pixel` sobre
//! coordenadas enteras la diferencia no se ve —colapsar los colineales a un
//! lado y a otro del nodo da la misma escalera—, que es justo por lo que esto
//! podía llevar tanto tiempo mal sin que nada lo notara.
//!
//! El único paso que sí mira el anillo entero es el de después: quitar los
//! vértices que quedan **exactamente** sobre la recta entre sus vecinos, que es
//! lo que pasa en un nodo por el que la frontera pasa de largo. Eso no es
//! ajustar, es no escribir un punto que no dibuja nada: la curva resultante es
//! idéntica, así que las dos caras siguen coincidiendo aunque una lo quite y la
//! otra no.

mod spline;

use crate::region::{self, Chainable, Regions, Ring};
use crate::trace::Point;

/// Un punto del contorno ya ajustado, en reales.
///
/// El trazado trabaja sobre la retícula de esquinas de píxel y por eso usa
/// enteros; en cuanto un ajustador puede inventar un punto que no está en la
/// retícula —los controles de una Bézier— deja de valer.
pub type Pt = (f64, f64);

/// Un vértice del contorno ajustado, con los controles de sus dos lados.
///
/// El tramo entre dos vértices es una Bézier cúbica cuando el primero tiene
/// `cout` y el segundo `cin`, y una recta cuando no. Guardar el control **en el
/// vértice** y no en el tramo es lo que hace que dar la vuelta a una cadena sea
/// exacto: se invierte el orden y cada vértice cambia sus controles de lado, con
/// los mismos cuatro números. Por eso las dos caras de una frontera compartida
/// siguen recibiendo la misma geometría, que es para lo que existe toda esta
/// estructura.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Vertex {
    pub p: Pt,
    /// Control con el que **llega** la curva a `p`, si llega curva.
    pub cin: Option<Pt>,
    /// Control con el que **sale** la curva de `p`, si sale curva.
    pub cout: Option<Pt>,
}

impl Vertex {
    /// Un vértice de esquina, sin curva a ningún lado.
    fn corner(p: Point) -> Self {
        Vertex {
            p: (p.0 as f64, p.1 as f64),
            cin: None,
            cout: None,
        }
    }
}

impl Chainable for Vertex {
    fn reversed(self) -> Self {
        Vertex {
            p: self.p,
            cin: self.cout,
            cout: self.cin,
        }
    }

    /// En una junta, lo que llega es del tramo que acaba y lo que sale del que
    /// empieza. Quedarse con uno de los dos vértices a secas perdería la mitad
    /// de la curva justo en el nodo.
    fn join(self, next: Self) -> Self {
        Vertex {
            p: self.p,
            cin: self.cin,
            cout: next.cout,
        }
    }

    fn same_place(self, other: Self) -> bool {
        self.p == other.p
    }
}

/// Cómo se convierte un contorno en datos de path.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Fit {
    /// La escalera literal del contorno, con comandos `h`/`v`.
    #[default]
    Pixel,
    /// Segmentos rectos, quedándose con los vértices que dibujan algo
    /// (Ramer–Douglas–Peucker). `tolerance` es la desviación máxima admitida,
    /// en píxeles del lienzo.
    Polygon { tolerance: f64 },
    /// Béziers cúbicas ajustadas por mínimos cuadrados, partiendo el contorno
    /// por sus esquinas (Schneider). `tolerance` es lo mismo que en
    /// [`Fit::Polygon`]: la desviación máxima admitida.
    Spline { tolerance: f64 },
}

impl Fit {
    /// Desviación por defecto, y la misma para los dos ajustadores que la usan.
    ///
    /// Es una sola constante porque es una sola promesa: **ningún punto del
    /// contorno acaba a más de esa distancia de lo que se dibuja**. Lo que cada
    /// ajustador hace con el margen es cosa suya.
    ///
    /// El número sale del polígono. El escalón de una diagonal a 45° se aparta
    /// `1/√2 ≈ 0.707` de su cuerda: por debajo no se endereza ni una diagonal y
    /// el ajuste no hace nada, y justo por encima se enderezan todas. Es el
    /// valor más pequeño que sirve para algo, y por eso es el de partida.
    ///
    /// Lo que **no** hace es garantizar que un detalle más alto que la
    /// tolerancia sobreviva. RDP mide contra la cuerda que tenga en cada paso de
    /// la recursión, no contra los vecinos del vértice, así que una cuerda que
    /// venga de lejos puede tragarse un píxel que sobresale aunque suelto se
    /// apartara 1.0. Y el ajuste de curvas hace lo propio: mide contra la Bézier
    /// que lleva en ese momento.
    ///
    /// Subirla comprime bastante más —de 0.75 a 1.5 son otro 20% de fichero en
    /// el corpus—, a cambio de ir redondeando las esquinas pequeñas.
    pub const TOLERANCE: f64 = 0.75;

    /// Desviación por defecto del ajuste de curvas, que es **otra**.
    ///
    /// El contorno del que se parte es una escalera sobre la retícula, así que
    /// ya viene con su propio error: los peldaños se apartan hasta `0.707` de la
    /// forma lisa que representan. El polígono no lo nota —sus vértices *son*
    /// los de la retícula, y con tolerancia 0 reproduce la escalera exacta—,
    /// pero una curva tiene que pasar cerca de todos esos peldaños a la vez, y
    /// por debajo de 1.0 se dedica a perseguirlos: se parte una y otra vez y
    /// acaba emitiendo recta tras recta.
    ///
    /// Medido sobre un círculo de radio 30, contando los tramos que salen:
    ///
    /// | tolerancia | tramos | bytes |
    /// | --- | --- | --- |
    /// | 0.75 | 2 curvas + 108 rectas | 618 |
    /// | 1.0 | 6 + 36 | 472 |
    /// | 1.25 | 14 + 12 | 568 |
    /// | **1.5** | **4 + 4** | **290** |
    /// | 3.0 | 4 + 4 | 290 |
    ///
    /// A 1.5 el círculo entero sale en ocho tramos y la cosa se estabiliza. Por
    /// debajo, el ajustador está midiendo contra el ruido de cuantización.
    pub const SPLINE_TOLERANCE: f64 = 1.5;

    /// El ajuste de polígono con la desviación por defecto.
    pub fn polygon() -> Self {
        Fit::Polygon {
            tolerance: Self::TOLERANCE,
        }
    }

    /// El ajuste de curvas con la desviación por defecto.
    pub fn spline() -> Self {
        Fit::Spline {
            tolerance: Self::SPLINE_TOLERANCE,
        }
    }

    /// La desviación por defecto del ajustador que se nombre, para las tres
    /// superficies: son dos números porque son dos suelos distintos, y quien
    /// sólo elige ajustador no tiene por qué saberlo.
    pub fn default_tolerance(name: &str) -> f64 {
        match name {
            "spline" => Self::SPLINE_TOLERANCE,
            _ => Self::TOLERANCE,
        }
    }

    /// Si el ajustador puede emitir tramos oblicuos o curvos, que es lo que
    /// decide si el suavizado del documento estorba o hace falta.
    pub fn smooth(self) -> bool {
        !matches!(self, Fit::Pixel)
    }
}

/// Los tramos ya ajustados, indexados por `EdgeId` igual que `Regions::edges`.
///
/// Existe como tipo, y no como una función que devuelva el `d` de un anillo,
/// para que el orden sea el que impone la firma: primero se ajusta todo, y sólo
/// con eso en la mano se puede pedir un anillo.
pub struct Fitted(Vec<Vec<Vertex>>);

impl Fitted {
    /// Ajusta cada media arista de la segmentación, una sola vez.
    pub fn new(regions: &Regions, fit: Fit) -> Self {
        Fitted(
            regions
                .edges
                .iter()
                .map(|edge| chain_fit(&edge.points, fit))
                .collect(),
        )
    }

    /// Datos `d` de un anillo cerrado, ensamblado a partir de sus tramos.
    pub fn ring_data(&self, ring: &Ring) -> String {
        let vertices: Vec<Vertex> = region::chain(ring, |edge| self.0[edge].as_slice());
        subpath(&drop_collinear(&vertices))
    }
}

/// Ajusta un tramo. Los dos extremos se quedan donde están: son los nodos en
/// los que se encuentran las cadenas vecinas, y moverlos las abriría.
fn chain_fit(points: &[Point], fit: Fit) -> Vec<Vertex> {
    // Un tramo cerrado repite el primer punto al final; esa repetición es su
    // marca, y hay que devolverla puesta.
    if points.len() > 1 && points[0] == points[points.len() - 1] {
        let mut out = closed_fit(&points[..points.len() - 1], fit);
        if let Some(&first) = out.first() {
            out.push(first);
        }
        return out;
    }
    match fit {
        Fit::Pixel => corners(simplify_open(points)),
        Fit::Polygon { tolerance } => corners(rdp(&simplify_open(points), tolerance)),
        Fit::Spline { tolerance } => spline::open(points, tolerance),
    }
}

/// Ajusta una cadena cerrada, ya sin el punto repetido del final.
fn closed_fit(points: &[Point], fit: Fit) -> Vec<Vertex> {
    match fit {
        Fit::Pixel => corners(simplify(points)),
        Fit::Polygon { tolerance } => corners(rdp_closed(&simplify(points), tolerance)),
        Fit::Spline { tolerance } => spline::closed(points, tolerance),
    }
}

/// Vértices de esquina a partir de una polilínea: lo que devuelven los dos
/// ajustadores que sólo eligen puntos y no inventan ninguno.
fn corners(points: Vec<Point>) -> Vec<Vertex> {
    points.into_iter().map(Vertex::corner).collect()
}

/// Elimina los vértices intermedios de los tramos rectos, dejando las esquinas.
///
/// Cíclica: el primer punto también se descarta si el contorno pasa recto por
/// él. Vive aquí y no en el trazado porque los ajustadores de curvas quieren la
/// polilínea densa —necesitan los puntos intermedios para estimar tangentes y
/// detectar esquinas—, y porque quitar un punto colineal no cambia la curva:
/// pasa por donde pasaba.
pub fn simplify(points: &[Point]) -> Vec<Point> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .filter(|&i| {
            turns(
                real(points[(i + n - 1) % n]),
                real(points[i]),
                real(points[(i + 1) % n]),
            )
        })
        .map(|i| points[i])
        .collect()
}

/// Lo mismo sobre el anillo ya ensamblado, que es donde aparecen las juntas.
///
/// Es el único paso que mira el anillo entero, y por eso sólo puede quitar lo
/// que no cambia el dibujo: un vértice **exactamente** sobre la recta entre sus
/// vecinos, y sólo si por él no pasa ninguna curva. Un nodo por el que la
/// frontera sigue de largo se puede fundir en una cara y no en la otra sin que
/// la línea se mueva un pelo, así que las dos siguen coincidiendo.
fn drop_collinear(v: &[Vertex]) -> Vec<Vertex> {
    let n = v.len();
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .filter(|&i| {
            let (prev, cur, next) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
            // Con un control a cualquiera de los dos lados, el vértice manda
            // sobre una curva y quitarlo sí cambiaría el dibujo.
            cur.cin.is_some()
                || cur.cout.is_some()
                || prev.cout.is_some()
                || next.cin.is_some()
                || turns(prev.p, cur.p, next.p)
        })
        .map(|i| v[i])
        .collect()
}

fn real(p: Point) -> Pt {
    (p.0 as f64, p.1 as f64)
}

/// Lo mismo sobre una polilínea abierta, conservando los dos extremos.
fn simplify_open(points: &[Point]) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(n);
    out.push(points[0]);
    out.extend(
        (1..n - 1)
            .filter(|&i| turns(real(points[i - 1]), real(points[i]), real(points[i + 1])))
            .map(|i| points[i]),
    );
    out.push(points[n - 1]);
    out
}

/// Si el camino cambia de dirección al pasar por `cur`.
///
/// Seguir recto es producto vectorial cero **y** avanzar en el mismo sentido:
/// una vuelta de 180° también tiene el vectorial a cero, y ese punto no se
/// puede quitar sin cambiar el dibujo. Se compara así y no por igualdad de
/// deltas porque después del primer colapso los pasos ya no son unitarios: dos
/// tramos rectos seguidos de 3 y 2 unidades son la misma recta con deltas
/// distintos.
///
/// En reales, y comparando contra cero exacto: los dos ajustadores que sólo
/// eligen vértices los traen de la retícula, y un entero de una imagen —y su
/// producto— cabe holgadamente en la mantisa de un `f64`, así que la cuenta es
/// la misma que en enteros. Sobre coordenadas de verdad reales no colapsa casi
/// nada, que es lo correcto: una curva no pasa exactamente por la cuerda.
fn turns(prev: Pt, cur: Pt, next: Pt) -> bool {
    let a = (cur.0 - prev.0, cur.1 - prev.1);
    let b = (next.0 - cur.0, next.1 - cur.1);
    a.0 * b.1 - a.1 * b.0 != 0.0 || a.0 * b.0 + a.1 * b.1 <= 0.0
}

/// Ramer–Douglas–Peucker sobre una polilínea **abierta**, en índices.
///
/// Conserva los dos extremos —que es justo lo que pide un tramo compartido: los
/// nodos no se mueven— y descarta los vértices que se apartan menos de
/// `tolerance` de la cuerda. Devuelve **cuáles** se quedan y no los puntos: RDP
/// sólo elige, nunca inventa, y el ajuste de curvas necesita saber qué trozo del
/// contorno original ha quedado debajo de cada tramo.
///
/// Lo usan los dos ajustadores que no son el de píxel: el de polígono para todo,
/// y el de curvas para las rectas. Es la única forma de que las rectas de uno
/// salgan tan cortas como las del otro.
pub(crate) fn rdp_keep(points: &[Pt], tolerance: f64) -> Vec<usize> {
    let n = points.len();
    if n < 3 {
        return (0..n).collect();
    }
    let limit = tolerance * tolerance;
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;

    // Pila explícita en vez de recursión: el reparto puede salir tan
    // desequilibrado como apartar un punto por nivel, y entonces la profundidad
    // sería la longitud del tramo, que en el contorno de una región grande son
    // miles de puntos.
    let mut stack = vec![(0, n - 1)];
    while let Some((a, b)) = stack.pop() {
        let mut worst = a;
        let mut worst_d = 0.0;
        for i in a + 1..b {
            let d = deviation2(points[i], points[a], points[b]);
            if d > worst_d {
                worst_d = d;
                worst = i;
            }
        }
        if worst_d <= limit {
            continue;
        }
        keep[worst] = true;
        stack.push((a, worst));
        stack.push((worst, b));
    }

    (0..n).filter(|&i| keep[i]).collect()
}

/// Lo mismo sobre puntos de la retícula, que es como lo quiere el polígono.
fn rdp(points: &[Point], tolerance: f64) -> Vec<Point> {
    let real: Vec<Pt> = points.iter().map(|&p| self::real(p)).collect();
    rdp_keep(&real, tolerance)
        .into_iter()
        .map(|i| points[i])
        .collect()
}

/// RDP sobre una cadena **cerrada**, ya sin el punto repetido del final.
///
/// Un anillo no tiene extremos que fijar y RDP necesita una cuerda de la que
/// medir, así que hay que elegir dos puntos y dejarlos quietos. Se toman el
/// primero y el más lejano a él, que es aproximadamente el diámetro del anillo:
/// las dos mitades salen parecidas y ninguno de los dos puntos cae en medio de
/// una recta larga, que es donde más se notaría clavarlo.
fn rdp_closed(points: &[Point], tolerance: f64) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let far = (1..n).max_by_key(|&i| dist2(points[0], points[i])).unwrap();

    let mut out = rdp(&points[..=far], tolerance);
    // El punto de corte lo vuelve a traer la segunda mitad.
    out.pop();

    let mut back: Vec<Point> = points[far..].to_vec();
    back.push(points[0]);
    let mut back = rdp(&back, tolerance);
    // Y el cierre del anillo es implícito.
    back.pop();

    out.extend(back);
    out
}

/// Distancia al cuadrado de `p` a la recta que pasa por `a` y `b`.
///
/// Al cuadrado para poder compararla con la tolerancia sin una raíz por punto.
/// Sobre puntos de la retícula la cuenta sigue siendo exacta: un entero de una
/// imagen y su producto caben de sobra en la mantisa de un `f64`, así que el
/// polígono con tolerancia 0 sigue reproduciendo la escalera al pie de la letra.
fn deviation2(p: Pt, a: Pt, b: Pt) -> f64 {
    let ab = (b.0 - a.0, b.1 - a.1);
    let ap = (p.0 - a.0, p.1 - a.1);
    let len2 = ab.0 * ab.0 + ab.1 * ab.1;
    if len2 == 0.0 {
        // Cuerda degenerada: la distancia es al propio punto.
        return ap.0 * ap.0 + ap.1 * ap.1;
    }
    let area = ab.0 * ap.1 - ab.1 * ap.0;
    area * area / len2
}

/// Distancia al cuadrado entre dos puntos de la retícula. En enteros porque
/// sólo la usa `rdp_closed` para elegir el punto más lejano, y ahí lo que hace
/// falta es un orden exacto y no una medida.
fn dist2(a: Point, b: Point) -> i64 {
    let d = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
    d.0 * d.0 + d.1 * d.1
}

/// Decimales con los que se escribe una coordenada.
///
/// Dos bastan: una centésima de píxel no se ve en ningún zoom razonable, y cada
/// decimal de más son dos bytes por número en un fichero que son casi todo
/// números. Los dos ajustadores de vértices dan enteros y no gastan ninguno.
const PRECISION: usize = 2;

/// Redondea una coordenada **absoluta**.
///
/// Se redondea el absoluto y no cada delta porque los comandos son relativos: un
/// error de redondeo por segmento se va sumando a lo largo del contorno, y en un
/// anillo de miles de tramos el final acaba lejos del principio. Redondeando
/// antes, los deltas salen de números ya redondeados y no se acumula nada. De
/// paso, las dos caras de una frontera redondean los mismos absolutos, así que
/// la costura sobrevive al redondeo.
fn round(v: f64) -> f64 {
    let factor = 10f64.powi(PRECISION as i32);
    let r = (v * factor).round() / factor;
    // `-0` se escribiría tal cual, y el ajuste de píxel nunca lo emitió.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn round_pt(p: Pt) -> Pt {
    (round(p.0), round(p.1))
}

/// Escribe un número detrás de otro, con separador sólo cuando hace falta.
///
/// El signo menos ya separa, y después de una letra de comando tampoco hay nada
/// que separar. Es la misma economía de siempre, ahora en un solo sitio.
fn push_num(d: &mut String, v: f64) {
    let v = if v == 0.0 { 0.0 } else { v };
    let mut s = format!("{v:.PRECISION$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if !s.starts_with('-') && d.ends_with(|c: char| c.is_ascii_digit()) {
        d.push(' ');
    }
    d.push_str(&s);
}

/// Un subtrazado cerrado, en comandos relativos.
///
/// Se usan `h`/`v` siempre que se pueda, que ocupan la mitad que `l`; con el
/// ajuste de píxel es siempre, porque todos sus tramos son de un eje. El `l`
/// aparece con el polígono, donde ya hay oblicuas, y la `c` con las curvas.
fn subpath(v: &[Vertex]) -> String {
    let n = v.len();
    if n == 0 {
        return String::new();
    }
    let mut at = round_pt(v[0].p);
    let mut d = String::from("M");
    push_num(&mut d, at.0);
    push_num(&mut d, at.1);

    for i in 0..n {
        let (a, b) = (v[i], v[(i + 1) % n]);
        let to = round_pt(b.p);
        let (dx, dy) = (to.0 - at.0, to.1 - at.1);

        match (a.cout, b.cin) {
            (Some(c1), Some(c2)) => {
                let (c1, c2) = (round_pt(c1), round_pt(c2));
                d.push('c');
                for n in [c1.0 - at.0, c1.1 - at.1, c2.0 - at.0, c2.1 - at.1, dx, dy] {
                    push_num(&mut d, n);
                }
            }
            // El último tramo, si es recto, lo dibuja la `z` y no hace falta
            // escribirlo. Si es curvo sí: `z` cierra con una recta.
            _ if i + 1 == n => {}
            _ if dy == 0.0 => {
                d.push('h');
                push_num(&mut d, dx);
            }
            _ if dx == 0.0 => {
                d.push('v');
                push_num(&mut d, dy);
            }
            _ => {
                d.push('l');
                push_num(&mut d, dx);
                push_num(&mut d, dy);
            }
        }
        at = to;
    }
    d.push('z');
    d
}
