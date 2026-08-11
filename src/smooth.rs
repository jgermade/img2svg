//! Regularización espacial de la asignación de paleta.
//!
//! [`crate::cluster::Palette`] decide las entradas mirando la imagen entera y
//! luego asigna **cada píxel por separado** a la más cercana. Nada en esa cadena
//! sabe que un píxel tiene vecinos, y ahí está el problema: en cuanto el ruido
//! local de una zona se acerca a la tolerancia, píxeles contiguos de lo que el
//! ojo lee como un color liso caen en entradas distintas, y el etiquetado por
//! componentes convierte ese parpadeo en islas.
//!
//! Medido sobre `Sonic1.png` —un aerógrafo escaneado, con su grano—, en ventanas
//! de 24x24 que se ven de un solo color, distancia en Oklab de cada píxel a la
//! media de su ventana:
//!
//! | ventana | mediana | p90 | máx |
//! | --- | --- | --- | --- |
//! | azul de la cabeza | 0,0089 | 0,0320 | 0,0712 |
//! | azul de las púas | 0,0078 | 0,0219 | 0,0494 |
//! | blanco de la cara | 0,0038 | 0,0071 | 0,0130 |
//!
//! Contra una tolerancia de 0,045: uno de cada diez píxeles del azul está más
//! lejos de la media de sus vecinos que la tolerancia entera. De ahí salían
//! 12.954 regiones, 8.911 de ellas de menos de 16 px, que pintaban el 6% del
//! lienzo y eran el 97% de los paths.
//!
//! # Por qué no vale el voto por mayoría
//!
//! Lo obvio —cada píxel toma la etiqueta más frecuente de su vecindad— quita el
//! ruido y **se lleva por delante los trazos finos**. Una línea de un píxel de
//! ancho tiene, en su propia vecindad de 3x3, tres píxeles suyos contra seis del
//! fondo: la mayoría vota siempre contra la línea, por negra que sea sobre por
//! blanco que esté el fondo. Y un dibujo de trazo —el caso que hay que arreglar—
//! es justo eso.
//!
//! El voto falla porque sólo cuenta vecinos. Le falta la otra mitad: **cuánto se
//! parece el píxel al color que se le propone**. Un píxel de grano está a medio
//! camino entre dos entradas —por eso saltó— y cambiarlo no cuesta casi nada; un
//! píxel de una línea negra sobre blanco está a 0,9 de la entrada del fondo, y
//! eso no lo compra ninguna mayoría.
//!
//! # Lo que se hace en su lugar
//!
//! Cada píxel se queda con la entrada que minimice
//!
//! ```text
//!     coste(c) = distancia(color del píxel, c) + beta * vecinos que no son c
//! ```
//!
//! Es un paso de ICM sobre un campo de Markov: el primer término tira de los
//! datos y el segundo de la coherencia con el vecindario, y `beta` dice cuánto
//! vale un vecino en unidades de distancia de color. Se itera unas pocas veces,
//! siempre fuera de sitio —los cambios de una pasada se aplican al final— para
//! que el resultado no dependa del orden del recorrido.
//!
//! # Lo que no puede tocar
//!
//! [`crate::cluster`] promete que ningún píxel se pinta a más de `tolerance` de
//! su color, y esa promesa es de la biblioteca, no un detalle interno: es lo que
//! hace que subir la tolerancia signifique algo. Mover píxeles por razones
//! geométricas la rompería sin más, así que aquí hay un techo: un píxel sólo
//! puede acabar en una entrada que esté a menos de [`CEILING`] veces lo que ya
//! estaba dispuesto a aceptar.
//!
//! El techo no se eligió por bonito. Medido sobre `Sonic1.png` con dos pasadas,
//! subtrazados emitidos según dónde se ponga:
//!
//! | techo | subtrazados |
//! | --- | --- |
//! | 1x la tolerancia | 23.804 |
//! | 1,5x | 17.468 |
//! | **2x** | **16.959** |
//! | sin techo | 16.957 |
//!
//! A `2x` no queda nada por ganar —dos subtrazados de diferencia sobre diecisiete
//! mil—, así que la restricción sale gratis y se queda. A `1x` sí cuesta: un
//! píxel que parpadea está *justo* en la frontera de decisión, y de ahí su
//! distancia a la otra entrada es la tolerancia y un pelo más. Cortar en la
//! tolerancia exacta prohíbe precisamente el caso que hay que arreglar.
//!
//! Aparte del techo hay una propiedad que sale de balde y vale más: los
//! candidatos son sólo las entradas **presentes en su vecindad**, así que un
//! píxel nunca se pinta de un color que no se estuviera ya pintando pegado a él.
//!
//! # La gracia
//!
//! El mismo criterio hace las dos cosas que hay que hacer:
//!
//! | caso | vecinos en contra | diferencia de color | qué sale |
//! | --- | --- | --- | --- |
//! | píxel de grano suelto | 8 | casi 0 | se funde con el vecindario |
//! | línea de 1 px de ancho | 6 contra 2 | ~0,9 | sobrevive |
//! | mota compacta de grano | 0 dentro, 5 en el borde | casi 0 | se come desde fuera |
//! | detalle pequeño de verdad | igual que la mota | grande | se queda entero |
//!
//! Una mota compacta no desaparece de golpe: su interior no tiene vecinos en
//! contra y no se mueve en la primera pasada. Se erosiona desde el borde, una
//! corona por pasada, y por eso el mando es el **número de pasadas** —cuánto
//! grosor de ruido se está dispuesto a raspar— y no un umbral.
//!
//! # Sobre etiquetas y no sobre colores
//!
//! Todo esto ocurre sobre el campo de entradas de la paleta, no sobre los píxeles
//! de la imagen. Suavizar la imagen antes de cuantizar sería más corto de
//! escribir y hace otra cosa: promedia a los dos lados de un borde de verdad, y
//! puede inventarse colores que no están en ninguna entrada. Aquí lo peor que
//! puede pasar es que un píxel acabe en una entrada que ya existía.

use crate::cluster::NONE;
use crate::color::Oklab;

/// Cuánto vale un vecino en desacuerdo, en unidades de distancia de Oklab.
///
/// Sale de acotar los dos casos que tienen que salir bien. Un píxel de ruido
/// suelto tiene los 8 vecinos en contra y está a menos de la tolerancia de las
/// dos entradas que se lo disputan, así que para que se funda hace falta
/// `8*beta > tolerancia`. Una línea de un píxel tiene 6 en contra y 2 a favor, y
/// para que sobreviva hace falta `4*beta < distancia al color del fondo`, que en
/// el caso que importa —tinta sobre papel— es del orden de 0,9.
///
/// Eso deja `beta` entre `tolerancia/8` y `0,22`, que es un hueco ancho. Se toma
/// la mitad de la tolerancia porque escala con ella: quien sube la tolerancia
/// está diciendo que acepta más error de color, y entonces también acepta que la
/// coherencia pese más. El suelo es para que con tolerancia 0 —paleta de todos
/// los colores distintos— esto siga haciendo algo en vez de nada.
pub fn beta(tolerance: f64) -> f64 {
    (tolerance / 2.0).max(0.005)
}

/// Cuánto error de color de más puede aceptar un píxel al cambiar de entrada, en
/// múltiplos de lo que la paleta ya había asumido para él. Ver la tabla del
/// módulo: por debajo de esto se pierde el arreglo, y por encima no se gana nada.
pub const CEILING: f64 = 2.0;

/// Vecinos que se miran: los 8 de alrededor, la misma vecindad con la que
/// [`crate::cluster`] une los tramos. Con 4 las diagonales de un trazo se
/// quedan sin apoyo y el criterio se vuelve anisótropo.
// Dispuestos como se leen, que es la mitad de lo que explican.
#[rustfmt::skip]
const NEIGHBOURS: [(isize, isize); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1,  0),          (1,  0),
    (-1,  1), (0,  1), (1,  1),
];

/// Regulariza el campo de entradas y devuelve cuántos píxeles se han movido.
///
/// - `field` lleva el índice de entrada de cada píxel, o [`NONE`] si no es
///   visible. Los no visibles no se tocan ni votan: el umbral de alfa es una
///   decisión tomada y difuminarla se comería la silueta.
/// - `lab_of` da el color del píxel original, y sólo se llama para los píxeles
///   que tienen algún vecino en desacuerdo —en una imagen normal, una minoría—.
/// - `entry_lab` es el color de cada entrada de la paleta, indexado como
///   `field`.
/// - `tolerance` es la de la paleta, sobre la que se calcula el techo de error
///   que un píxel puede aceptar al cambiar de entrada. Ver [`CEILING`].
#[allow(clippy::too_many_arguments)]
pub fn regularize(
    field: &mut [u32],
    width: usize,
    height: usize,
    lab_of: impl Fn(usize) -> Oklab,
    entry_lab: &[Oklab],
    beta: f64,
    tolerance: f64,
    passes: usize,
) -> usize {
    if passes == 0 || beta <= 0.0 || entry_lab.len() < 2 {
        return 0;
    }
    let mut moved = 0;
    // Los cambios se acumulan y se aplican al acabar la pasada. Es lo que hace
    // que el resultado no dependa de por dónde se empiece, y de paso cuesta
    // menos que duplicar el campo: sólo se guarda lo que se mueve.
    let mut changes: Vec<(usize, u32)> = Vec::new();

    for _ in 0..passes {
        changes.clear();
        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;
                let current = field[i];
                if current == NONE {
                    continue;
                }
                let Some(best) = choose(
                    field, width, height, x, y, current, &lab_of, entry_lab, beta, tolerance,
                ) else {
                    continue;
                };
                changes.push((i, best));
            }
        }
        if changes.is_empty() {
            break;
        }
        moved += changes.len();
        for &(i, entry) in &changes {
            field[i] = entry;
        }
    }
    moved
}

/// La entrada que le conviene a un píxel, o `None` si se queda como está.
///
/// Los candidatos son su entrada actual y las de sus vecinos: cambiar a una
/// entrada que no toca no baja nunca el término de coherencia, así que sólo
/// podría ganar por color —y por color ya ganaba la actual cuando se construyó
/// la paleta.
#[allow(clippy::too_many_arguments)]
fn choose(
    field: &[u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    current: u32,
    lab_of: &impl Fn(usize) -> Oklab,
    entry_lab: &[Oklab],
    beta: f64,
    tolerance: f64,
) -> Option<u32> {
    // (entrada, cuántos vecinos la tienen). Nunca pasa de ocho.
    let mut tally: [(u32, u32); 8] = [(0, 0); 8];
    let mut kinds = 0;
    let mut visible = 0;
    let mut agree = 0;

    for (dx, dy) in NEIGHBOURS {
        let (nx, ny) = (x as isize + dx, y as isize + dy);
        if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
            continue;
        }
        let entry = field[ny as usize * width + nx as usize];
        if entry == NONE {
            continue;
        }
        visible += 1;
        if entry == current {
            agree += 1;
            continue;
        }
        match tally[..kinds].iter_mut().find(|(e, _)| *e == entry) {
            Some(slot) => slot.1 += 1,
            None => {
                tally[kinds] = (entry, 1);
                kinds += 1;
            }
        }
    }

    // El caso normal con diferencia: todo el vecindario opina lo mismo. Se sale
    // antes de convertir ningún color, que es lo que mantiene el coste de esto
    // en el orden de una comparación por vecino.
    if kinds == 0 {
        return None;
    }

    let lab = lab_of(y * width + x);
    let error = |entry: u32| lab.distance(&entry_lab[entry as usize]);
    // El techo de error: lo que la paleta ya había asumido para este píxel. Ver
    // la nota del módulo sobre por qué esto no puede saltarse.
    // Lo que ya estaba dispuesto a aceptar, o el techo, lo que sea mayor. Y no
    // `max(tolerance, error) * CEILING`, que es lo mismo mientras la paleta
    // respete su tolerancia pero se dispara al componerlo con `SNAP_CEILING`: un
    // píxel arrastrado a 4x tendría permiso para irse a 8x. Así, componer no
    // afloja la cota.
    let ceiling = (tolerance * CEILING).max(error(current));

    let mut best = current;
    let mut best_cost = error(current) + beta * f64::from(visible - agree);
    for &(entry, favour) in &tally[..kinds] {
        // Sólo se admite lo que deja al píxel **de acuerdo con más vecinos** de
        // los que tiene ahora. Sin esta condición el término de color decide
        // solo en los empates de vecindad, y como la paleta asigna cada color a
        // la entrada más cercana *de las que ya existían* —no a la más cercana
        // de todas—, en cada frontera recta hay una fila de píxeles a los que
        // les conviene la entrada de enfrente por poco. Cambiarlos serraba las
        // fronteras rectas: un degradado limpio pasaba de 10 paths a 12, con los
        // bordes en dientes de sierra. Mejorar el color no es lo que hace esta
        // etapa; hacer que un píxel se parezca a su vecindario, sí.
        if favour <= agree {
            continue;
        }
        let d = error(entry);
        if d > ceiling {
            continue;
        }
        let c = d + beta * f64::from(visible - favour);
        // Estrictamente mejor, y a igualdad la de menor índice: sin el desempate
        // el resultado dependería del orden en que se hayan encontrado los
        // vecinos.
        if c < best_cost || (c == best_cost && entry < best) {
            best = entry;
            best_cost = c;
        }
    }

    (best != current).then_some(best)
}
