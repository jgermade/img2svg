//! Línea de órdenes.
//!
//! El subcomando elige la **segmentación** —cómo se pasa de la imagen a un
//! conjunto de regiones— y los ajustes que no dependen de ella van en un bloque
//! compartido. `pixelart` detecta la rejilla del dibujo; `photo` agrupa los
//! colores en una paleta y etiqueta las regiones conexas.
//!
//! Sus opciones no se parecen porque no hablan de lo mismo: una tolerancia de
//! `12` en pixel art es distancia RGB entre dos tonos de una paleta discreta, y
//! una de `0.045` en foto es distancia en Oklab dentro de un degradado continuo.
//! Mezclarlas en un solo comando con banderas que a veces sirven y a veces no
//! sería más corto de escribir y peor de usar.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use img2svg::{ClusterOptions, Config, Conversion, Fit, GridOptions, Grouping};

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
    /// Foto o dibujo sin rejilla: agrupa los colores y traza cada región.
    Photo(Photo),
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

    /// Cómo se convierte el contorno de una región en datos de path.
    #[arg(long, value_enum, default_value_t = FitArg::Pixel)]
    fit: FitArg,

    /// Desviación máxima en píxeles al simplificar el contorno.
    ///
    /// Sólo la lee `--fit polygon`. Por debajo de 0.707 no se endereza ni una
    /// diagonal; subirla comprime más y va redondeando las esquinas pequeñas.
    #[arg(long, default_value_t = Fit::POLYGON_TOLERANCE)]
    fit_tolerance: f64,

    /// No imprime información del proceso.
    #[arg(short, long)]
    quiet: bool,
}

/// El eje de ajuste, tal como se nombra en la línea de órdenes.
///
/// Es un enum aparte y no el [`Fit`] de la biblioteca porque ese lleva dentro
/// los parámetros de cada ajustador, y una bandera de línea de órdenes es sólo
/// el nombre: la tolerancia llega por su cuenta.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
enum FitArg {
    /// La escalera literal del contorno.
    Pixel,
    /// Segmentos rectos, quitando los vértices que no dibujan nada.
    Polygon,
}

impl Common {
    fn fit(&self) -> Fit {
        match self.fit {
            FitArg::Pixel => Fit::Pixel,
            FitArg::Polygon => Fit::Polygon {
                tolerance: self.fit_tolerance.max(0.0),
            },
        }
    }
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
        let grid = GridOptions {
            scale: self.scale,
            offset: self.offset.as_ref().map(|o| (o[0], o[1])),
            tolerance: self.tolerance,
            alpha_threshold: self.alpha_threshold,
            pixel_size: self.pixel_size,
            grouping: if self.merge_colors {
                Grouping::Color
            } else {
                Grouping::Region
            },
            remove_checkerboard: !self.keep_checkerboard,
            remove_background: self.remove_background,
        };
        Config {
            background: self.common.background.clone(),
            fit: self.common.fit(),
            ..Config::grid(grid)
        }
    }
}

#[derive(Args)]
struct Photo {
    #[command(flatten)]
    common: Common,

    /// Distancia máxima en Oklab entre un color y el de la región que lo pinta.
    ///
    /// La escala es perceptual y va de 0 a 1: de negro a blanco es 1.0. Subirla
    /// deja menos colores y regiones más grandes.
    #[arg(short, long, default_value_t = ClusterOptions::default().tolerance)]
    tolerance: f64,

    /// Bits por canal a los que se recorta el color antes de agrupar.
    ///
    /// Baja el ruido del último bit, que en una foto son miles de colores
    /// distintos que no se ven.
    #[arg(short, long, default_value_t = ClusterOptions::default().color_precision)]
    color_precision: u8,

    /// Alfa mínimo para considerar un píxel visible.
    #[arg(short = 'a', long, default_value_t = ClusterOptions::default().alpha_threshold)]
    alpha_threshold: u8,

    /// Área en píxeles hasta la que una región se funde con su vecina.
    #[arg(long, default_value_t = ClusterOptions::default().filter_speckle)]
    filter_speckle: usize,

    /// Grosor por debajo del cual una región se funde con su vecina.
    ///
    /// No es un filtro de tamaño: existe por las bandas de un píxel de ancho que
    /// aparecen a lo largo de cada frontera de color, que son largas —y por
    /// tanto sobreviven a --filter-speckle— pero no dibujan nada. El grosor es
    /// 2*área/perímetro, que ronda 0.5 en una banda por larga que sea y crece
    /// con el lado en un bloque compacto.
    ///
    /// El valor por defecto, 1, es el grosor justo de un bloque de 2x2, así que
    /// **se lleva por delante todo lo que mida un píxel de ancho**, incluida una
    /// línea fina que sí fuese dibujo. Es el precio de quitar los rebordes de
    /// antialias, y se paga porque en una foto hay muchísimos más rebordes que
    /// líneas de un píxel. En un dibujo de línea fina, ponerlo a 0.
    #[arg(long, default_value_t = ClusterOptions::default().min_thickness)]
    min_thickness: f64,

    /// Ensancha las bandas de un degradado fundiendo por diferencia de luz.
    ///
    /// Es la herramienta para un cielo liso: funde tonos que sólo se distinguen
    /// en luminosidad, dejando el tono donde está. En un dibujo con volumen hace
    /// lo contrario de lo que se quiere, porque aplana el sombreado; y pasado
    /// ~0.15 las fronteras entre bandas salen moteadas. Por eso viene apagado.
    #[arg(long, default_value_t = ClusterOptions::default().gradient_step)]
    gradient_step: f64,

    /// Entradas máximas de la paleta (0 no pone tope).
    ///
    /// Con tope, los colores que sobran van a la entrada más cercana aunque
    /// quede lejos: deja de valer la garantía de --tolerance. Y menos colores no
    /// es menos regiones, suele ser más.
    #[arg(long, default_value_t = ClusterOptions::default().max_colors)]
    max_colors: usize,

    /// Vacía el fondo liso y recorta el SVG a lo que queda dibujado.
    ///
    /// El fondo es lo que toca el borde de la imagen, así que una zona encerrada
    /// del mismo color se queda.
    #[arg(short, long)]
    remove_background: bool,
}

impl Photo {
    fn config(&self) -> Config {
        let cluster = ClusterOptions {
            color_precision: self.color_precision,
            tolerance: self.tolerance,
            alpha_threshold: self.alpha_threshold,
            filter_speckle: self.filter_speckle,
            min_thickness: self.min_thickness,
            gradient_step: self.gradient_step,
            max_colors: self.max_colors,
            // Una paleta impuesta es una lista de colores, y parsearla pide una
            // sintaxis que nadie ha pedido todavía. Está en la biblioteca.
            palette: Vec::new(),
            remove_background: self.remove_background,
        };
        Config {
            background: self.common.background.clone(),
            fit: self.common.fit(),
            ..Config::cluster(cluster)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Pixelart(args) => run_pixelart(args),
        Command::Photo(args) => run_photo(args),
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
    let (out, path) = convert(common, &args.config())?;
    if common.quiet {
        return Ok(());
    }

    if let Some(found) = out.checkerboard() {
        eprintln!(
            "damero de transparencia {} / {}, casilla {:.1}x{:.1} px: {:.0}% a transparente",
            found.colors.0.to_hex(),
            found.colors.1.to_hex(),
            found.cell.0,
            found.cell.1,
            found.coverage * 100.0
        );
    }
    report_background(&out);
    if let (Some(cell), Some(offset)) = (out.cell(), out.offset()) {
        eprintln!(
            "rejilla {}x{} (celda {:.2}x{:.2}, offset {:.2},{:.2})",
            out.canvas.0, out.canvas.1, cell.0, cell.1, offset.0, offset.1
        );
    }
    report_output(&out, &path);
    Ok(())
}

fn run_photo(args: &Photo) -> Result<(), String> {
    let common = &args.common;
    let (out, path) = convert(common, &args.config())?;
    if common.quiet {
        return Ok(());
    }

    report_background(&out);
    // El número de regiones es lo que se mueve al tocar el filtrado de motas, y
    // no se deduce de los paths: un color con varias regiones va en un `<g>`.
    if let img2svg::Detail::Cluster { regions } = out.detail {
        eprintln!(
            "lienzo {}x{}, {} regiones",
            out.canvas.0, out.canvas.1, regions
        );
    }
    report_output(&out, &path);
    Ok(())
}

/// Lee la entrada, convierte y escribe la salida. Es lo idéntico entre los dos
/// subcomandos; lo que cambia es qué se cuenta después.
fn convert(common: &Common, config: &Config) -> Result<(Conversion, PathBuf), String> {
    let data = std::fs::read(&common.input)
        .map_err(|e| format!("no se pudo leer {}: {e}", common.input.display()))?;

    let out = img2svg::convert(&data, config).map_err(|e| e.to_string())?;

    let path = common
        .output
        .clone()
        .unwrap_or_else(|| common.input.with_extension("svg"));
    std::fs::write(&path, &out.svg)
        .map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))?;
    Ok((out, path))
}

fn report_background(out: &Conversion) {
    if let Some(color) = out.background {
        eprintln!("fondo {} retirado y lienzo recortado", color.to_hex());
    }
}

fn report_output(out: &Conversion, path: &std::path::Path) {
    eprintln!(
        "{} colores, {} paths, {} subtrazados -> {} ({:.1} KB)",
        out.colors,
        out.paths,
        out.subpaths,
        path.display(),
        out.svg.len() as f64 / 1024.0
    );
}
