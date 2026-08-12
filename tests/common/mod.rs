//! Instantáneas: se guarda la salida de una conversión y se compara byte a byte
//! con la de la próxima vez.
//!
//! Es la red de seguridad del refactor a dos ejes (segmentación x ajuste): el
//! camino de pixel art tiene que seguir dando exactamente lo de hoy. Detecta lo
//! que ningún test de unidad ve, como que un `simplify` perdido llene los paths
//! de vértices colineales sin cambiar el dibujo.
//!
//! Para regenerar las instantáneas tras un cambio intencionado:
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test
//! ```
//!
//! y **mirar el diff** antes de commitearlo.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use img2svg::{Conversion, Detail};

pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn updating() -> bool {
    std::env::var_os("UPDATE_GOLDEN").is_some()
}

/// Cabecera con los datos de la conversión, en un comentario XML.
///
/// Van dentro del propio fichero para que el diff diga *qué* ha cambiado (la
/// rejilla, el número de paths) y no sólo que ha cambiado. Como es un comentario
/// legal en el prólogo, la instantánea sigue siendo un SVG que se abre en el
/// navegador.
/// Lo propio de cada segmentación va **antes y después** del bloque común, en el
/// sitio exacto que ocupaba cuando sólo existía la rejilla. Reordenar una línea
/// movería las once instantáneas de pixel art sin que nada haya cambiado en la
/// conversión, que es justo lo que esta red de seguridad no debe hacer.
fn header(out: &Conversion, extra: &str) -> String {
    let (head, tail) = match out.detail {
        Detail::Grid {
            cell,
            offset,
            checkerboard,
        } => (
            format!(
                "  rejilla     {}x{}\n  celda       {:.3} x {:.3}\n  offset      {:.3}, {:.3}\n",
                out.canvas.0, out.canvas.1, cell.0, cell.1, offset.0, offset.1
            ),
            format!(
                "  damero      {}\n",
                checkerboard.map_or(String::from("no"), |c| format!(
                    "{:.2} x {:.2} px, {:.0}%",
                    c.cell.0,
                    c.cell.1,
                    c.coverage * 100.0
                ))
            ),
        ),
        #[cfg(feature = "illustration")]
        Detail::Cluster {
            regions,
            ramps,
            scale,
        } => (
            format!(
                "  lienzo      {}x{}\n{}  regiones    {}\n  degradados  {}\n",
                out.canvas.0,
                out.canvas.1,
                // La escala sólo aparece cuando ha habido reescalado, que es lo
                // que deja intactas las instantáneas que trabajan sobre la
                // retícula del original: una línea nueva en todas ellas movería
                // las once de pixel art sin que nada haya cambiado.
                if scale == 1.0 {
                    String::new()
                } else {
                    format!("  escala      x{scale:.3}\n")
                },
                regions,
                ramps
            ),
            String::new(),
        ),
    };
    format!(
        "<!--\n  {extra}\n{head}  colores     {}\n  paths       {}\n  \
subtrazados {}\n{tail}  fondo       {}\n-->\n",
        out.colors,
        out.paths,
        out.subpaths,
        out.background.map_or(String::from("no"), |c| c.to_hex()),
    )
}

/// Compara la conversión con la instantánea guardada, o la escribe si se ha
/// pedido regenerar. `extra` describe la entrada (fichero y huella, o el caso).
pub fn check(name: &str, out: &Conversion, extra: &str) {
    let actual = format!("{}{}", header(out, extra), out.svg);
    write_or_compare(name, &format!("{name}.svg"), &actual);
}

fn write_or_compare(name: &str, file: &str, actual: &str) {
    let path = golden_dir().join(file);

    if updating() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, actual).unwrap();
        eprintln!("instantánea escrita: {}", path.display());
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "no se pudo leer {}: {e}\n\
             genérala con: UPDATE_GOLDEN=1 cargo test",
            path.display()
        )
    });

    if actual != expected {
        // Se vuelca al lado para poder mirarlo con un diff de verdad: el de
        // `assert_eq!` sobre 30 KB de path data no se lee.
        let dump = path.with_extension("actual");
        std::fs::write(&dump, actual).ok();
        panic!(
            "«{name}» ha cambiado.\n  diff {} {}\n\
             Si el cambio es intencionado: UPDATE_GOLDEN=1 cargo test",
            path.display(),
            dump.display()
        );
    }
}

/// Como [`check`], pero guarda sólo la huella del SVG en vez del cuerpo.
///
/// Para los casos cuya salida se va a cientos de KB y que nadie va a abrir para
/// mirar el diff: detectan el cambio igual de bien, y la instantánea cabe en
/// unas líneas. El resto usa [`check`], que sí deja el SVG mirable.
pub fn check_digest(name: &str, out: &Conversion, extra: &str) {
    let actual = format!(
        "{}<svg-fnv>{}</svg-fnv>  <!-- {} bytes -->\n",
        header(out, extra),
        fingerprint(out.svg.as_bytes()),
        out.svg.len()
    );
    write_or_compare(name, &format!("{name}.txt"), &actual);
}

/// Huella FNV-1a de 64 bits. Sólo sirve para avisar de que algo ha cambiado, así
/// que no hace falta nada criptográfico ni una dependencia nueva.
pub fn fingerprint(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016x}")
}
