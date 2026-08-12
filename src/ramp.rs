//! Degradados: cuando un montón de bandas son en realidad una sola figura.
//!
//! La paleta reparte una rampa continua en escalones, porque una región tiene un
//! color y nada más. En una foto eso es la mayor parte del documento: un cielo
//! sale a doce bandas, con once fronteras entre ellas que no dibujan nada —sólo
//! marcan por dónde cruzó la rampa un umbral de cuantización— y que son los
//! contornos más retorcidos de todo el SVG, porque siguen el ruido del original.
//!
//! Un SVG sí sabe decir «degradado». Esto encuentra los grupos de bandas que lo
//! son, los funde en una figura y los pinta con un `<linearGradient>` o un
//! `<radialGradient>`, según cuál de los dos lo explique.
//!
//! # Qué es ser una rampa
//!
//! Un degradado de SVG expresa exactamente una cosa: **el color como función de una
//! altura**, y la altura es la proyección sobre un eje o la distancia a un centro.
//! Así que el criterio es esa misma cosa, con la condición de que el degradado sirva
//! de algo:
//!
//! > Un grupo de regiones vecinas es una rampa cuando un solo degradado —lineal o
//! > radial— reproduce el color de todas ellas con un error que no pasa de
//! > [`CEILING`] tolerancias **ni de la [`GAIN`]-ésima parte del color que abarca**.
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
//! # Los dos modelos
//!
//! Hacen falta los dos y ninguno suple al otro: un cielo o una pared iluminada son
//! función de la proyección sobre un eje, y el terminador de una superficie redonda
//! es función de la distancia a un punto, que con un eje sale embadurnado a lo largo
//! de una dirección que el dibujo no tiene.
//!
//! Cuál de los dos lo decide el error de cada uno, con el desempate a favor del eje
//! ([`PREFER`]). El centro del radial sale de la **curvatura de la costura** y no de
//! los colores: la línea de nivel de un degradado radial es un círculo, así que la
//! costura entre dos de sus bandas es un arco, y ajustarlo da el centro ([`circle`]).
//!
//! Y no todo grupo puede aspirar al radial: sólo se le prueba a los que tienen una
//! costura **blanda**, que es la señal de que ahí hubo un sombreado y no un canto
//! (ver [`crate::softness`]). Eso es lo que acota su coste, que no es despreciable:
//! el recorrido de una banda sobre una distancia no se puede acotar por fuera con
//! nada convexo —un anillo contiene su centro— y hay que medirlo sobre los píxeles,
//! una pasada por imagen y centro. En un cartel de color plano no se mide ninguno.
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

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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

/// Cuánto mejor tiene que salirle el modelo radial a un grupo para quedárselo.
///
/// El desempate va al eje a propósito, y no es cosmética: un `<linearGradient>` es
/// la descripción que un editor vectorial y un humano esperan de un sombreado
/// tendido, y una costura con un radio de curvatura de trescientos píxeles la
/// explican los dos igual de bien. Sin margen, el radial gana esos empates por unas
/// milésimas y el documento sale lleno de círculos enormes centrados fuera del
/// lienzo, que dibujan exactamente lo mismo.
///
/// Medido en un barrido de costuras del mismo par de colores, del arco cerrado a la
/// recta —error del eje contra error del centro, en milésimas de Oklab:
///
/// | radio de la costura | eje | centro | se queda |
/// | --- | --- | --- | --- |
/// | 20 px | 119 | 69 | centro |
/// | 40 px | 104 | 78 | centro |
/// | 60 px | 86 | 75 | eje |
/// | 100 px | 83 | 77 | eje |
/// | 180 px, casi recta | 81 | 78 | eje |
///
/// El curvo gana siempre, porque el arco es de verdad un arco; lo que cambia es
/// cuánto. Con `0,8` se lo queda mientras la curvatura se note y lo suelta cuando la
/// costura es casi recta, que es donde las dos descripciones dibujan lo mismo.
const PREFER: f64 = 0.8;

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
    let seams = soft_seams(regions, soft);
    let mut reaches = Reaches::new(labels, bands.len(), regions.width);
    // Hasta dónde puede irse el centro de un degradado radial: más allá y lo que
    // describe es un degradado lineal, que ya está entre los candidatos.
    let limit = 4.0 * (regions.width as f64).hypot(regions.height as f64);

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
        if let Some((group, fit)) = grow(
            seed,
            &bands,
            &neighbours,
            &seams,
            &mut reaches,
            limit,
            &taken,
            tolerance,
        ) {
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
    // aquí no se toca nada de lo que dependen. El modelo de cada grupo ya viene
    // elegido: lo eligió el crecimiento, ver [`Fit::best_of`].
    regions.ramps = found
        .into_iter()
        .map(|(group, fit)| shape(regions, &group, &bands, &fit))
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
#[allow(clippy::too_many_arguments)]
fn grow(
    seed: RegionId,
    bands: &[Band],
    neighbours: &[Vec<RegionId>],
    seams: &HashMap<(RegionId, RegionId), Seam>,
    reaches: &mut Reaches,
    limit: f64,
    taken: &[bool],
    tolerance: f64,
) -> Option<(Vec<RegionId>, Fit)> {
    let mut group = vec![seed];
    let mut best: Option<Fit> = None;
    // El modelo radial del grupo, si alguna de sus costuras blandas se curva, y la
    // longitud de la costura de la que salió: manda la más larga, que es la que
    // mejor determina un círculo.
    let mut radial: Option<Model> = None;
    let mut largo = 0usize;

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
        // La costura de la candidata con el grupo: si es blanda hay sombreado, y si
        // además se curva, su centro de curvatura es el del degradado radial.
        //
        // Se cambia de centro sólo por una costura la mitad más larga que la que
        // manda, y no por cualquiera que lo sea un poco: cada centro nuevo cuesta una
        // pasada por la imagen, y en un dibujo con ruido las costuras cortas de un
        // mismo sombreado ajustan círculos que se parecen y no aportan nada.
        let (mut radial_ahora, mut largo_ahora) = (radial.clone(), largo);
        for &m in &group {
            let Some(seam) = seams.get(&(m.min(candidate), m.max(candidate))) else {
                continue;
            };
            if seam.points.len() * 2 <= largo_ahora * 3 {
                continue;
            }
            if let Some(c) = circle(&seam.points, limit) {
                largo_ahora = seam.points.len();
                radial_ahora = Some(Model::Radial {
                    c,
                    reach: reaches.get(c),
                });
            }
        }
        group.push(candidate);
        match Fit::best_of(&group, bands, radial_ahora.as_ref(), tolerance) {
            Some(fit) => {
                best = Some(fit);
                (radial, largo) = (radial_ahora, largo_ahora);
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
    let por_blandura = group.len() >= 2 && alguna_blanda(&group, neighbours, seams);
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
#[derive(Clone)]
enum Model {
    /// Proyección sobre una dirección unitaria.
    Linear { u: (f64, f64) },
    /// Distancia a un centro, con las alturas de cada banda ya medidas **sobre los
    /// píxeles** para ese centro. Ver [`Reach`].
    Radial { c: (f64, f64), reach: Rc<Reach> },
}

impl Model {
    /// A qué altura está la masa de una banda, que es donde va su parada.
    ///
    /// Es la altura **media sobre los píxeles**, y no la altura del centroide, que
    /// es lo que parece lo mismo y no lo es: para una distancia las dos cuentas se
    /// separan tanto como quiera la forma de la banda, y en el peor caso —una banda
    /// en anillo— el centroide es el centro y su altura sale cero, o sea la parada
    /// del color de dentro puesta encima de la del de fuera. Con el eje sí son la
    /// misma cosa, porque una proyección es lineal.
    ///
    /// Ese error concreto ya se cometió: los degradados radiales salían con las dos
    /// paradas pegadas y el error de todas las bandas al máximo, así que un disco
    /// sombreado desde dentro se rechazaba entero.
    fn height(&self, id: RegionId, bands: &[Band]) -> f64 {
        match self {
            Model::Linear { u } => {
                let (cx, cy) = bands[id].centroid();
                u.0 * cx + u.1 * cy
            }
            Model::Radial { reach, .. } => reach.of(id).2,
        }
    }

    /// El recorrido de alturas que cubre una banda: entre qué dos valores se mueve
    /// el degradado dentro de ella.
    ///
    /// Con el eje es una **cota** superior, sacada del octógono de la banda, y
    /// siempre de más: una cota holgada rechaza rampas legítimas, que es el lado
    /// seguro, mientras que una corta aceptaría grupos que el degradado no
    /// reproduce.
    ///
    /// Con la distancia a un centro no vale ninguna cota convexa, y por eso el
    /// recorrido radial se mide exacto. Las bandas de un degradado radial son
    /// **anillos**, y cualquier convexo que contenga un anillo contiene su centro:
    /// el mínimo sale 0 en vez del radio interior, el degradado se evalúa en el
    /// color del centro y el error de todos los anillos se dispara. Con la caja
    /// alineada —el primer intento— un disco sombreado desde un punto de dentro
    /// salía a cero degradados.
    fn extent(&self, id: RegionId, bands: &[Band]) -> (f64, f64) {
        match self {
            Model::Linear { u } => bands[id].span(*u),
            Model::Radial { reach, .. } => {
                let (lo, hi, _) = reach.of(id);
                (lo, hi)
            }
        }
    }
}

/// Los píxeles de cada banda, agrupados por banda.
///
/// Sirve para poder recorrer **una** banda sin recorrer la imagen, que es lo que
/// hace asequible medir alturas radiales exactas: un grupo con costura blanda son
/// dos o tres bandas de un dibujo con cuatrocientas, y medir sobre la imagen entera
/// costaba una pasada completa por centro —46 pasadas en el aerógrafo y 33 en la
/// portada, o 20 y 40 ms de los 130 y 90 que cuesta la conversión.
///
/// Se construye una sola vez y sólo si hace falta: en un cartel de color plano no
/// llega a construirse.
struct Pixels {
    /// Índices de píxel ordenados por banda.
    index: Vec<u32>,
    /// Dónde empieza cada banda dentro de `index`.
    offsets: Vec<usize>,
    width: usize,
}

impl Pixels {
    fn new(labels: &[u32], bands: usize, width: usize) -> Pixels {
        let mut offsets = vec![0usize; bands + 1];
        for &label in labels {
            if label != NONE && (label as usize) < bands {
                offsets[label as usize + 1] += 1;
            }
        }
        for i in 0..bands {
            offsets[i + 1] += offsets[i];
        }
        let mut at = offsets.clone();
        let mut index = vec![0u32; offsets[bands]];
        for (i, &label) in labels.iter().enumerate() {
            if label != NONE && (label as usize) < bands {
                index[at[label as usize]] = i as u32;
                at[label as usize] += 1;
            }
        }
        Pixels {
            index,
            offsets,
            width,
        }
    }

    fn of(&self, id: RegionId) -> &[u32] {
        &self.index[self.offsets[id]..self.offsets[id + 1]]
    }
}

/// Las alturas de las bandas para un centro radial, medidas sobre los píxeles.
///
/// Aquí no hay atajo por momentos como con el eje: la distancia a un punto no es
/// lineal, así que ni el recorrido ni la media salen de la posición media de la
/// banda, y las dos cosas hacen falta exactas. Ver [`Model::height`] y
/// [`Model::extent`].
///
/// Cada banda se mide la primera vez que se pregunta por ella, no todas de golpe: un
/// grupo pregunta por las suyas y por las de las candidatas que prueba, que son unas
/// pocas de todas las del dibujo.
struct Reach {
    c: (f64, f64),
    pixels: Rc<Pixels>,
    /// Por banda: recorrido y altura media. En una celda mutable porque se rellena al
    /// preguntar, y se pregunta desde un ajuste que no se considera mutable —lo que
    /// cambia es la caché, no la respuesta.
    heights: RefCell<HashMap<RegionId, (f64, f64, f64)>>,
}

impl Reach {
    /// El recorrido y la altura media de una banda: `(min, max, media)`.
    fn of(&self, id: RegionId) -> (f64, f64, f64) {
        if let Some(&hit) = self.heights.borrow().get(&id) {
            return hit;
        }
        let (mut lo, mut hi, mut sum) = (f64::INFINITY, f64::NEG_INFINITY, 0.0);
        let px = self.pixels.of(id);
        for &i in px {
            let i = i as usize;
            let (x, y) = (
                (i % self.pixels.width) as f64,
                (i / self.pixels.width) as f64,
            );
            let d = (x - self.c.0).hypot(y - self.c.1);
            lo = lo.min(d);
            hi = hi.max(d);
            sum += d;
        }
        // Una banda sin un solo píxel no existe, pero el tipo no lo impide y dividir
        // por su área sí que rompería.
        let out = if px.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            (lo, hi, sum / px.len() as f64)
        };
        self.heights.borrow_mut().insert(id, out);
        out
    }
}

/// Las alturas radiales ya medidas, por centro.
///
/// Los centros son pocos —uno por grupo con costura blanda, y repetidos entre grupos
/// vecinos, que la caché junta—, y aun así conviene no repetirlos: lo que se guarda
/// aquí es lo ya medido para cada uno.
struct Reaches<'a> {
    labels: &'a [u32],
    bands: usize,
    width: usize,
    pixels: Option<Rc<Pixels>>,
    /// Por centro redondeado al píxel: dos centros que caen en el mismo píxel dan las
    /// mismas alturas con la precisión que esto necesita.
    cache: HashMap<(i64, i64), Rc<Reach>>,
}

impl<'a> Reaches<'a> {
    fn new(labels: &'a [u32], bands: usize, width: usize) -> Self {
        Reaches {
            labels,
            bands,
            width,
            pixels: None,
            cache: HashMap::new(),
        }
    }

    fn get(&mut self, c: (f64, f64)) -> Rc<Reach> {
        let key = (c.0.round() as i64, c.1.round() as i64);
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let labels = self.labels;
        let (bands, width) = (self.bands, self.width);
        let pixels = self
            .pixels
            .get_or_insert_with(|| Rc::new(Pixels::new(labels, bands, width)))
            .clone();
        let reach = Rc::new(Reach {
            c,
            pixels,
            heights: RefCell::new(HashMap::new()),
        });
        self.cache.insert(key, reach.clone());
        reach
    }
}

/// Lo que se sabe de la costura blanda entre dos regiones.
///
/// Una pareja y no una arista porque dos regiones pueden compartir varios tramos, y
/// lo que decide es el conjunto: basta que alguno sea blando para que la costura lo
/// sea, porque una transición difuminada que en un trozo se estreche sigue siendo la
/// misma transición.
struct Seam {
    /// Puntos de la costura, para el ajuste de círculo. Se guardan y no se ajusta
    /// aquí porque sólo hace falta el círculo de las costuras que acaban dentro de
    /// un grupo, que son muchas menos.
    points: Vec<(i32, i32)>,
}

/// Las costuras blandas, por pareja de regiones.
fn soft_seams(regions: &Regions, soft: &[bool]) -> HashMap<(RegionId, RegionId), Seam> {
    let mut out: HashMap<(RegionId, RegionId), Seam> = HashMap::new();
    for (id, edge) in regions.edges.iter().enumerate() {
        if !soft.get(id).copied().unwrap_or(false) {
            continue;
        }
        if let Some(right) = edge.right {
            out.entry((edge.left.min(right), edge.left.max(right)))
                .or_insert_with(|| Seam { points: Vec::new() })
                .points
                .extend_from_slice(&edge.points);
        }
    }
    out
}

/// El centro del círculo que mejor pasa por unos puntos, o nada si no hay círculo
/// que valga.
///
/// Es de dónde sale el centro de un degradado radial, y no de los colores: la línea
/// de nivel de un degradado radial **es** un círculo, así que la costura entre dos de
/// sus bandas es un arco y ajustarlo da el centro directamente. Sacarlo del centroide
/// del color más claro del grupo —que fue el primer intento— falla en cuanto el grupo
/// no llega al centro: el centroide de un arco está en el arco, no en el centro de
/// curvatura.
///
/// El ajuste es el algebraico: con `x² + y² = ax + by + c` la incógnita entra lineal
/// y sale de un sistema de 3x3. Es sensible al ruido cuando el arco es corto, y da
/// igual, porque quien decide si el modelo sirve es el error de después, que se mide
/// exacto.
fn circle(points: &[(i32, i32)], limit: f64) -> Option<(f64, f64)> {
    if points.len() < 8 {
        return None;
    }
    // Centrado en la media para que el sistema no dependa de dónde esté el dibujo.
    let n = points.len() as f64;
    let (mx, my) = points.iter().fold((0.0, 0.0), |acc, p| {
        (acc.0 + f64::from(p.0) / n, acc.1 + f64::from(p.1) / n)
    });
    let (mut sxx, mut sxy, mut syy, mut sxz, mut syz) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for p in points {
        let (x, y) = (f64::from(p.0) - mx, f64::from(p.1) - my);
        let z = x * x + y * y;
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        sxz += x * z;
        syz += y * z;
    }
    let det = sxx * syy - sxy * sxy;
    // Puntos en línea recta: no hay centro de curvatura que estimar, y una costura
    // recta ya la explica el eje.
    if !det.is_finite() || det.abs() <= 1e-9 * sxx.max(syy).max(1.0).powi(2) {
        return None;
    }
    let (a, b) = (
        (sxz * syy - syz * sxy) / (2.0 * det),
        (syz * sxx - sxz * sxy) / (2.0 * det),
    );
    // Un centro que se va del lienzo por una tangente casi recta describe un
    // degradado lineal, que ya está entre los candidatos y sale más barato.
    (a.hypot(b) <= limit).then_some((a + mx, b + my))
}

/// Si alguna costura **interna** del grupo es blanda.
///
/// Se pregunta por las internas y no por el contorno: lo que el degradado va a
/// borrar son las fronteras de dentro, y el borde de fuera se queda como esté.
///
/// Y basta **una**, que es lo que parece flojo y no lo es. Se probó a pedirlas todas,
/// razonando que ésta es la única puerta para un grupo que no llega a [`MIN_COLORS`]
/// colores y que con una sola costura difuminada se colaría un damero de dos entradas
/// en una zona de ruido. Medido, lo que se cuela no es ruido: pedirlas todas deja el
/// aerógrafo con los mismos 21 degradados repartidos en **15 bandas más** —una
/// transición que en un trozo se estreche hasta parecer canto sigue siendo la misma
/// transición— y le quita dos a la portada, sin que se vea nada mejor en ninguna de
/// las dos. Del ruido no protege esto, sino [`Fit::earns_it`] y [`BOOTSTRAP`].
fn alguna_blanda(
    group: &[RegionId],
    neighbours: &[Vec<RegionId>],
    seams: &HashMap<(RegionId, RegionId), Seam>,
) -> bool {
    let member: HashSet<RegionId> = group.iter().copied().collect();
    let mut alguna = false;
    for &a in group {
        // Sólo cuentan las parejas que de verdad se tocan; dos bandas del grupo que
        // no comparten frontera no dicen nada de ninguna transición.
        for &b in neighbours[a].iter().filter(|b| member.contains(b)) {
            if seams.contains_key(&(a.min(b), a.max(b))) {
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
    /// El mejor de los modelos candidatos para un grupo: el eje por mínimos
    /// cuadrados y, si `radial`, los centros que su geometría sugiere.
    ///
    /// «Mejor» es el que menos se equivoca en la banda que peor le sale, que es la
    /// magnitud con la que luego se decide si el grupo vale ([`Fit::earns_it`]), así
    /// que elegir aquí el de menor error y filtrar después es lo mismo que preguntar
    /// si **algún** modelo aguanta el grupo.
    ///
    /// Se elige en cada paso del crecimiento y no al final, que fue el primer
    /// intento y no vale: el grupo lo forma el modelo con el que se prueba cada
    /// candidata, así que con el eje solo una costura curvada se rechaza —el
    /// recorrido de una banda curva sobre un eje es larguísimo y el error se
    /// dispara— y al radial no le llega el turno nunca. Medido: un disco sombreado
    /// desde un punto de dentro salía a **cero** degradados eligiendo al final.
    ///
    /// Lo que acota el coste de probar dos modelos en vez de uno es la puerta: el
    /// radial llega ya construido y sólo lo construye una costura blanda que se
    /// curva, que es donde puede haber un sombreado redondo. En un cartel de color
    /// plano no se prueba nunca.
    fn best_of(
        group: &[RegionId],
        bands: &[Band],
        radial: Option<&Model>,
        tolerance: f64,
    ) -> Option<Fit> {
        // Primero quién aguanta el grupo y sólo después quién lo explica mejor, y en
        // ese orden: al revés —elegir el mejor y luego comprobarlo— la preferencia
        // por el eje tira grupos que el centro sí sostiene. Medido en el aerógrafo:
        // los mismos 21 degradados repartidos en 15 bandas menos.
        let medir = |fit: Fit| {
            fit.earns_it(group, bands, tolerance)
                .then(|| (fit.error(group, bands), fit))
        };
        let linear =
            axis(group, bands).and_then(|u| medir(Fit::with(Model::Linear { u }, group, bands)));
        let radial = radial.and_then(|model| medir(Fit::with(model.clone(), group, bands)));
        match (linear, radial) {
            (Some((recto, fit)), Some((curvo, redondo))) => {
                Some(if curvo < recto * PREFER { redondo } else { fit })
            }
            (uno, otro) => uno.or(otro).map(|(_, fit)| fit),
        }
    }

    /// Coloca una parada por color sobre el modelo dado.
    fn with(model: Model, group: &[RegionId], bands: &[Band]) -> Fit {
        // Una parada por **color**, en el centro de todo lo que ese color pinta.
        // Por banda sería una parada por dato, y entonces no hay ajuste que pueda
        // fallar: el degradado acertaría cada banda en su centro por construcción,
        // y una mota es corta sobre cualquier eje. Ver el porqué arriba.
        let mut sum: Vec<(Rgba, f64, f64)> = Vec::new();
        for &id in group {
            let (w, t) = (bands[id].moments.n, model.height(id, bands));
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
        Fit {
            model,
            stops,
            reach,
        }
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
        let (lo, hi) = self.model.extent(id, bands);
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
        Model::Radial { c, .. } => {
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
