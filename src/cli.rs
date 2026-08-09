//! Línea de órdenes.
//!
//! El subcomando elige la **segmentación** —cómo se pasa de la imagen a un
//! conjunto de regiones— y los ajustes que no dependen de ella van en un bloque
//! compartido. De momento sólo existe `pixelart`; `photo`, con segmentación por
//! clustering, entra cuando exista su motor.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use img2svg::{Config, Grouping};

/// Convierte imágenes en SVG.
#[derive(Parser)]
#[command(name = "img2svg", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pixel art: detecta la rejilla y une los píxeles del mismo color en paths.
    Pixelart(Pixelart),
}

/// Ajustes que no dependen de la segmentación.
#[derive(Args)]
struct Common {
    /// Imagen de entrada (png, jpeg, gif, bmp, webp).
    input: PathBuf,

    /// Fichero SVG de salida (por defecto, la entrada con extensión .svg).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Color de fondo del SVG, p. ej. "#ffffff".
    #[arg(short, long)]
    background: Option<String>,

    /// No imprime información del proceso.
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Args)]
struct Pixelart {
    #[command(flatten)]
    common: Common,

    /// Tamaño en píxeles reales de cada píxel del dibujo. Por defecto se detecta.
    #[arg(short, long)]
    scale: Option<f64>,

    /// Desplazamiento de la rejilla respecto al borde izquierdo/superior.
    #[arg(long, value_names = ["X", "Y"], num_args = 2)]
    offset: Option<Vec<f64>>,

    /// Tolerancia al fusionar colores parecidos (0 los conserva todos).
    #[arg(short, long, default_value_t = 12.0)]
    tolerance: f64,

    /// Alfa mínimo para considerar un píxel visible.
    #[arg(short = 'a', long, default_value_t = 128)]
    alpha_threshold: u8,

    /// Unidades SVG por píxel del dibujo (por defecto, la escala detectada).
    #[arg(short, long)]
    pixel_size: Option<u32>,

    /// Un solo path por color, en vez de uno por bloque de píxeles contiguos.
    #[arg(short = 'm', long)]
    merge_colors: bool,

    /// No busca el damero de transparencia para quitarlo.
    #[arg(short, long)]
    keep_checkerboard: bool,

    /// Vacía el fondo liso y recorta el SVG a lo que queda dibujado.
    #[arg(short, long)]
    remove_background: bool,
}

impl Pixelart {
    fn config(&self) -> Config {
        Config {
            scale: self.scale,
            offset: self.offset.as_ref().map(|o| (o[0], o[1])),
            tolerance: self.tolerance,
            alpha_threshold: self.alpha_threshold,
            pixel_size: self.pixel_size,
            background: self.common.background.clone(),
            grouping: if self.merge_colors {
                Grouping::Color
            } else {
                Grouping::Region
            },
            remove_checkerboard: !self.keep_checkerboard,
            remove_background: self.remove_background,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Pixelart(args) => run_pixelart(args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("img2svg: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_pixelart(args: &Pixelart) -> Result<(), String> {
    let common = &args.common;
    let data = std::fs::read(&common.input)
        .map_err(|e| format!("no se pudo leer {}: {e}", common.input.display()))?;

    let out = img2svg::convert(&data, &args.config()).map_err(|e| e.to_string())?;

    let path = common
        .output
        .clone()
        .unwrap_or_else(|| common.input.with_extension("svg"));
    std::fs::write(&path, &out.svg)
        .map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))?;

    if !common.quiet {
        if let Some(found) = out.checkerboard {
            eprintln!(
                "damero de transparencia {} / {}, casilla {:.1}x{:.1} px: {:.0}% a transparente",
                found.colors.0.to_hex(),
                found.colors.1.to_hex(),
                found.cell.0,
                found.cell.1,
                found.coverage * 100.0
            );
        }
        if let Some(color) = out.background {
            eprintln!("fondo {} retirado y lienzo recortado", color.to_hex());
        }
        eprintln!(
            "rejilla {}x{} (celda {:.2}x{:.2}, offset {:.2},{:.2})",
            out.grid.0, out.grid.1, out.cell.0, out.cell.1, out.offset.0, out.offset.1
        );
        eprintln!(
            "{} colores, {} paths, {} subtrazados -> {} ({:.1} KB)",
            out.colors,
            out.paths,
            out.subpaths,
            path.display(),
            out.svg.len() as f64 / 1024.0
        );
    }
    Ok(())
}
