//! Línea de órdenes.
//!
//! El subcomando elige la **segmentación** —cómo se pasa de la imagen a un
//! conjunto de regiones— y los ajustes que no dependen de ella van en un bloque
//! compartido. `pixelart` detecta la rejilla del dibujo; `illustration` agrupa
//! los colores en una paleta y etiqueta las regiones conexas.
//!
//! Sus opciones no se parecen porque no hablan de lo mismo: una tolerancia de
//! `12` en pixel art es distancia RGB entre dos tonos de una paleta discreta, y
//! una de `0.045` en ilustración es distancia en Oklab dentro de un degradado
//! continuo.
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
    /// Ilustración: agrupa los colores de la imagen y traza cada región.
    ///
    /// El alias `photo` es como se llamaba antes; sigue valiendo.
    #[command(alias = "photo")]
    Illustration(Illustration),
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
    /// Por defecto, `pixel` en pixelart y `polygon` en illustration, que es lo
    /// que quiere cada uno: en un sprite la escalera **es** el dibujo y
    /// redondearla sería estropearlo, mientras que en una ilustración no hay
    /// ninguna escalera que preservar —sólo la de la retícula de píxeles— y
    /// enderezarla quita entre un 23% y un 32% del fichero sin que se note.
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
struct Illustration {
    #[command(flatten)]
    common: Common,

    /// El rasgo más pequeño que sobrevive, en tantos por mil del lado largo.
    ///
    /// Es el mando de simplificar, y decide a qué resolución se segmenta: todas
    /// las demás constantes van en píxeles absolutos —un área de motas, un
    /// grosor, una desviación de ajuste—, así que lo que significan depende de
    /// cuántos píxeles gasta la imagen en un rasgo. Subirlo simplifica; bajarlo
    /// conserva detalle y engorda el fichero.
    ///
    /// Por debajo de lo que la imagen mide, sube de escala, que es lo que
    /// recupera el borde escrito en el antialias; por encima, baja, que es lo que
    /// promedia el grano. Con --no-simplify se trabaja sobre la retícula del
    /// original tal cual.
    #[arg(long, default_value_t = img2svg::resample::SIMPLIFY)]
    simplify: f64,

    /// Segmenta sobre la retícula del original, sin elegir escala de trabajo.
    #[arg(long, conflicts_with = "simplify")]
    no_simplify: bool,

    /// Distancia máxima en Oklab entre un color y el de la región que lo pinta.
    ///
    /// La escala es perceptual y va de 0 a 1: de negro a blanco es 1.0. Subirla
    /// deja menos colores y regiones más grandes.
    #[arg(short, long, default_value_t = ClusterOptions::default().tolerance)]
    tolerance: f64,

    /// Bits por canal a los que se recorta el color antes de agrupar.
    ///
    /// Baja el ruido del último bit, que en una imagen son miles de colores
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

    /// Cuánto puede moverse un vértice del contorno para quitarle el temblor de
    /// la escalera, en píxeles de trabajo.
    ///
    /// El contorno sale de recorrer grietas entre píxeles, así que un canto
    /// oblicuo sale a peldaños, y los de un dibujo real son irregulares: el
    /// simplificador no puede tirarlos sin salirse de lo que promete, y los
    /// escribe. Esto los lima moviendo los vértices, con tope y sin tocar las
    /// esquinas, que se reconocen por el giro a lo largo del contorno.
    #[arg(long, default_value_t = ClusterOptions::default().relax)]
    relax: f64,

    /// Deja el contorno tal como sale del trazado, con su temblor de escalera.
    #[arg(long, conflicts_with = "relax")]
    no_relax: bool,

    /// Mide la blandura de cada frontera y la imprime, sin escribir SVG.
    ///
    /// La blandura es cuántos píxeles tarda una frontera en pasar del color de una
    /// cara al de la otra: un trazo de tinta cambia en uno, el terminador de una
    /// superficie redonda pintada a aerógrafo tarda cinco o diez. Es lo que decide
    /// qué costuras pueden salir como degradado aunque sólo junten dos colores, y
    /// a qué grupos se les prueba el degradado radial.
    ///
    /// Sirve para comprobar la medida contra el dibujo cuando un degradado sale
    /// donde no toca o no sale donde debería: las costuras de una barriga o un
    /// morro tienen que salir blandas, y los bordes de tinta duros.
    #[arg(long)]
    softness: bool,

    /// No fundir en un degradado los grupos de bandas que son una rampa.
    ///
    /// La paleta reparte una rampa continua en escalones, y las fronteras entre
    /// esas bandas no dibujan nada: sólo marcan por dónde cruzó la rampa un
    /// umbral de cuantización, siguiendo el ruido del original. Un grupo de
    /// bandas que un solo degradado sabe reproducir se funde en una figura con ese
    /// degradado, lo que baja a la vez colores, figuras y anclas.
    ///
    /// El degradado sale <linearGradient> o <radialGradient> según cuál de los dos
    /// explique mejor el grupo: un cielo es función de la proyección sobre un eje,
    /// y el sombreado de una superficie redonda, de la distancia a un centro.
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

    /// Grosor por debajo del cual una región puede fundirse con su vecina.
    ///
    /// No es un filtro de tamaño: existe por las bandas de un píxel de ancho que
    /// aparecen a lo largo de cada frontera de color, que son largas —y por
    /// tanto sobreviven a --filter-speckle— pero no dibujan nada. El grosor es
    /// 2*área/perímetro, que ronda 0.5 en una banda por larga que sea y crece
    /// con el lado en un bloque compacto.
    ///
    /// Ser delgada no basta para fundirse, porque un trazo de tinta también lo
    /// es: se funden las delgadas cuyo color es una **mezcla** de sus dos
    /// vecinas, que es lo que un reborde de antialias es y un trazo no. Así que
    /// subirlo no se lleva el dibujo por delante; lo que hace es admitir
    /// rebordes más gordos.
    #[arg(long, default_value_t = ClusterOptions::default().min_thickness)]
    min_thickness: f64,

    /// Funde tonos que sólo se distinguen en luminosidad, dejando el tono donde
    /// está.
    ///
    /// Viene con un poco puesto, y no por bandear un cielo sino por la tinta: un
    /// trazo fino nunca llega a tinta plena, así que sale más claro que uno gordo
    /// y la paleta parte el mismo trazo en dos tonos. Subirlo ensancha las bandas
    /// de un degradado a propósito —la herramienta para un cielo liso—, pero
    /// pasado ~0.15 aplana el volumen de un dibujo y motea las fronteras.
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

impl Illustration {
    fn config(&self) -> Config {
        let cluster = ClusterOptions {
            simplify: Some(if self.no_simplify { 0.0 } else { self.simplify }),
            color_precision: self.color_precision,
            tolerance: self.tolerance,
            subpixel: !self.no_subpixel,
            relax: if self.no_relax { 0.0 } else { self.relax },
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
            // cuesta dibujo. La tolerancia es la de fábrica y no una más estrecha
            // porque la escala de trabajo lleva el rasgo pequeño a
            // `resample::FEATURE` píxeles, y ahí los escalones de la retícula
            // —0,5 y raíz(2)/2— quedan por debajo de la tolerancia y ya no muerden.
            fit: self.common.fit(FitArg::Polygon, Fit::TOLERANCE),
            ..Config::cluster(cluster)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Pixelart(args) => run_pixelart(args),
        Command::Illustration(args) => run_illustration(args),
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

fn run_illustration(args: &Illustration) -> Result<(), String> {
    let common = &args.common;
    if args.softness {
        return report_softness(args);
    }
    let (out, path) = convert(common, &args.config())?;
    if common.quiet {
        return Ok(());
    }

    report_background(&out);
    // El número de regiones es lo que se mueve al tocar el filtrado de motas, y
    // no se deduce de los paths: un color con varias regiones va en un `<g>`.
    if let img2svg::Detail::Cluster {
        regions,
        ramps,
        scale,
    } = out.detail
    {
        // Los degradados sólo se nombran cuando los hay: en un dibujo de colores
        // planos no hay ninguno y la línea no tiene por qué decirlo.
        let con_degradados = match ramps {
            0 => String::new(),
            1 => ", 1 degradado".to_string(),
            n => format!(", {n} degradados"),
        };
        // La escala sólo se nombra cuando ha habido reescalado: si se ha
        // trabajado sobre la retícula del original no hay nada que contar, y el
        // lienzo ya es el de la imagen.
        let a_escala = if scale == 1.0 {
            String::new()
        } else {
            format!(" (escala x{scale:.2})")
        };
        eprintln!(
            "lienzo {}x{}{}, {} regiones{}",
            out.canvas.0, out.canvas.1, a_escala, regions, con_degradados
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

/// Imprime la distribución de blandura de las fronteras de una imagen.
///
/// No escribe SVG: es un diagnóstico, y lo que hay que poder leer es si la
/// distribución **separa** los bordes del sombreado. Se enseña pesada por longitud
/// —una frontera de mil píxeles decide más que una de diez— y con las fronteras más
/// largas por su nombre, que es lo que permite comprobar la medida contra el dibujo:
/// las costuras de una barriga o un morro tienen que salir blandas y los bordes de
/// tinta duros.
fn report_softness(args: &Illustration) -> Result<(), String> {
    let data = std::fs::read(&args.common.input)
        .map_err(|e| format!("no se pudo leer {}: {e}", args.common.input.display()))?;
    let img = image::load_from_memory(&data)
        .map_err(|e| format!("no se pudo decodificar la imagen: {e}"))?
        .to_rgba8();

    let cluster = match args.config().segmentation {
        img2svg::Segmentation::Cluster(options) => options,
        _ => unreachable!("este subcomando siempre segmenta por clustering"),
    };
    let medidas = img2svg::softness_of(&img, &cluster);
    if medidas.is_empty() {
        eprintln!("ninguna frontera interior que medir");
        return Ok(());
    }

    let total: usize = medidas.iter().map(|m| m.cracks).sum();
    eprintln!(
        "{} fronteras interiores, {total} píxeles de frontera",
        medidas.len()
    );
    eprintln!("blandura  fronteras   píxeles de frontera");
    let tope = medidas.iter().map(|m| m.width as usize).max().unwrap_or(0);
    for ancho in 0..=tope {
        let iguales: Vec<&img2svg::softness::Softness> = medidas
            .iter()
            .filter(|m| m.width as usize == ancho)
            .collect();
        if iguales.is_empty() {
            continue;
        }
        let pixeles: usize = iguales.iter().map(|m| m.cracks).sum();
        eprintln!(
            "{ancho:>5} px  {:>6}      {:>6}  {:>3}%",
            iguales.len(),
            pixeles,
            pixeles * 100 / total
        );
    }

    let mut largas: Vec<&img2svg::softness::Softness> = medidas.iter().collect();
    largas.sort_by_key(|m| std::cmp::Reverse(m.cracks));
    eprintln!("\nlas fronteras más largas:");
    for m in largas.iter().take(14) {
        eprintln!(
            "  {:>5} px  blandura {:>2}  salto {:.3}  {} | {}",
            m.cracks,
            m.width,
            m.jump,
            m.colors.0.to_hex(),
            m.colors.1.to_hex()
        );
    }
    Ok(())
}
