use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use px2svg::{Config, Grouping};

/// Convierte imágenes pixel art en SVG, uniendo los píxeles del mismo color en
/// paths con el contorno mínimo.
#[derive(Parser)]
#[command(name = "px2svg", version, about, long_about = None)]
struct Args {
    /// Imagen de entrada (png, jpeg, gif, bmp, webp).
    input: PathBuf,

    /// Fichero SVG de salida (por defecto, la entrada con extensión .svg).
    #[arg(short, long)]
    output: Option<PathBuf>,

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

    /// Color de fondo del SVG, p. ej. "#ffffff".
    #[arg(short, long)]
    background: Option<String>,

    /// Un solo path por color, en vez de uno por bloque de píxeles contiguos.
    #[arg(short = 'm', long)]
    merge_colors: bool,

    /// No busca el damero de transparencia para quitarlo.
    #[arg(short, long)]
    keep_checkerboard: bool,

    /// Vacía el fondo liso y recorta el SVG a lo que queda dibujado.
    #[arg(short, long)]
    remove_background: bool,

    /// No imprime información del proceso.
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("px2svg: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<(), String> {
    let data = std::fs::read(&args.input)
        .map_err(|e| format!("no se pudo leer {}: {e}", args.input.display()))?;

    let config = Config {
        scale: args.scale,
        offset: args.offset.as_ref().map(|o| (o[0], o[1])),
        tolerance: args.tolerance,
        alpha_threshold: args.alpha_threshold,
        pixel_size: args.pixel_size,
        background: args.background.clone(),
        grouping: if args.merge_colors {
            Grouping::Color
        } else {
            Grouping::Region
        },
        remove_checkerboard: !args.keep_checkerboard,
        remove_background: args.remove_background,
    };
    let out = px2svg::convert(&data, &config).map_err(|e| e.to_string())?;

    let path = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension("svg"));
    std::fs::write(&path, &out.svg)
        .map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))?;

    if !args.quiet {
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
