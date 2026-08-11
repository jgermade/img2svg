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
    ///
    /// Por defecto, `pixel` en pixelart y `polygon` en photo, que es lo que
    /// quiere cada uno: en un sprite la escalera **es** el dibujo y redondearla
    /// sería estropearlo, mientras que en una foto no hay ninguna escalera que
    /// preservar —sólo la de la retícula de píxeles— y enderezarla quita entre un
    /// 23% y un 32% del fichero sin que se note en el dibujo.
    ///
    /// No puede ir en `default_value_t` porque no es el mismo para los dos
    /// subcomandos; lo resuelve [`Common::fit`].
    #[arg(long, value_enum)]
    fit: Option<FitArg>,

    /// Desviación máxima en píxeles al ajustar el contorno.
    ///
    /// La leen `--fit polygon` (0.75 por defecto) y `--fit spline` (1.5), y
    /// promete lo mismo en los dos: ningún punto del contorno acaba más lejos de
    /// lo que se dibuja. El valor de partida es distinto porque el suelo lo es:
    /// el polígono elige vértices de la retícula y a 0.75 ya endereza diagonales,
    /// mientras que una curva por debajo de 1.0 se dedica a perseguir los
    /// peldaños de la escalera. Subirla comprime más y redondea las esquinas
    /// pequeñas.
    #[arg(long)]
    fit_tolerance: Option<f64>,

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
    /// Béziers cúbicas, con las esquinas en pico y el resto liso.
    Spline,
}

/// La tolerancia con la que sale `photo` cuando endereza un contorno.
///
/// Es más estrecha que [`Fit::TOLERANCE`] y el motivo es geométrico, no de gusto.
/// Sobre una retícula de píxeles la tolerancia tiene dos escalones y nada entre
/// medias: a `0.5` se aplana un peldaño suelto, y a `raíz(2)/2 = 0.707` una
/// escalera de 45 grados colapsa en su diagonal. Contando anclas en la portada:
///
/// | tolerancia | anclas | |
/// | --- | --- | --- |
/// | 0,49 | 11.464 | |
/// | 0,50 | 11.419 | se aplanan los peldaños sueltos |
/// | 0,70 | 8.997 | |
/// | **0,71** | **7.704** | **−14,4%: colapsan las diagonales** |
/// | 0,75 | 7.503 | |
///
/// El problema es que **el borde de un círculo pequeño es una sucesión de
/// escaleras cortas de 45 grados**, así que ese mismo colapso que endereza un
/// canto largo convierte una lente de gafas en un octógono. Sobre la retícula las
/// dos cosas son localmente idénticas y ninguna tolerancia las distingue.
///
/// Así que por debajo del escalón, y la compresión que se deja ahí se recupera
/// cuando los contornos dejen de estar clavados a la retícula —ver
/// `SESSIONS/2026-08-11_12h45.off-the-pixel-lattice.md`—, que es el arreglo de
/// verdad. En una imagen grande `--fit-tolerance 0.75` sigue siendo buen negocio:
/// ahí un rasgo mide cientos de píxeles y no hay arco que perder.
const PHOTO_POLYGON_TOLERANCE: f64 = 0.5;

impl Common {
    /// El ajustador pedido, o el que trae de fábrica la segmentación que llama,
    /// con la tolerancia que le toca a ese ajustador en esa segmentación.
    fn fit(&self, default: FitArg, polygon_tolerance: f64) -> Fit {
        match self.fit.unwrap_or(default) {
            FitArg::Pixel => Fit::Pixel,
            FitArg::Polygon => Fit::Polygon {
                tolerance: self.tolerance(polygon_tolerance),
            },
            FitArg::Spline => Fit::Spline {
                tolerance: self.tolerance(Fit::SPLINE_TOLERANCE),
            },
        }
    }

    /// La tolerancia pedida, o la que le toca a este ajustador. No puede ir en
    /// `default_value_t` porque no es la misma para los dos.
    fn tolerance(&self, default: f64) -> f64 {
        self.fit_tolerance.unwrap_or(default).max(0.0)
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
            // La escalera de un sprite es el dibujo, no un artefacto. Y si aquí
            // se pide polígono, una unidad es un píxel *del dibujo*, no de la
            // imagen: los rasgos miden lo que tienen que medir y `Fit::TOLERANCE`
            // vale tal cual.
            fit: self.common.fit(FitArg::Pixel, Fit::TOLERANCE),
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

    /// No busca el borde dentro del píxel: deja los vértices en la retícula.
    ///
    /// El contorno sale de recorrer grietas entre píxeles, así que sus vértices
    /// caen en la retícula entera de la imagen. En un dibujo pequeño eso es lo
    /// que decide el resultado: una lente de gafas de dieciséis píxeles no puede
    /// ser redonda si sus vértices tienen que caer en esa retícula. El color de
    /// los píxeles del borde dice por dónde corta de verdad, y eso los recoloca.
    #[arg(long)]
    no_subpixel: bool,

    /// No fundir en un degradado los grupos de bandas que son una rampa.
    ///
    /// La paleta reparte una rampa continua en escalones, y las fronteras entre
    /// esas bandas no dibujan nada: sólo marcan por dónde cruzó la rampa un
    /// umbral de cuantización, siguiendo el ruido del original. Un grupo de
    /// bandas que un solo <linearGradient> sabe reproducir se funde en una figura
    /// con ese degradado, lo que baja a la vez colores, figuras y anclas.
    ///
    /// Se aceptan grupos cuyo degradado acierta el color de cada banda; un borde
    /// duro, donde el salto entre bandas es grande, no pasa el corte y se queda
    /// duro. Con esto puesto todo sale a bandas planas, como antes.
    #[arg(long)]
    no_ramps: bool,

    /// Pasadas de regularización de la paleta mirando el vecindario (0 la apaga).
    ///
    /// La paleta asigna cada píxel por su cuenta, así que en cuanto el ruido del
    /// original se acerca a --tolerance dos píxeles vecinos de una zona lisa caen
    /// en entradas distintas y la zona sale rota en motas. Esto lo deshace
    /// pesando el parecido de color contra el acuerdo con los vecinos, que es lo
    /// que distingue un píxel de grano —igual de cerca de las dos entradas— de un
    /// trazo fino, que está lejísimos de la del fondo y sobrevive.
    ///
    /// Cada pasada raspa una corona de las motas compactas. Subirlo redondea el
    /// detalle pequeño; a 0, el comportamiento de antes.
    #[arg(long, default_value_t = ClusterOptions::default().smoothing)]
    smoothing: usize,

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

    /// Lo que un color tiene que valer para llevarse una entrada propia, como
    /// fracción de la imagen (0 se la da a cualquiera).
    ///
    /// La agrupación va por frecuencia, pero la frecuencia sólo ordena y nunca
    /// frena: un color que sale treinta veces en toda la imagen funda entrada
    /// igual que el fondo, y por eso el ringing de un JPEG alrededor de un trazo
    /// negro deja una entrada por escalón. No se mide por recuento, que no
    /// distingue el ringing de un lunar del mismo tamaño, sino por el error que
    /// la entrada ahorra: píxeles por distancia a la entrada más cercana. Un
    /// color al doble de la tolerancia necesita la mitad de píxeles.
    #[arg(long, default_value_t = ClusterOptions::default().min_color_share)]
    min_color_share: f64,

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
            subpixel: !self.no_subpixel,
            ramps: !self.no_ramps,
            smoothing: self.smoothing,
            alpha_threshold: self.alpha_threshold,
            filter_speckle: self.filter_speckle,
            min_thickness: self.min_thickness,
            gradient_step: self.gradient_step,
            min_color_share: self.min_color_share,
            max_colors: self.max_colors,
            // Una paleta impuesta es una lista de colores, y parsearla pide una
            // sintaxis que nadie ha pedido todavía. Está en la biblioteca.
            palette: Vec::new(),
            remove_background: self.remove_background,
        };
        Config {
            background: self.common.background.clone(),
            // Aquí la escalera es sólo la retícula de píxeles y enderezarla no
            // cuesta dibujo, pero por debajo del escalón donde empieza a costarlo.
            // Ver `PHOTO_POLYGON_TOLERANCE`.
            fit: self.common.fit(FitArg::Polygon, PHOTO_POLYGON_TOLERANCE),
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
    if let img2svg::Detail::Cluster { regions, ramps } = out.detail {
        // Los degradados sólo se nombran cuando los hay: en un dibujo de colores
        // planos no hay ninguno y la línea no tiene por qué decirlo.
        let con_degradados = match ramps {
            0 => String::new(),
            1 => ", 1 degradado".to_string(),
            n => format!(", {n} degradados"),
        };
        eprintln!(
            "lienzo {}x{}, {} regiones{}",
            out.canvas.0, out.canvas.1, regions, con_degradados
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
