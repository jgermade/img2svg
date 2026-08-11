//! Dónde está el borde de verdad, dentro del píxel que lo contiene.
//!
//! El contorno de una región sale de [`crate::boundary`] recorriendo grietas
//! entre píxeles, así que **todos sus vértices caen en la retícula entera** de la
//! imagen. En una foto grande da igual; en un dibujo pequeño es lo que decide el
//! resultado. La montura de unas gafas de 300x300 mide dos píxeles de ancho, y un
//! círculo obligado a poner sus vértices en esa retícula es un octógono.
//!
//! # Que no es un problema del ajustador
//!
//! Es tentador culpar a la tolerancia del polígono, pero medirla enseña que no
//! hay dónde ponerla. Sobre una retícula la simplificación tiene dos escalones y
//! nada entre medias: a `0,5` se aplana un peldaño suelto, y a `raíz(2)/2 = 0,707`
//! una escalera de 45 grados colapsa en su diagonal. Y **el borde de un círculo
//! pequeño es una sucesión de escaleras cortas de 45 grados**, así que el mismo
//! colapso que endereza un canto largo —que es lo que uno quiere— convierte la
//! lente en un octógono. Sobre la retícula las dos cosas son localmente la misma,
//! y ninguna tolerancia las separa.
//!
//! Lo que hay que quitar no es el colapso: es la retícula.
//!
//! # De dónde sale la posición
//!
//! De la propia imagen, que ya la lleva escrita y la estábamos tirando. Donde un
//! píxel cae encima del borde entre dos regiones, su color no es el de ninguna de
//! las dos sino la mezcla que le toca por cuánto lo tapa cada una — eso es lo que
//! es el antialias, y también lo que deja un JPEG al difuminar el salto. De la
//! proporción de la mezcla sale por dónde corta el borde.
//!
//! Para una grieta vertical entre el píxel `p` (de la región que se pinta `A`) y
//! el `q` (de la que se pinta `B`), si `a(x)` es la parte de `A` que hay en el
//! color del píxel `x`, el borde está en
//!
//! ```text
//!     x = grieta + a(p) + a(q) - 1
//! ```
//!
//! Se lee solo en los extremos: con los dos píxeles puros —`a(p) = 1`, `a(q) = 0`—
//! el borde cae justo en la grieta; si `q` está medio tapado por `A`, se corre
//! medio píxel hacia `q`. Y usa **los dos** lados en vez de fiarse de uno, que es
//! lo que hace que un píxel raro a un lado no se lleve el borde él solo.
//!
//! # Se mueven vértices, no grietas
//!
//! Cada grieta por su cuenta se despegaría de sus vecinas y el anillo dejaría de
//! cerrar. Así que lo que se desplaza es **el vértice**: las grietas verticales
//! que lo tocan dicen cuánto se mueve en `x`, las horizontales cuánto en `y`, y de
//! cada par se toma la media. En un tramo recto las dos vecinas son de la misma
//! orientación y el vértice se mueve perpendicular; en una esquina se mueve en
//! diagonal, que es lo que hay que hacer con una esquina.
//!
//! Y como el desplazamiento sale de **una función del vértice** —su posición, las
//! etiquetas de alrededor y la imagen— y de nada del recorrido, dos tramos que
//! comparten un extremo reciben el mismo número sin tener que ponerse de acuerdo.
//! Es la misma propiedad que hace que [`crate::region::HalfEdge`] exista: la
//! frontera compartida se ajusta una vez y las dos caras reciben lo mismo.

use image::RgbaImage;

use crate::cluster::{Clustering, NONE};
use crate::color::Rgba;
use crate::region::Regions;

/// Cuánto puede apartarse un vértice de su sitio en la retícula, en píxeles.
///
/// Medio píxel es exactamente hasta donde el borde puede estar sin salirse de los
/// dos píxeles que lo delatan: más allá, quien lo vería sería otra grieta. Y de
/// paso es lo que impide que dos vértices vecinos se crucen y el contorno se
/// anude.
const MAX_SHIFT: f32 = 0.5;

/// Coloca cada vértice del contorno donde la imagen dice que está el borde.
///
/// No cambia ni una región ni un anillo: sólo mueve puntos, así que todo lo que
/// se decidió sobre las etiquetas sigue valiendo. Si no hay nada que mover —una
/// imagen sin antialias— los desplazamientos salen todos a cero y el resultado es
/// el de antes.
pub fn place(regions: &mut Regions, clustering: &Clustering, img: &RgbaImage) {
    let field = Field {
        w: clustering.width,
        h: clustering.height,
        labels: &clustering.labels,
        colors: &clustering
            .clusters
            .iter()
            .map(|c| c.color)
            .collect::<Vec<_>>(),
        img,
    };
    for edge in &mut regions.edges {
        edge.offsets = edge.points.iter().map(|&p| field.shift(p)).collect();
    }
}

/// A cuántos escalones por píxel se redondea el desplazamiento.
///
/// No es cosmético: es lo que decide el tamaño del fichero. Sobre la retícula, un
/// tramo horizontal se escribe `h` con **un** número, porque sus dos extremos
/// tienen la misma `y`. Desplazando en crudo, cada vértice sale con su decimal y
/// esa igualdad se rompe: el mismo tramo pasa a `l` con dos números y decimales,
/// que son tres veces los bytes. Redondeando a un escalón grueso, una fila de
/// vértices que miden casi lo mismo vuelve a medir **exactamente** lo mismo, y el
/// `h` sobrevive.
///
/// Y de paso denoisa, que es lo que hacía falta: el desplazamiento sale de
/// deshacer una mezcla de colores sobre un JPEG, así que su centésima no
/// significa nada. Un cuarto de píxel sí.
///
/// Medido sobre la portada, con `polygon` a 0,5:
///
/// | escalón | bytes | anclas | |
/// | --- | --- | --- | --- |
/// | en crudo | 89,6 KB | 10.856 | cada tramo recto pasa a `l` |
/// | 1/8 px | 82,6 KB | | |
/// | 1/3 px | 79,6 KB | | peor que 1/4: `0,333` no cabe en dos decimales |
/// | **1/4 px** | **75,1 KB** | **10.723** | |
/// | 1/2 px | 59,1 KB | 10.348 | se ven los peldaños en el arco |
/// | sin subpíxel | 43,0 KB | 11.419 | y la lente sale octogonal |
///
/// Se toma `1/4`. `1/2` sale más barato en las dos columnas, pero a 16 aumentos
/// el arco de la lente enseña sus escalones, y ese arco es justo lo que se ha
/// venido a arreglar. Los escalones que no son potencia de dos salen peor de lo
/// que prometen porque no caen exactos en los dos decimales con los que se
/// escriben las coordenadas.
const STEPS_PER_PIXEL: f32 = 4.0;

/// Redondea un desplazamiento al escalón. Ver [`STEPS_PER_PIXEL`].
fn snap(v: f32) -> f32 {
    (v * STEPS_PER_PIXEL).round() / STEPS_PER_PIXEL
}

struct Field<'a> {
    w: usize,
    h: usize,
    labels: &'a [u32],
    /// El color con el que se pinta cada región, indexado por etiqueta.
    colors: &'a [Rgba],
    img: &'a RgbaImage,
}

impl Field<'_> {
    /// La etiqueta de un píxel, o [`NONE`] si cae fuera de la imagen. Fuera y
    /// transparente son lo mismo aquí: ni uno ni otro tienen color que mezclar.
    fn label(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return NONE;
        }
        self.labels[y as usize * self.w + x as usize]
    }

    /// El desplazamiento del vértice de retícula `(x, y)`.
    fn shift(&self, (x, y): crate::trace::Point) -> (f32, f32) {
        // Las dos grietas verticales que salen del vértice, hacia arriba y hacia
        // abajo, mueven en x; las dos horizontales, en y.
        let dx = mean(self.vertical(x, y - 1), self.vertical(x, y));
        let dy = mean(self.horizontal(x - 1, y), self.horizontal(x, y));
        (snap(dx), snap(dy))
    }

    /// Grieta vertical en `x` a la altura de la fila `y`: separa el píxel de la
    /// izquierda del de la derecha. Devuelve cuánto se corre el borde en `x`.
    fn vertical(&self, x: i32, y: i32) -> Option<f32> {
        self.offset((x - 1, y), (x, y))
    }

    /// Grieta horizontal en `y` a lo ancho de la columna `x`: separa el píxel de
    /// arriba del de abajo. Devuelve cuánto se corre el borde en `y`.
    fn horizontal(&self, x: i32, y: i32) -> Option<f32> {
        self.offset((x, y - 1), (x, y))
    }

    /// El corrimiento del borde entre dos píxeles vecinos, o `None` si esa grieta
    /// no es frontera o no hay con qué medirla.
    fn offset(&self, p: (i32, i32), q: (i32, i32)) -> Option<f32> {
        let (la, lb) = (self.label(p.0, p.1), self.label(q.0, q.1));
        if la == lb {
            return None;
        }
        // Con un lado fuera o transparente no hay mezcla que leer: el borde de la
        // silueta lo puso el umbral de alfa y no se toca.
        if la == NONE || lb == NONE {
            return None;
        }
        let (ca, cb) = (self.colors[la as usize], self.colors[lb as usize]);
        // Dos regiones distintas del mismo color existen —la frontera entre ellas
        // no se ve—, y ahí no hay nada que localizar.
        let axis = Vec3::of(ca).minus(Vec3::of(cb));
        let len2 = axis.dot(axis);
        if len2 < 1.0 {
            return None;
        }
        let share = |at: (i32, i32)| {
            let px = self.img.get_pixel(at.0 as u32, at.1 as u32).0;
            let c = Vec3::new(px[0], px[1], px[2]).minus(Vec3::of(cb));
            (c.dot(axis) / len2).clamp(0.0, 1.0)
        };
        Some((share(p) + share(q) - 1.0).clamp(-MAX_SHIFT, MAX_SHIFT))
    }
}

/// La media de los desplazamientos que haya, o cero si no hay ninguno.
fn mean(a: Option<f32>, b: Option<f32>) -> f32 {
    match (a, b) {
        (Some(a), Some(b)) => (a + b) / 2.0,
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => 0.0,
    }
}

/// Un color como vector, para proyectar uno sobre la recta que une otros dos.
///
/// En sRGB tal cual viene, sin pasar por Oklab: lo que se está deshaciendo aquí
/// es una **mezcla**, y la mezcla la hizo quien generó la imagen sobre estos
/// mismos números. Convertir a un espacio perceptual mediría muy bien la
/// diferencia entre los dos colores y muy mal la proporción entre ellos, que es
/// lo único que se pregunta.
#[derive(Clone, Copy)]
struct Vec3(f32, f32, f32);

impl Vec3 {
    fn new(r: u8, g: u8, b: u8) -> Self {
        Vec3(f32::from(r), f32::from(g), f32::from(b))
    }

    fn of(c: Rgba) -> Self {
        Vec3::new(c.r, c.g, c.b)
    }

    fn minus(self, o: Vec3) -> Self {
        Vec3(self.0 - o.0, self.1 - o.1, self.2 - o.2)
    }

    fn dot(self, o: Vec3) -> f32 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }
}
