//! Degradados: cuando un montón de bandas son en realidad una sola figura.
//!
//! La paleta reparte una rampa continua en escalones, porque una región tiene un
//! color y nada más. En una foto eso es la mayor parte del documento: un cielo
//! sale a doce bandas, con once fronteras entre ellas que no dibujan nada —sólo
//! marcan por dónde cruzó la rampa un umbral de cuantización— y que son los
//! contornos más retorcidos de todo el SVG, porque siguen el ruido del original.
//!
//! Un SVG sí sabe decir «degradado». Esto encuentra los grupos de bandas que lo
//! son, los funde en una figura y los pinta con un `<linearGradient>`.
//!
//! # Qué es ser una rampa
//!
//! Un `<linearGradient>` expresa exactamente una cosa: **el color como función de
//! la proyección de la posición sobre un eje**. Así que el criterio es esa misma
//! cosa, con la condición de que el degradado sirva de algo:
//!
//! > Un grupo de regiones vecinas es una rampa cuando un solo degradado lineal
//! > reproduce el color de todas ellas con un error que no pasa de [`CEILING`]
//! > tolerancias **ni de la [`GAIN`]-ésima parte del color que abarca**.
//!
//! Lo segundo no estaba en el plan y hace falta: un color plano ya reproduce
//! cualquier grupo con un error igual a lo que el grupo abarca, así que un
//! degradado que no baje bastante de ahí no está explicando nada. Sin esa
//! condición, las seis entradas casi negras que el *ringing* de un JPEG deja
//! alrededor de un trazo caben **todas** dentro del techo, cualquier eje las
//! «explica», y salían degradados de puro ruido: medidos sobre la portada de un
//! disco, veinticuatro, y dos de ellos repartían una cara en dos tonos de piel a
//! lo largo de una diagonal que no existe.
//!
//! Lo que **no** hace falta pedir aparte es que los colores estén alineados en
//! Oklab —el degradado puede torcerse por donde quiera— ni que la geometría se
//! apile: dos colores a la misma altura del eje piden dos paradas en el mismo
//! sitio, el error se dispara y el grupo se cae solo.
//!
//! # Una parada por color
//!
//! Con dos paradas el degradado tendría que ser recto en el color, y cualquier
//! rampa con una gamma dentro quedaría fuera. Con una parada por color, puesta en
//! el centro de todo lo que ese color pinta, el degradado puede seguir una rampa
//! torcida y sigue habiendo algo que comprobar.
//!
//! Por **banda** —que fue el primer intento— no lo hay: sería una parada por dato,
//! el degradado acertaría el centro de cada banda por construcción y una mota es
//! corta sobre cualquier eje, así que el criterio no podía fallar y aceptaba
//! cualquier cosa. Es la diferencia entre ajustar e interpolar, y sólo lo primero
//! se puede comprobar.
//!
//! El error entonces significa algo afilado: dentro de una banda el degradado va
//! del punto medio con la anterior al punto medio con la siguiente, así que lo más
//! que se aparta de su color plano es **medio escalón**. El techo limita el tamaño
//! del escalón entre bandas vecinas, no el ancho de una banda. Una rampa de verdad
//! tiene escalones pequeños y pasa; una bandera de franjas tiene escalones enormes
//! y se rechaza, que es lo que hay que acertar: un degradado ablandaría un borde
//! que el original tiene neto.
//!
//! # Medido contra lo que el navegador va a dibujar
//!
//! SVG interpola las paradas en **sRGB**, no en Oklab. Así que el modelo con el
//! que se mide el error es la interpolación en sRGB, y sólo la *distancia* se toma
//! en Oklab. Medir en Oklab una recta que en Oklab es recta sería comprobar un
//! degradado que no va a dibujar nadie.
//!
//! # Fundir sale gratis
//!
//! La unión de un grupo de regiones vecinas no necesita álgebra de polígonos.
//! Cada tramo de [`crate::region`] ya sabe qué región tiene a cada lado, así que
//! el contorno del grupo son **los tramos con exactamente un lado dentro**, y
//! `boundary::rings` —que ya sabe encadenar tramos orientados en anillos— hace el
//! resto. Los de dentro simplemente dejan de estar referenciados.
//!
//! Ahí es donde están las anclas: esas fronteras entre bandas son las más
//! retorcidas del documento, y son justo las que desaparecen.
//!
//! # Dónde se nota y dónde no
//!
//! Se nota donde hay degradado, y sólo ahí. Medido sobre un cielo de 900x600 con
//! grano de foto —una rampa de nueve entradas repartida en bandas de borde
//! dentado—, el documento pasa de **121 figuras y 70,6 KB a 3 y 5,9 KB**, y el
//! bandeado desaparece del dibujo.
//!
//! En un cartel de colores planos no encuentra casi nada, que es lo correcto, y
//! entonces **cuesta**: en la portada de un disco son 17 degradados pequeños y
//! 2 KB de más. La conversión entera se encarece un 10%.
//!
//! Y tiene un pique con `min_color_share`, que conviene saber: esa opción quita de
//! la paleta las entradas que pintan poco, y las de en medio de una rampa pintan
//! poco, así que deja los escalones más grandes de lo que un degradado sabe
//! reproducir. En un dibujo de 5 Mpx, apagarla sube de 24 degradados a 98 — y aun
//! así el documento sale más grande, porque la paleta fina cuesta más de lo que los
//! degradados ahorran. Cada una gana en su sitio y no se puede tener las dos.

use std::collections::HashSet;

use crate::cluster::NONE;
use crate::color::{Oklab, Rgba};
use crate::region::{Axis, EdgeId, Ramp, RegionId, Regions};

/// Cuánto puede apartarse el degradado del color plano de una banda, en múltiplos
/// de `tolerance`.
///
/// Es lo que este paso añade a la cota de [`crate::cluster`], así que no se sube
/// alegremente: cada décima aquí es error que se le suma a un píxel que ya venía
/// desviado. Se eligió mirando el dibujo, porque es un límite de daño y el daño se
/// ve:
///
/// | techo | portada | Sonic1.png | qué se ve |
/// | --- | --- | --- | --- |
/// | 1 | 10 degradados | 5 | nada, ni bueno ni malo |
/// | **2** | **17** | **24** | igual que sin degradados |
/// | 3 | 24 | 25 | la `l` de «blur» se apaga, dos caras se lavan |
/// | 5 | 25 | 22 | el fondo amarillo se derrama sobre el verde |
///
/// A `2` los dos dibujos salen indistinguibles de como salían, y a `3` ya no. Lo
/// que se gana pasando de ahí es poco y lo que se pierde es el dibujo.
pub const CEILING: f64 = 2.0;

/// Colores de paleta distintos que hace falta juntar para llamarlo rampa.
///
/// Dos regiones vecinas siempre se pueden partir con algún degradado tendido: el
/// modelo se queda a medio camino de los dos colores y, si están a una tolerancia,
/// el error de cada uno es media. Eso no es un degradado, es hacer la media, y ya
/// hay un ajuste para eso que se llama `tolerance`. Con tres colores el modelo
/// tiene que acertar un orden además de un valor, que es lo que distingue una
/// rampa de un promedio.
const MIN_COLORS: usize = 3;

/// Cuántas veces menor que el recorrido de color del degradado tiene que ser su
/// error para que valga la pena. Ver [`Fit::earns_it`].
///
/// Una rampa de `n` colores repartidos por el eje se equivoca como mucho medio
/// escalón, o sea `recorrido / 2(n-1)`, así que en teoría `3.0` deja entrar las de
/// tres colores en adelante. En la práctica no: las bandas no son cortes limpios
/// de la rampa, y a `3.0` sólo sobreviven cuatro degradados en un dibujo de 5 Mpx
/// contra veinticuatro a `2.0`. Se queda en `2.0`, que es además lo mínimo que se
/// le puede pedir a algo que quiere ser mejor que un color plano: equivocarse la
/// mitad.
pub const GAIN: f64 = 2.0;

/// Bandas que se le dan a un grupo para enseñar su tercer color.
///
/// Hasta [`MIN_COLORS`] colores el degradado no tiene que ganarse nada —no puede,
/// ver [`Fit::earns_it`]—, así que un damero de dos entradas en una zona de ruido
/// crece hasta el tope sin que nada lo frene, cuesta su cuadrado y acaba en la
/// basura por no llegar a tres colores. En un dibujo de 5 Mpx eso era **el 97% del
/// tiempo de esta etapa**: 1,5 s de 1,54 s. Cortando a las ocho bandas quedan 38 ms
/// y los mismos degradados, porque una rampa de verdad enseña su tercer color en
/// los primeros saltos.
const BOOTSTRAP: usize = 8;

/// Tope de bandas por grupo.
///
/// Cada candidata que se prueba vuelve a ajustar el eje y a revisar a todos los
/// miembros, así que un grupo de `m` cuesta del orden de `m²`. Un tope acota eso,
/// pero **no puede ser bajo**: el sitio donde esto gana es justo un cielo con
/// grano, que son ciento y pico bandas de una sola rampa. Bajarlo a 64 devuelve ese
/// cielo de 3 figuras a 23, y a 24 lo devuelve a 75. Quien paga el coste no es el
/// tope sino [`BOOTSTRAP`].
const MAX_BANDS: usize = 256;

/// Momentos de una región sobre sus píxeles. Con esto el ajuste por mínimos
/// cuadrados de un grupo es una suma, y quitar o poner una banda no vuelve a mirar
/// la imagen.
#[derive(Clone, Copy, Default)]
struct Moments {
    n: f64,
    x: f64,
    y: f64,
    xx: f64,
    xy: f64,
    yy: f64,
}

impl Moments {
    fn add(&mut self, o: &Moments) {
        self.n += o.n;
        self.x += o.x;
        self.y += o.y;
        self.xx += o.xx;
        self.xy += o.xy;
        self.yy += o.yy;
    }
}

/// Direcciones en las que se guarda hasta dónde llega cada banda, repartidas por
/// media vuelta. La otra media es la misma cambiada de signo.
///
/// El recorrido de una banda sobre el eje es lo que decide si el degradado la
/// explica, y hay que saberlo para un eje que todavía no está elegido. Sacarlo del
/// **rectángulo** que la contiene —que es lo primero que se le ocurre a uno— es una
/// cota malísima en cuanto la banda se curva: una franja de sombra que rodea un
/// brazo tiene un rectángulo enorme y un recorrido corto, y con esa cota se
/// rechazaban casi todas las rampas de un aerógrafo.
///
/// Con ocho direcciones lo que se guarda es la banda vista como un octógono en vez
/// de como un rectángulo, y un eje cualquiera se acota interpolando entre las dos
/// direcciones que lo abrazan. El error de esa interpolación es a lo sumo
/// `1/cos(π/16) - 1`, un 2%; con cuatro direcciones sería un 8%, y con dos —el
/// rectángulo— no hay cota que valga.
const DIRS: usize = 8;

/// Lo que hay que saber de una banda para decidir si entra en un grupo.
struct Band {
    moments: Moments,
    /// Hasta dónde llega la banda en cada dirección de [`DIRS`], por abajo y por
    /// arriba. Ver [`Band::span`].
    lo: [f32; DIRS],
    hi: [f32; DIRS],
    color: Rgba,
    lab: Oklab,
}

impl Band {
    fn centroid(&self) -> (f64, f64) {
        (
            self.moments.x / self.moments.n,
            self.moments.y / self.moments.n,
        )
    }

    /// Hasta dónde llega la banda sobre el modelo: el recorrido de alturas que
    /// cubre, acotado por fuera.
    ///
    /// Acotado y no exacto, y siempre de más: una cota holgada rechaza rampas
    /// legítimas, que es el lado seguro, mientras que una corta aceptaría grupos que
    /// el degradado no reproduce.
    fn extent(&self, model: &Model) -> (f64, f64) {
        match *model {
            Model::Linear { u } => self.span(u),
            // Para la distancia a un centro basta la caja alineada, que está
            // guardada exacta: las direcciones 0 y DIRS/2 son los dos ejes. El
            // mínimo es cero si el centro cae dentro, y el máximo, la esquina más
            // lejana.
            Model::Radial { c } => {
                let (x0, x1) = (f64::from(self.lo[0]), f64::from(self.hi[0]));
                let (y0, y1) = (f64::from(self.lo[DIRS / 2]), f64::from(self.hi[DIRS / 2]));
                let dx = (x0 - c.0).max(c.0 - x1).max(0.0);
                let dy = (y0 - c.1).max(c.1 - y1).max(0.0);
                let far = (x0 - c.0).abs().max((x1 - c.0).abs());
                let up = (y0 - c.1).abs().max((y1 - c.1).abs());
                ((dx * dx + dy * dy).sqrt(), (far * far + up * up).sqrt())
            }
        }
    }

    /// Hasta dónde llega la banda sobre el eje `u`, acotado por las dos
    /// direcciones guardadas que lo abrazan.
    ///
    /// `u` se escribe como `a·d_k + b·d_(k+1)` con los dos pesos positivos, y
    /// entonces el máximo de `u·p` no pasa de `a` por el máximo en `d_k` más `b`
    /// por el de `d_(k+1)`. Sigue siendo una cota y no el recorrido exacto, pero
    /// una que se pasa de larga un 2% en vez de multiplicarse por dos.
    fn span(&self, u: (f64, f64)) -> (f64, f64) {
        let step = std::f64::consts::PI / DIRS as f64;
        // A media vuelta: `-u` da el mismo recorrido con los extremos cambiados,
        // y así el índice siempre cae dentro.
        let angle = u.1.atan2(u.0).rem_euclid(std::f64::consts::PI);
        let k = ((angle / step) as usize).min(DIRS - 1);
        let a = ((k + 1) as f64 * step - angle).sin() / step.sin();
        let b = (angle - k as f64 * step).sin() / step.sin();
        // La dirección de después de la última es la primera dada la vuelta.
        let (lo_next, hi_next) = if k + 1 < DIRS {
            (f64::from(self.lo[k + 1]), f64::from(self.hi[k + 1]))
        } else {
            (-f64::from(self.hi[0]), -f64::from(self.lo[0]))
        };
        (
            a * f64::from(self.lo[k]) + b * lo_next,
            a * f64::from(self.hi[k]) + b * hi_next,
        )
    }
}

/// Las direcciones de [`DIRS`], en coseno y seno.
fn dirs() -> [(f32, f32); DIRS] {
    std::array::from_fn(|k| {
        let angle = std::f64::consts::PI * k as f64 / DIRS as f64;
        (angle.cos() as f32, angle.sin() as f32)
    })
}

/// Busca degradados y funde en uno cada grupo de bandas que lo sea.
///
/// Trabaja sobre `regions` ya construido y sobre las etiquetas del clustering,
/// que es de donde salen los momentos. Las regiones que se lleva un degradado
/// desaparecen de `regions.regions`.
pub fn merge(regions: &mut Regions, labels: &[u32], tolerance: f64, soft: &[bool]) {
    if tolerance <= 0.0 || regions.regions.is_empty() {
        return;
    }
    let bands = bands(regions, labels);
    let neighbours = neighbours(regions);
    let soft_pairs = soft_pairs(regions, soft);

    // De la región más grande a la más pequeña: una rampa se reconoce por sus
    // bandas anchas, y empezar por una mota daría un eje sacado de nada.
    let mut order: Vec<RegionId> = (0..regions.regions.len()).collect();
    order.sort_by(|&a, &b| {
        bands[b]
            .moments
            .n
            .total_cmp(&bands[a].moments.n)
            .then(a.cmp(&b))
    });

    let mut taken = vec![false; regions.regions.len()];
    let mut found: Vec<(Vec<RegionId>, Fit)> = Vec::new();
    for seed in order {
        if taken[seed] {
            continue;
        }
        if let Some((group, fit)) =
            grow(seed, &bands, &neighbours, &soft_pairs, &taken, tolerance)
        {
            for &id in &group {
                taken[id] = true;
            }
            found.push((group, fit));
        }
    }
    if found.is_empty() {
        return;
    }

    // Las figuras se arman con `regions.edges` todavía intacto, así que hasta
    // aquí no se toca nada de lo que dependen.
    // El modelo se elige aquí, con el grupo ya cerrado: ver [`Fit::best`].
    regions.ramps = found
        .into_iter()
        .map(|(group, fit)| {
            let fit = Fit::best(&group, &bands, fit);
            shape(regions, &group, &bands, &fit)
        })
        .collect();
    regions.regions = std::mem::take(&mut regions.regions)
        .into_iter()
        .zip(&taken)
        .filter_map(|(region, &taken)| (!taken).then_some(region))
        .collect();
}

/// Los momentos y el octógono de cada región, en una sola pasada por la imagen.
fn bands(regions: &Regions, labels: &[u32]) -> Vec<Band> {
    let n = regions.regions.len();
    let dirs = dirs();
    let mut moments = vec![Moments::default(); n];
    let mut lows = vec![[f32::INFINITY; DIRS]; n];
    let mut highs = vec![[f32::NEG_INFINITY; DIRS]; n];
    for (i, &label) in labels.iter().enumerate() {
        if label == NONE || label as usize >= n {
            continue;
        }
        let (x, y) = ((i % regions.width) as f64, (i / regions.width) as f64);
        let m = &mut moments[label as usize];
        m.n += 1.0;
        m.x += x;
        m.y += y;
        m.xx += x * x;
        m.xy += x * y;
        m.yy += y * y;
        let (lo, hi) = (&mut lows[label as usize], &mut highs[label as usize]);
        let (x, y) = (x as f32, y as f32);
        for (k, &(cos, sin)) in dirs.iter().enumerate() {
            let t = cos * x + sin * y;
            lo[k] = lo[k].min(t);
            hi[k] = hi[k].max(t);
        }
    }

    regions
        .regions
        .iter()
        .enumerate()
        .map(|(id, region)| Band {
            // Una región sin un solo píxel no existe, pero el tipo no lo impide y
            // dividir por su área sí que rompería.
            moments: if moments[id].n > 0.0 {
                moments[id]
            } else {
                Moments {
                    n: 1.0,
                    ..Default::default()
                }
            },
            lo: lows[id],
            hi: highs[id],
            color: region.color,
            lab: Oklab::from_rgba(region.color),
        })
        .collect()
}

/// Qué regiones tocan a cuáles, sacado de los tramos.
fn neighbours(regions: &Regions) -> Vec<Vec<RegionId>> {
    let mut out: Vec<Vec<RegionId>> = vec![Vec::new(); regions.regions.len()];
    for edge in &regions.edges {
        let Some(right) = edge.right else { continue };
        if edge.left == right {
            continue;
        }
        out[edge.left].push(right);
        out[right].push(edge.left);
    }
    for list in &mut out {
        list.sort_unstable();
        list.dedup();
    }
    out
}

/// Hace crecer un grupo desde una región, aceptando vecinas mientras el degradado
/// siga explicando a todas.
///
/// Cada candidata se prueba **una vez**: si el ajuste no la admite, se descarta
/// para este grupo. Volver a encolarla cada vez que el grupo crece —el ajuste ha
/// cambiado, luego podría entrar ahora— es tentador y sale carísimo: cada
/// reintento cuesta un ajuste entero, y en una imagen de 5 Mpx multiplicaba por
/// tres el tiempo de la conversión a cambio de tres degradados más. Lo que se
/// descarta aquí no se pierde: sigue libre para fundar su propio grupo.
fn grow(
    seed: RegionId,
    bands: &[Band],
    neighbours: &[Vec<RegionId>],
    soft_pairs: &HashSet<(RegionId, RegionId)>,
    taken: &[bool],
    tolerance: f64,
) -> Option<(Vec<RegionId>, Fit)> {
    let mut group = vec![seed];
    let mut best: Option<Fit> = None;

    let mut seen: HashSet<RegionId> = HashSet::from([seed]);
    let mut queue: Vec<RegionId> = Vec::new();
    let push = |queue: &mut Vec<RegionId>, seen: &mut HashSet<RegionId>, of: RegionId| {
        queue.extend(
            neighbours[of]
                .iter()
                .copied()
                .filter(|&n| !taken[n] && seen.insert(n)),
        );
    };
    push(&mut queue, &mut seen, seed);

    let mut head = 0;
    while head < queue.len() && group.len() < MAX_BANDS {
        // Mientras no haya tres colores el degradado no tiene que ganarse nada
        // —ver [`Fit::earns_it`]—, así que un damero de dos entradas en una zona de
        // ruido crece sin que nada lo pare, cuesta su cuadrado y acaba en la basura
        // por no llegar a [`MIN_COLORS`]. Ahí estaba el 95% del tiempo de esta
        // etapa en una imagen de 5 Mpx. Un degradado de verdad enseña su tercer
        // color en los primeros saltos.
        if group.len() >= BOOTSTRAP && best.as_ref().is_none_or(|f| f.stops.len() < MIN_COLORS) {
            break;
        }
        let candidate = queue[head];
        head += 1;
        group.push(candidate);
        match Fit::of(&group, bands).filter(|fit| fit.earns_it(&group, bands, tolerance)) {
            Some(fit) => {
                best = Some(fit);
                push(&mut queue, &mut seen, candidate);
            }
            None => {
                group.pop();
            }
        }
    }

    let fit = best?;
    // Dos caminos para entrar, y hace falta uno de los dos.
    //
    // Por **colores**: el número de paradas es el de colores distintos del grupo
    // —hay una por color— y con tres el modelo tiene que acertar un orden además de
    // un valor, que es lo que distingue una rampa de un promedio.
    //
    // O por **blandura**: dos bandas cuya costura el original pintó difuminada son
    // una transición, lo diga el recuento de colores o no. Es la única puerta que se
    // le abre a una pareja, y la abre una propiedad de la imagen y no del ajuste, que
    // es lo que impide que se cuele cualquier par de vecinas. Ver [`crate::softness`].
    let por_colores = fit.stops.len() >= MIN_COLORS;
    let por_blandura = group.len() >= 2 && todas_blandas(&group, soft_pairs);
    (por_colores || por_blandura).then_some((group, fit))
}

/// Cómo se pasa de una posición a la altura del degradado.
///
/// Son dos porque hay dos clases de sombreado y una no cabe en la otra. Un
/// `<linearGradient>` expresa el color como función de la proyección sobre un eje,
/// que es lo que es un cielo o una pared iluminada; el terminador de una superficie
/// redonda es función de la **distancia a un centro**, y con un eje sale
/// embadurnado en una dirección que no existe. Ese error concreto ya se cometió una
/// vez —dos caras estiradas a lo largo de una diagonal inventada, en la sesión del
/// 4c—, así que aquí el modelo es parte del ajuste y no una suposición.
#[derive(Clone, Copy, Debug)]
enum Model {
    /// Proyección sobre una dirección unitaria.
    Linear { u: (f64, f64) },
    /// Distancia a un centro.
    Radial { c: (f64, f64) },
}

impl Model {
    /// La altura del degradado en un punto.
    fn at(&self, p: (f64, f64)) -> f64 {
        match *self {
            Model::Linear { u } => u.0 * p.0 + u.1 * p.1,
            Model::Radial { c } => ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt(),
        }
    }
}

/// Las parejas de regiones cuya frontera compartida es blanda.
///
/// Una pareja y no una arista porque dos regiones pueden compartir varios tramos, y
/// lo que decide es el conjunto: basta que alguno sea blando para que la costura lo
/// sea, porque una transición difuminada que en un trozo se estreche sigue siendo la
/// misma transición.
fn soft_pairs(regions: &Regions, soft: &[bool]) -> HashSet<(RegionId, RegionId)> {
    let mut out = HashSet::new();
    for (id, edge) in regions.edges.iter().enumerate() {
        if soft.get(id).copied().unwrap_or(false) {
            if let Some(right) = edge.right {
                out.insert((edge.left.min(right), edge.left.max(right)));
            }
        }
    }
    out
}

/// Si todas las costuras **internas** del grupo son blandas.
///
/// Se pregunta por las internas y no por el contorno: lo que el degradado va a
/// borrar son las fronteras de dentro, y el borde de fuera se queda como esté.
fn todas_blandas(group: &[RegionId], soft_pairs: &HashSet<(RegionId, RegionId)>) -> bool {
    let mut alguna = false;
    for (i, &a) in group.iter().enumerate() {
        for &b in &group[i + 1..] {
            let par = (a.min(b), a.max(b));
            // Sólo cuentan las parejas que de verdad se tocan; dos bandas del grupo
            // que no comparten frontera no dicen nada.
            if soft_pairs.contains(&par) {
                alguna = true;
            }
        }
    }
    alguna
}

/// El degradado ajustado a un grupo: un modelo y una parada por color.
struct Fit {
    /// De posición a altura.
    model: Model,
    /// Paradas ordenadas por su posición sobre el eje: una por color, con el color
    /// ya en Oklab porque se pregunta muchas veces y la conversión lleva raíces
    /// cúbicas.
    stops: Vec<Stop>,
    /// Cuánto color abarca: la mayor distancia entre dos paradas. Ver
    /// [`Fit::earns_it`].
    reach: f64,
}

struct Stop {
    /// Posición sobre el eje, sin normalizar.
    at: f64,
    color: Rgba,
    lab: Oklab,
}

impl Fit {
    /// Ajusta el modelo lineal, que es con el que se hace crecer un grupo.
    fn of(group: &[RegionId], bands: &[Band]) -> Option<Fit> {
        Fit::with(Model::Linear { u: axis(group, bands)? }, group, bands)
    }

    /// El mejor de los modelos candidatos para un grupo ya cerrado: el eje que
    /// salió de hacerlo crecer y los centros radiales que su geometría sugiere.
    ///
    /// Se elige **al final** y no en cada paso del crecimiento por coste: el ajuste
    /// se rehace en cada candidata que se prueba, y multiplicarlo por el número de
    /// modelos multiplicaría la etapa entera. El precio es que el grupo lo forma el
    /// modelo lineal, así que una rampa curva de muchas bandas puede dejar de crecer
    /// antes de que le llegue el turno al radial. La barriga de un dibujo son dos
    /// bandas y no le pasa; un degradado radial de ocho, puede.
    fn best(group: &[RegionId], bands: &[Band], linear: Fit) -> Fit {
        let mut best = (linear.error(group, bands), linear);
        for c in centers(group, bands) {
            if let Some(fit) = Fit::with(Model::Radial { c }, group, bands) {
                let error = fit.error(group, bands);
                if error < best.0 {
                    best = (error, fit);
                }
            }
        }
        best.1
    }

    /// Coloca una parada por color sobre el modelo dado.
    fn with(model: Model, group: &[RegionId], bands: &[Band]) -> Option<Fit> {
        // Una parada por **color**, en el centro de todo lo que ese color pinta.
        // Por banda sería una parada por dato, y entonces no hay ajuste que pueda
        // fallar: el degradado acertaría cada banda en su centro por construcción,
        // y una mota es corta sobre cualquier eje. Ver el porqué arriba.
        let mut sum: Vec<(Rgba, f64, f64)> = Vec::new();
        for &id in group {
            let (cx, cy) = bands[id].centroid();
            let (w, t) = (bands[id].moments.n, model.at((cx, cy)));
            match sum.iter_mut().find(|(c, _, _)| *c == bands[id].color) {
                Some(entry) => {
                    entry.1 += w * t;
                    entry.2 += w;
                }
                None => sum.push((bands[id].color, w * t, w)),
            }
        }
        let mut stops: Vec<Stop> = sum
            .iter()
            .map(|&(color, wt, w)| Stop {
                at: wt / w,
                color,
                lab: color.into(),
            })
            .collect();
        stops.sort_by(|a, b| a.at.total_cmp(&b.at));
        let mut reach: f64 = 0.0;
        for (i, a) in stops.iter().enumerate() {
            for b in &stops[i + 1..] {
                reach = reach.max(a.lab.distance(&b.lab));
            }
        }
        Some(Fit {
            model,
            stops,
            reach,
        })
    }

    /// El color del degradado a la altura `t` del eje, interpolando en sRGB, que
    /// es lo que hace un navegador.
    fn at(&self, t: f64) -> Oklab {
        let stops = &self.stops;
        let i = stops.partition_point(|s| s.at <= t);
        if i == 0 {
            return stops[0].lab;
        }
        if i == stops.len() {
            return stops[stops.len() - 1].lab;
        }
        let (t0, c0) = (stops[i - 1].at, stops[i - 1].color);
        let (t1, c1) = (stops[i].at, stops[i].color);
        // Dos paradas de distinto color en el mismo sitio no se funden a
        // propósito: son un salto, y lo que dibuja un salto es el color de
        // después. Que una de las dos bandas quede entonces lejos del degradado es
        // exactamente lo que tiene que tumbar al grupo.
        let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
        let k = k.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (f64::from(a) + k * (f64::from(b) - f64::from(a))).round() as u8;
        Oklab::from_rgba(Rgba::new(
            mix(c0.r, c1.r),
            mix(c0.g, c1.g),
            mix(c0.b, c1.b),
            mix(c0.a, c1.a),
        ))
    }

    /// Si el degradado vale la pena: reproduce el color de todas las bandas, y lo
    /// hace explicando bastante más color del que se equivoca.
    ///
    /// Son **dos** condiciones y hacen falta las dos. El techo por sí solo se
    /// cumple gratis en cuanto todos los colores del grupo caben dentro de él: en
    /// una portada con trazo negro, las seis entradas oscuras que deja el *ringing*
    /// de un JPEG están todas a menos de dos tolerancias unas de otras, así que
    /// cualquier eje las «explica» y salían degradados de puro ruido. Lo que hace
    /// falta pedir es que el degradado gane: que su error sea [`GAIN`] veces menor
    /// que el recorrido de color que abarca. Un color plano lo cumpliría con error
    /// igual al recorrido, y ahí está la diferencia.
    ///
    /// El error de una banda se mira en los quiebros: los dos extremos de su
    /// recorrido sobre el eje y las paradas que caigan dentro. Entre dos paradas el
    /// color va por un segmento recto en sRGB, y sobre un escalón tan corto Oklab
    /// es prácticamente afín, así que esos puntos acotan la desviación.
    fn earns_it(&self, group: &[RegionId], bands: &[Band], tolerance: f64) -> bool {
        // Con menos colores que los que hace falta juntar, el degradado todavía no
        // puede ganar nada: dos paradas se equivocan la mitad de lo que abarcan por
        // pura aritmética, así que pedirle el triple ahí sería no dejar arrancar
        // ningún grupo. Hasta el tercer color manda sólo el techo.
        let ceiling = if self.stops.len() < MIN_COLORS {
            tolerance * CEILING
        } else {
            (tolerance * CEILING).min(self.reach / GAIN)
        };
        // Por el final: la que se acaba de proponer es la que suele fallar, y
        // mirarla primero corta el resto.
        group
            .iter()
            .rev()
            .all(|&id| self.band_error(id, bands) <= ceiling)
    }

    /// Lo peor que se equivoca el degradado en todo el grupo.
    ///
    /// Es lo que compara dos modelos entre sí, y por eso no puede cortar por lo
    /// sano como hace [`Fit::earns_it`]: ahí basta saber si pasa de un techo, y aquí
    /// hace falta el número.
    fn error(&self, group: &[RegionId], bands: &[Band]) -> f64 {
        group
            .iter()
            .map(|&id| self.band_error(id, bands))
            .fold(0.0, f64::max)
    }

    /// Lo que se equivoca el degradado dentro de una banda.
    ///
    /// Se mira en los quiebros: los dos extremos del recorrido de la banda sobre el
    /// modelo y las paradas que caigan dentro. Entre dos paradas el color va por un
    /// segmento recto en sRGB, y sobre un escalón tan corto Oklab es prácticamente
    /// afín, así que esos puntos acotan la desviación.
    fn band_error(&self, id: RegionId, bands: &[Band]) -> f64 {
        let band = &bands[id];
        let (lo, hi) = band.extent(&self.model);
        let first = self.stops.partition_point(|s| s.at <= lo);
        // Una banda que sobre el modelo no ocupa nada —un píxel suelto, o una que
        // cae perpendicular al eje— no tiene ninguna parada dentro, y ahí el segundo
        // corte se queda por detrás del primero.
        let last = self.stops.partition_point(|s| s.at < hi).max(first);
        let inner = self.stops[first..last].iter().map(|s| s.at);
        [lo, hi]
            .into_iter()
            .chain(inner)
            .map(|t| self.at(t).distance(&band.lab))
            .fold(0.0, f64::max)
    }
}

/// El eje por mínimos cuadrados de un grupo.
///
/// Sale de la regresión de Oklab sobre `(x, y)`, ponderada por área: da una matriz
/// de `3x2` cuyo mayor vector singular por la derecha es la dirección en la que más
/// cambia el color. Para `2x2` sale en cerrado.
fn axis(group: &[RegionId], bands: &[Band]) -> Option<(f64, f64)> {
    let mut total = Moments::default();
    for &id in group {
        total.add(&bands[id].moments);
    }
    let n = total.n;
    let (mx, my) = (total.x / n, total.y / n);
    // Covarianza de la posición, sobre los píxeles.
    let (cxx, cxy, cyy) = (
        total.xx / n - mx * mx,
        total.xy / n - mx * my,
        total.yy / n - my * my,
    );
    let det = cxx * cyy - cxy * cxy;
    if !det.is_finite() || det <= 1e-9 * cxx * cyy {
        // Todas las bandas en una recta o en un punto: no hay dos direcciones que
        // distinguir y el eje no está determinado.
        return None;
    }

    // Covarianza entre color y posición, con el color pesado por área.
    let mut cov = [(0.0, 0.0); 3];
    let mut mean = (0.0, 0.0, 0.0);
    for &id in group {
        let w = bands[id].moments.n / n;
        let lab = bands[id].lab;
        mean.0 += w * f64::from(lab.l);
        mean.1 += w * f64::from(lab.a);
        mean.2 += w * f64::from(lab.b);
    }
    for &id in group {
        let w = bands[id].moments.n / n;
        let (cx, cy) = bands[id].centroid();
        let lab = bands[id].lab;
        let d = [
            f64::from(lab.l) - mean.0,
            f64::from(lab.a) - mean.1,
            f64::from(lab.b) - mean.2,
        ];
        for (c, dc) in cov.iter_mut().zip(d) {
            c.0 += w * dc * (cx - mx);
            c.1 += w * dc * (cy - my);
        }
    }

    // Gradiente de cada canal: la covarianza por la inversa de la de posición.
    let inv = (cyy / det, -cxy / det, cxx / det);
    let g: Vec<(f64, f64)> = cov
        .iter()
        .map(|c| (c.0 * inv.0 + c.1 * inv.1, c.0 * inv.1 + c.1 * inv.2))
        .collect();

    // Dirección dominante: mayor autovector de la suma de `g·gᵀ`.
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    for &(gx, gy) in &g {
        a += gx * gx;
        b += gx * gy;
        c += gy * gy;
    }
    dominant(a, b, c)
}

/// Centros candidatos para un degradado radial.
///
/// Un sombreado radial tiene su color extremo en el centro: el brillo de una
/// superficie redonda está donde la luz cae de frente y el tono se apaga hacia
/// fuera. Así que se prueban los centroides de los dos colores extremos del grupo
/// —el más claro y el más oscuro sobre el recorrido de color— y el del grupo entero,
/// que es el caso de una rampa centrada.
///
/// Son tres candidatos y no un ajuste: el centro entra no lineal en el modelo, y
/// ajustarlo de verdad pide iterar. Tres tiros bastan para lo que hay que decidir,
/// que es si el sombreado es radial **o** lineal, y de acertar el centro se encarga
/// el que menos se equivoca.
fn centers(group: &[RegionId], bands: &[Band]) -> Vec<(f64, f64)> {
    // Los dos colores más separados del grupo, que son los extremos de la rampa.
    let mut extremos: Option<(Rgba, Rgba)> = None;
    let mut mayor = 0.0;
    for (i, &a) in group.iter().enumerate() {
        for &b in &group[i + 1..] {
            let d = bands[a].lab.distance(&bands[b].lab);
            if d > mayor {
                mayor = d;
                extremos = Some((bands[a].color, bands[b].color));
            }
        }
    }
    // El centroide de todo lo que pinta un color, no el de una banda suelta: un
    // color puede estar repartido en varias.
    let centroide = |quien: Option<Rgba>| {
        let (mut cx, mut cy, mut n) = (0.0, 0.0, 0.0);
        for &id in group {
            if quien.is_some_and(|c| c != bands[id].color) {
                continue;
            }
            let w = bands[id].moments.n;
            let (bx, by) = bands[id].centroid();
            cx += w * bx;
            cy += w * by;
            n += w;
        }
        (n > 0.0).then(|| (cx / n, cy / n))
    };
    let (a, b) = match extremos {
        Some((a, b)) => (Some(a), Some(b)),
        None => (None, None),
    };
    [centroide(a), centroide(b), centroide(None)]
        .into_iter()
        .flatten()
        .collect()
}

/// Autovector dominante de la simétrica `[[a, b], [b, c]]`, normalizado.
fn dominant(a: f64, b: f64, c: f64) -> Option<(f64, f64)> {
    let trace = a + c;
    if !trace.is_finite() || trace <= 0.0 {
        // Color constante: no hay degradado que ajustar.
        return None;
    }
    let disc = ((a - c) * (a - c) + 4.0 * b * b).max(0.0).sqrt();
    let lambda = (trace + disc) / 2.0;
    // De las dos filas de `M - lambda·I` se toma la de mayor norma: la otra puede
    // ser nula si el autovalor está repetido.
    let (vx, vy) = if (b.abs()).max((a - lambda).abs()) >= (b.abs()).max((c - lambda).abs()) {
        (b, lambda - a)
    } else {
        (lambda - c, b)
    };
    let norm = (vx * vx + vy * vy).sqrt();
    if norm < 1e-12 {
        // Isótropo: el color cambia igual en todas direcciones, que no es una
        // rampa por mucho que el ajuste sea bueno.
        return None;
    }
    Some((vx / norm, vy / norm))
}

/// Arma la figura: el contorno de la unión y el degradado ya en coordenadas.
fn shape(regions: &Regions, group: &[RegionId], bands: &[Band], fit: &Fit) -> Ramp {
    let member: HashSet<RegionId> = group.iter().copied().collect();

    // El contorno de la unión son los tramos con exactamente un lado dentro,
    // orientados con el grupo a la izquierda. Los de dentro se caen solos.
    let mut uses: Vec<(EdgeId, bool)> = Vec::new();
    for (id, edge) in regions.edges.iter().enumerate() {
        let left = member.contains(&edge.left);
        let right = edge.right.is_some_and(|r| member.contains(&r));
        if left && !right {
            uses.push((id, false));
        } else if right && !left {
            uses.push((id, true));
        }
    }

    // El eje va de la primera parada a la última, y no de un extremo del grupo al
    // otro: fuera del eje `pad` repite el color del extremo, que es exactamente lo
    // que hay antes de la primera parada y después de la última. Se dibuja igual y
    // las posiciones salen en `0..1` cerrado en vez de metidas en un trozo.
    let lo = fit
        .stops
        .first()
        .expect("un grupo aceptado tiene paradas")
        .at;
    let hi = fit
        .stops
        .last()
        .expect("un grupo aceptado tiene paradas")
        .at;
    let (mut cx, mut cy, mut n) = (0.0, 0.0, 0.0);
    for &id in group {
        let w = bands[id].moments.n;
        let (bx, by) = bands[id].centroid();
        cx += w * bx;
        cy += w * by;
        n += w;
    }
    let (cx, cy) = (cx / n, cy / n);

    // Cada modelo lleva sus paradas a `0..1` a su manera, y la diferencia no es
    // cosmética: en un radial el cero **es** el centro, no la primera parada, así
    // que el radio se mide desde ahí y la parada de dentro se queda donde le toca.
    // Antes de la primera parada y después de la última, un degradado repite el
    // color del extremo, que es exactamente lo que hay al otro lado.
    let (axis, escala): (Axis, Box<dyn Fn(f64) -> f64>) = match fit.model {
        Model::Linear { u } => {
            let tc = u.0 * cx + u.1 * cy;
            let at = |t: f64| (cx + u.0 * (t - tc), cy + u.1 * (t - tc));
            let span = (hi - lo).max(f64::EPSILON);
            (
                Axis::Linear {
                    from: at(lo),
                    to: at(hi),
                },
                Box::new(move |t| (t - lo) / span),
            )
        }
        Model::Radial { c } => {
            let radius = hi.max(f64::EPSILON);
            (
                Axis::Radial { center: c, radius },
                Box::new(move |t| t / radius),
            )
        }
    };

    Ramp {
        rings: crate::boundary::rings(&regions.edges, &uses),
        axis,
        stops: fit
            .stops
            .iter()
            .map(|s| (escala(s.at).clamp(0.0, 1.0), s.color))
            .collect(),
        bands: group.len(),
    }
}
