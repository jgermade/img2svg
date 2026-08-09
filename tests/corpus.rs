//! Instantáneas sobre las imágenes reales de `examples/`.
//!
//! Son pixel art generado, con ruido de antialiasing por píxel (más de 60.000
//! colores distintos en el fichero, frente a las decenas que quedan tras fundir
//! la paleta). Eso es justo lo que las hace buenos fixtures: ejercitan el
//! histograma por cubos de `cell_color`, la reducción de paleta y la tolerancia
//! del damero, cosas que un dibujo sintético limpio no toca.
//!
//! `examples/` está en `.gitignore` y los PNG pesan 9 MB, así que **la entrada
//! no está versionada y la salida sí**: si las imágenes no están, estos tests se
//! saltan con un aviso en vez de fallar —o fallan, si se pone `REQUIRE_CORPUS`—.
//! La huella FNV de cada fichero va en la cabecera de la instantánea para que un
//! cambio de entrada se distinga de una regresión del código.

#![cfg(feature = "formats")]

mod common;

use std::path::{Path, PathBuf};

use common::{check, check_digest, fingerprint};
use img2svg::{Config, Conversion, GridOptions, Grouping};

/// Las tres imágenes, con nombre corto estable: el de Gemini no dice nada y
/// cambiaría la instantánea entera si alguien renombra el fichero.
const DAMERO: &str = "Gemini_Generated_Image_crfa7acrfa7acrfa.png";
const DENSO: &str = "Gemini_Generated_Image_g7uuvtg7uuvtg7uu.png";
const SIMPLE: &str = "Gemini_Generated_Image_s2xijjs2xijjs2xi.png";

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

/// Carga una imagen del corpus, o `None` si no está disponible.
///
/// Saltarse un test lo deja en verde, así que sin las imágenes estos casos
/// pasarían sin haber comprobado nada y el log de CI lo contaría como éxito. Con
/// `REQUIRE_CORPUS` puesto, faltar es un fallo: es lo que hay que activar el día
/// que las imágenes sean alcanzables desde un clon limpio.
fn load(file: &str) -> Option<Vec<u8>> {
    let path = examples_dir().join(file);
    match std::fs::read(&path) {
        Ok(data) => Some(data),
        Err(_) if std::env::var_os("REQUIRE_CORPUS").is_some() => panic!(
            "falta {}, y REQUIRE_CORPUS exige el corpus entero",
            path.display()
        ),
        Err(_) => {
            eprintln!(
                "SALTADO: falta {}.\n  \
                 El corpus no está versionado (examples/ está en .gitignore).",
                path.display()
            );
            None
        }
    }
}

/// Convierte y compara con la instantánea. Devuelve la conversión para poder
/// afirmar sobre ella; `None` si la imagen no estaba.
fn snapshot(name: &str, file: &str, config: &Config) -> Option<Conversion> {
    let (out, extra) = run(file, config)?;
    check(name, &out, &extra);
    Some(out)
}

/// Igual, pero guardando sólo la huella del SVG.
fn snapshot_digest(name: &str, file: &str, config: &Config) -> Option<Conversion> {
    let (out, extra) = run(file, config)?;
    check_digest(name, &out, &extra);
    Some(out)
}

fn run(file: &str, config: &Config) -> Option<(Conversion, String)> {
    let data = load(file)?;
    let out = img2svg::convert(&data, config).expect("la conversión no debe fallar");
    Some((out, format!("{file}  fnv {}", fingerprint(&data))))
}

#[test]
fn damero_con_rejilla_no_entera() {
    let Some(out) = snapshot("corpus-damero", DAMERO, &Config::default()) else {
        return;
    };
    let checker = out
        .checkerboard
        .expect("esta imagen trae damero de transparencia: debe encontrarlo");
    assert!(
        checker.coverage > 0.10,
        "el damero debería cubrir una parte apreciable, no {:.1}%",
        checker.coverage * 100.0
    );
    assert_eq!(out.grid, (80, 126));
    assert!(
        (out.cell.0 - 20.45).abs() < 0.05,
        "celda detectada {:.3}",
        out.cell.0
    );
    assert!(
        out.offset.0 > 1.0,
        "la rejilla no arranca en el borde: offset {:.3}",
        out.offset.0
    );
}

#[test]
fn denso_muchos_colores_y_paths() {
    let Some(out) = snapshot("corpus-denso", DENSO, &Config::default()) else {
        return;
    };
    assert_eq!(out.grid, (236, 133));
    assert!(out.colors > 50, "{} colores", out.colors);
    assert!(out.paths > 1000, "{} paths", out.paths);
    assert!(
        out.subpaths >= out.paths,
        "cada path tiene al menos un subtrazado"
    );
}

#[test]
fn simple_ejes_con_celda_distinta() {
    let Some(out) = snapshot("corpus-simple", SIMPLE, &Config::default()) else {
        return;
    };
    assert_eq!(out.grid, (100, 108));
    assert_ne!(
        out.cell.0, out.cell.1,
        "los dos ejes se detectan por separado y aquí no coinciden"
    );
}

/// Sin fundir colores parecidos el ruido del generador sobrevive entero: es la
/// prueba de que `reduce_palette` está haciendo el trabajo que dice.
///
/// Son 150 KB de SVG con miles de colores, así que va por huella: nadie va a
/// abrir ese diff.
#[test]
fn simple_sin_tolerancia() {
    let config = Config::grid(GridOptions {
        tolerance: 0.0,
        ..GridOptions::default()
    });
    let Some(out) = snapshot_digest("corpus-simple-sin-tolerancia", SIMPLE, &config) else {
        return;
    };
    let base = snapshot("corpus-simple", SIMPLE, &Config::default()).unwrap();
    assert!(
        out.colors > base.colors,
        "sin tolerancia deben quedar más colores que con ella ({} vs {})",
        out.colors,
        base.colors
    );
}

#[test]
fn simple_un_path_por_color() {
    let config = Config::grid(GridOptions {
        grouping: Grouping::Color,
        ..GridOptions::default()
    });
    let Some(out) = snapshot("corpus-simple-un-path-por-color", SIMPLE, &config) else {
        return;
    };
    assert_eq!(
        out.paths, out.colors,
        "un path por color, cada bloque como subtrazado"
    );
    assert!(out.subpaths > out.paths, "y muchos subtrazados dentro");
}

/// Con el damero desactivado la cuadrícula de transparencia se queda como
/// píxeles opacos, y hasta puede arrastrar la detección de rejilla. Fija ese
/// comportamiento para que el refactor no lo cambie por accidente.
#[test]
fn damero_conservado() {
    let config = Config::grid(GridOptions {
        remove_checkerboard: false,
        ..GridOptions::default()
    });
    let Some(out) = snapshot("corpus-damero-conservado", DAMERO, &config) else {
        return;
    };
    assert!(
        out.checkerboard.is_none(),
        "no se ha pedido buscarlo, no debe informar de él"
    );
    let quitado = snapshot("corpus-damero", DAMERO, &Config::default()).unwrap();
    assert!(
        out.subpaths > quitado.subpaths,
        "la cuadrícula opaca parte las regiones en más trozos ({} vs {})",
        out.subpaths,
        quitado.subpaths
    );
    // Contraintuitivo pero correcto: quitar el damero deja *más* colores, no
    // menos. Sus dos grises son de los más frecuentes, así que en
    // `reduce_palette` hacen de ancla y absorben los tonos claros del dibujo que
    // caen dentro de la tolerancia; sin ellos, esos tonos sobreviven aparte.
    assert!(
        out.colors < quitado.colors,
        "{} colores con damero, {} sin él",
        out.colors,
        quitado.colors
    );
}
