//! El limado del contorno: qué temblor se va, qué esquina se queda, y el tope.
//!
//! Las tres cosas que hay que fijar de una etapa que **mueve** vértices en vez de
//! elegir cuáles se quedan: que sirva para algo, que no redondee lo que era una
//! esquina, y que ningún vértice acabe más lejos de su sitio de lo que se ha
//! pedido. Sin la tercera esto sería un suavizado, y un suavizado no se puede
//! poner por defecto.

#![cfg(feature = "illustration")]

use img2svg::{ClusterOptions, Config, Conversion, Fit};

/// Opciones de partida: sobre la retícula del dibujo y sin filtrar motas, que es
/// lo que deja ver el contorno y nada más.
fn opciones(relax: f64) -> ClusterOptions {
    ClusterOptions {
        simplify: Some(0.0),
        filter_speckle: 0,
        min_thickness: 0.0,
        subpixel: false,
        relax,
        ..ClusterOptions::default()
    }
}

fn convert(w: u32, h: u32, buf: &[u8], relax: f64, fit: Fit) -> Conversion {
    let config = Config {
        fit,
        ..Config::cluster(opciones(relax))
    };
    img2svg::convert_rgba(w, h, buf, &config).expect("la conversión no debe fallar")
}

/// Dos colores separados por una frontera casi recta a la que se le ha metido un
/// diente de un píxel cada pocas columnas: el temblor irregular de un dibujo real,
/// que no es una escalera regular y por eso el simplificador no lo puede tirar.
fn diente_de_sierra() -> (u32, u32, Vec<u8>) {
    let (w, h) = (60usize, 40usize);
    let mut buf = Vec::with_capacity(w * h * 4);
    // Un patrón fijo, no aleatorio: un test tiene que dar lo mismo cada vez.
    let diente = [0, 1, 0, 0, 1, 1, 0, 1];
    for y in 0..h {
        for x in 0..w {
            let frontera = 20 + diente[x % diente.len()];
            let color: [u8; 4] = if y < frontera {
                [220, 60, 60, 255]
            } else {
                [40, 70, 200, 255]
            };
            buf.extend_from_slice(&color);
        }
    }
    (w as u32, h as u32, buf)
}

/// Los `d` del documento, partidos en comandos con sus números.
///
/// Un `-` empieza número, no separa: el escritor pega `l-1-2` para ahorrar
/// espacios, y leerlo de otra forma se lleva el signo por delante.
fn comandos(svg: &str) -> Vec<(char, Vec<f64>)> {
    let mut out = Vec::new();
    for trozo in svg.split(" d=\"").skip(1) {
        let datos = &trozo[..trozo.find('"').unwrap()];
        let mut cmd = ' ';
        let mut args: Vec<f64> = Vec::new();
        let mut num = String::new();
        let cerrar = |num: &mut String, args: &mut Vec<f64>| {
            if !num.is_empty() {
                args.push(num.parse().unwrap());
                num.clear();
            }
        };
        for c in datos.chars() {
            if c.is_ascii_digit() || c == '.' {
                num.push(c);
            } else if c == '-' {
                cerrar(&mut num, &mut args);
                num.push('-');
            } else {
                cerrar(&mut num, &mut args);
                if c.is_ascii_alphabetic() {
                    if cmd != ' ' {
                        out.push((cmd, std::mem::take(&mut args)));
                    }
                    cmd = c;
                }
            }
        }
        cerrar(&mut num, &mut args);
        if cmd != ' ' {
            out.push((cmd, args));
        }
    }
    out
}

/// Todos los números de los `d` del documento.
fn numeros(svg: &str) -> Vec<f64> {
    comandos(svg)
        .into_iter()
        .flat_map(|(_, args)| args)
        .collect()
}

/// Los vértices absolutos de todos los subtrazados.
///
/// Hay que reconstruirlos porque el escritor emite comandos **relativos** —`l`,
/// `h`, `v`—, que son los que caben en menos bytes: los números de un `d` son
/// desplazamientos y no posiciones.
fn puntos(svg: &str) -> Vec<(f64, f64)> {
    let (mut x, mut y) = (0.0f64, 0.0f64);
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    let mut out = Vec::new();
    for (cmd, args) in comandos(svg) {
        match cmd {
            'M' => (x, y) = (args[0], args[1]),
            'm' => (x, y) = (x + args[0], y + args[1]),
            'L' => (x, y) = (args[0], args[1]),
            'l' => (x, y) = (x + args[0], y + args[1]),
            'H' => x = args[0],
            'h' => x += args[0],
            'V' => y = args[0],
            'v' => y += args[0],
            // De una cúbica sólo interesa el punto de llegada: los controles no
            // son vértices del contorno.
            'c' => (x, y) = (x + args[4], y + args[5]),
            'C' => (x, y) = (args[4], args[5]),
            'z' | 'Z' => (x, y) = (sx, sy),
            _ => {}
        }
        if cmd == 'M' || cmd == 'm' {
            (sx, sy) = (x, y);
        }
        out.push((x, y));
    }
    out
}

/// Limar el temblor deja el mismo dibujo con menos datos.
///
/// La frontera es casi recta y el diente se aparta de ella un píxel, más que la
/// tolerancia, así que sin limar el simplificador tiene que escribirlo entero: es
/// lo que promete. Limado, el diente entra dentro y el tramo sale de una pieza.
#[test]
fn el_temblor_limado_cuesta_menos_datos() {
    let (w, h, buf) = diente_de_sierra();
    let crudo = convert(w, h, &buf, 0.0, Fit::polygon());
    let limado = convert(w, h, &buf, 0.75, Fit::polygon());

    assert_eq!(
        crudo.paths, limado.paths,
        "limar no cambia cuántas figuras hay, sólo cómo se dibujan"
    );
    let (antes, despues) = (numeros(&crudo.svg).len(), numeros(&limado.svg).len());
    assert!(
        despues * 2 < antes,
        "el diente tiene que desaparecer: {antes} números antes, {despues} después"
    );
}

/// Una esquina en pico sigue en su sitio exacto.
///
/// Es lo que separa esto de un suavizado. Un rectángulo tiene cuatro giros de 90
/// grados y ninguno se puede mover: se reconocen por el giro a lo largo del
/// contorno, que en un arco está repartido y en una esquina está concentrado.
#[test]
fn las_esquinas_no_se_mueven() {
    let (w, h) = (24usize, 24usize);
    let mut buf = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let dentro = (6..18).contains(&x) && (6..18).contains(&y);
            buf.extend_from_slice(if dentro {
                &[220, 60, 60, 255]
            } else {
                &[240, 240, 240, 255]
            });
        }
    }
    let out = convert(w as u32, h as u32, &buf, 1.5, Fit::polygon());

    // Con el limado a tope, las cuatro esquinas del rectángulo tienen que seguir
    // siendo enteras: cualquier decimal en estos números sería una esquina redonda.
    for n in numeros(&out.svg) {
        assert_eq!(
            n.fract(),
            0.0,
            "un número no entero en un dibujo de sólo esquinas: {n}"
        );
    }
    assert!(
        out.svg.contains("M6 6") || out.svg.contains("M18 6") || out.svg.contains("M6 18"),
        "el rectángulo tiene que empezar en una de sus esquinas: {}",
        out.svg
    );
}

/// El tope se respeta: con el subpíxel apagado los vértices del contorno están en
/// la retícula entera, así que ninguno puede acabar a más del tope de un entero.
///
/// Se pide con tolerancia 0 para que el simplificador no quite ninguno: así lo que
/// se mide es exactamente lo que ha movido el limado, y nada más.
#[test]
fn ningun_vertice_se_va_mas_lejos_del_tope() {
    let (w, h, buf) = diente_de_sierra();
    for tope in [0.25, 0.5, 1.0] {
        let out = convert(w, h, &buf, tope, Fit::Polygon { tolerance: 0.0 });
        for (x, y) in puntos(&out.svg) {
            let desvio = ((x - x.round()).powi(2) + (y - y.round()).powi(2)).sqrt();
            assert!(
                desvio <= tope + 0.02,
                "con tope {tope} el vértice ({x}, {y}) se ha ido {desvio:.3} de la retícula"
            );
        }
    }
}
