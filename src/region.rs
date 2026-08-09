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
    /// Encadena los puntos de un anillo. El primero no se repite al final: el
    /// cierre es implícito.
    pub fn ring_points(&self, ring: &Ring) -> Vec<Point> {
        let mut out: Vec<Point> = Vec::new();
        for &(edge, reversed) in ring {
            let points = &self.edges[edge].points;
            let mut it: Box<dyn Iterator<Item = &Point>> = if reversed {
                Box::new(points.iter().rev())
            } else {
                Box::new(points.iter())
            };
            // El primer punto de un tramo es el último del anterior.
            if !out.is_empty() {
                it.next();
            }
            out.extend(it.copied());
        }
        // Y el último punto del anillo es otra vez el primero, porque el tramo
        // que lo cierra acaba donde empezó el primero. Se descarta aquí, en un
        // solo sitio, en vez de que cada ajustador tenga que acordarse.
        if out.len() > 1 && out.last() == out.first() {
            out.pop();
        }
        out
    }
}
