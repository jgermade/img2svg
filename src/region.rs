//! Representación intermedia entre segmentar y ajustar.
//!
//! La segmentación produce regiones; el ajuste convierte sus contornos en datos
//! de `<path>`. Entre medias va esto, y su forma decide si el ajuste puede o no
//! resolver las costuras.
//!
//! El contorno **no** se guarda como bucles independientes por región, sino como
//! tramos con una región a cada lado ([`HalfEdge`]). Con bucles independientes,
//! una frontera compartida por dos regiones se ajustaría dos veces, con
//! resultados distintos, y entre ellas asomaría un pelo de fondo; con tramos
//! compartidos se ajusta una vez y el problema no existe. Con `h`/`v` sobre
//! coordenadas enteras eso da igual, pero con Béziers no, y el tipo tiene que
//! estar antes de escribir los ajustadores.
//!
//! La segmentación por rejilla, hoy, deja `right` en `None`: sus regiones se
//! trazan cada una por su cuenta y no comparten geometría. Rellenarlo es trabajo
//! de la segmentación por clustering, que además lo necesita para saber en qué
//! vecina fundir una mota.

use crate::color::Rgba;
use crate::trace::Point;

pub type RegionId = usize;
pub type EdgeId = usize;

/// Un tramo de frontera, orientado de forma que `left` queda a su izquierda.
#[derive(Clone, Debug)]
pub struct HalfEdge {
    /// Polilínea densa, sin simplificar: el ajuste necesita todos los puntos
    /// para estimar tangentes, y el ajustador `pixel` ya se encarga de colapsar
    /// los tramos rectos.
    ///
    /// Incluye **los dos extremos**, y un tramo cerrado repite el primero al
    /// final. Esa repetición es la señal de que el tramo se cierra sobre sí
    /// mismo, que es justo lo que un ajustador de curvas necesita saber para
    /// tratarlo como periódico en vez de dejarle dos puntas sueltas.
    pub points: Vec<Point>,
    pub left: RegionId,
    /// La región del otro lado, o `None` si al otro lado está el exterior.
    pub right: Option<RegionId>,
}

/// Un anillo cerrado, como secuencia de tramos. El `bool` marca que el tramo se
/// recorre al revés, que es lo que pasa cuando se comparte con la región vecina.
pub type Ring = Vec<(EdgeId, bool)>;

#[derive(Clone, Debug)]
pub struct Region {
    pub color: Rgba,
    /// Píxeles que ocupa. El filtrado de motas se apoya en esto.
    pub area: usize,
    /// Anillos del contorno: el exterior y los agujeros, todos con el mismo
    /// trato porque se rellenan con `fill-rule="evenodd"`.
    pub rings: Vec<Ring>,
}

/// Lo que devuelve la segmentación.
///
/// Las regiones vienen **en orden de emisión**: las del mismo color seguidas, y
/// los colores más presentes primero, de modo que los paths grandes queden al
/// fondo del documento.
#[derive(Clone, Debug)]
pub struct Regions {
    /// Tamaño del lienzo en las unidades en que están los puntos.
    pub width: usize,
    pub height: usize,
    /// Colores distintos encontrados. No tiene por qué coincidir con los que
    /// acaben emitiéndose: un color cuyos bloques no den contorno no deja
    /// región, pero sí estaba en la imagen.
    pub colors: usize,
    pub regions: Vec<Region>,
    pub edges: Vec<HalfEdge>,
}

impl Regions {
    /// Encadena los puntos de un anillo tal como salieron de la segmentación.
    pub fn ring_points(&self, ring: &Ring) -> Vec<Point> {
        chain(ring, |edge| self.edges[edge].points.as_slice())
    }
}

/// Lo que hace falta saber de un elemento de cadena para poder ensamblarlo.
///
/// Existe para que [`chain`] valga tanto para los puntos que salen de la
/// segmentación como para los vértices con tangentes que produce el ajuste de
/// curvas, sin escribir dos veces la regla de qué se repite y dónde. Es un rato
/// sutil, y tenerla dos veces es exactamente como se estropea.
pub trait Chainable: Copy {
    /// El mismo elemento recorrido en sentido contrario. Para un punto es él
    /// mismo; para un vértice, sus dos controles cambian de lado.
    fn reversed(self) -> Self;

    /// Funde los dos elementos que coinciden en una junta: `self` es el que ya
    /// está puesto —el final del tramo anterior— y `next` el primero del
    /// siguiente, que ocupa el mismo sitio. De uno viene lo que llega a la
    /// junta y del otro lo que sale.
    fn join(self, next: Self) -> Self;

    /// Si los dos ocupan el mismo sitio, controles aparte.
    fn same_place(self, other: Self) -> bool;
}

impl Chainable for Point {
    fn reversed(self) -> Self {
        self
    }

    fn join(self, _next: Self) -> Self {
        self
    }

    fn same_place(self, other: Self) -> bool {
        self == other
    }
}

/// Encadena los tramos de un anillo. El primer elemento no se repite al final:
/// el cierre es implícito.
///
/// Los tramos llegan por parámetro en vez de leerse de `edges` porque el ajuste
/// ensambla los suyos, que son otros ([`crate::fit::Fitted`]). La regla de qué
/// se repite y dónde vive aquí, en un solo sitio, y no una vez por ajustador.
pub fn chain<'a, T: Chainable + 'a>(ring: &Ring, items: impl Fn(EdgeId) -> &'a [T]) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    for &(edge, reversed) in ring {
        let edge = items(edge);
        let mut it: Box<dyn Iterator<Item = T> + '_> = if reversed {
            Box::new(edge.iter().rev().map(|v| v.reversed()))
        } else {
            Box::new(edge.iter().copied())
        };
        // El primer elemento de un tramo es el último del anterior: son el
        // mismo sitio visto desde los dos lados, y se funden en uno.
        if let Some(prev) = out.last_mut() {
            if let Some(first) = it.next() {
                *prev = prev.join(first);
            }
        }
        out.extend(it);
    }
    // Y el final del anillo es otra vez su principio, porque el tramo que lo
    // cierra acaba donde empezó el primero.
    if out.len() > 1 && out[out.len() - 1].same_place(out[0]) {
        let last = out.pop().expect("acabamos de mirar que hay más de uno");
        out[0] = last.join(out[0]);
    }
    out
}
