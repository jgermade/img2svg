# img2svg

<img src="img2svg.svg" alt="" width="88" align="right">

Turns images into SVG. The **pixel art** mode detects the drawing's grid and
merges every contiguous block of same-coloured pixels into a single `<path>`,
tracing its minimal outline, holes included — rather than emitting one rectangle
per pixel. The **illustration** mode instead groups the colours into a palette
and traces the connected regions of each entry, for images that sit on no grid.

Three ways to use it: **web**, **CLI** and **library**.

## Web

<https://jgermade.github.io/img2svg/> — the whole conversion runs in the browser
(Rust compiled to WebAssembly); the image is never uploaded anywhere.

## CLI

```sh
cargo build --release
./target/release/img2svg pixelart sprite.png
```

Writes `sprite.svg` and reports what it did (the program speaks Spanish):

```
damero de transparencia #fefefe / #dadada, casilla 40.9x40.3 px: 16% a transparente
rejilla 80x126 (celda 20.45x20.36, offset 18.09,0.14)
43 colores, 385 paths, 1049 subtrazados -> sprite.svg (30.2 KB)
```

The subcommand picks how the image is read. `pixelart` assumes a regular grid;
`illustration` reinterprets the image as a drawing: it picks a **working scale**,
groups the colours into a palette, traces the connected regions of each entry and
files the staircase off their contours (`photo` is a legacy alias of it):

```sh
./target/release/img2svg illustration drawing.png --remove-background
```

```
lienzo 382x600 (escala x0.23), 321 regiones
16 colores, 321 paths, 1145 subtrazados -> drawing.svg (77.8 KB)
```

The working scale is the first thing this mode decides and the one knob worth
knowing. Every other constant in it is an absolute pixel count — a speck area, a
thickness, a fit deviation — so what they mean depends on how many pixels the
image spends on a feature, and no two images agree: a 300 px album cover spends
two pixels on an ink stroke, while a 1800×2823 airbrush scan spends two hundred on
a feature and two on its grain. So `--simplify` asks for the smallest feature that
should survive, as a fraction of the long side, and the image is resampled until
that feature is three pixels across. A small drawing goes up, which recovers the
edge the antialiasing wrote inside the pixel; a big scan comes down, which is what
averages the grain away. Measured on those two images, everything else unchanged:

| | before | now |
| --- | --- | --- |
| `cover.jpg`, 300×300 | 20 colours, 501 paths, 77 KB | **16, 450, 84 KB**, and the strokes are whole |
| `Sonic1.png`, 1800×2823 | 21 colours, 6,895 paths, 1,870 KB | **19, 137, 35 KB** |

`--fit` is shared by both, because how a contour becomes path data is a separate
decision from how the image becomes regions. `pixel` writes the staircase of
pixel edges literally; `polygon` straightens it into segments, which takes 12–30%
off the file depending on the tolerance; `spline` fits cubic Béziers, keeping the
corners sharp. The default differs by subcommand, because the two disagree about
what a staircase is: in a sprite it **is** the drawing, so `pixelart` writes it
literally, while off the grid it is only the pixel lattice showing through, so
`illustration` straightens it.

```sh
./target/release/img2svg illustration label.png --fit polygon
./target/release/img2svg illustration label.png --fit spline
```

Pick `spline` for an outline that stays smooth however far you zoom, not for a
smaller file: at the same tolerance it comes out 10–25% *bigger* than `polygon`,
because a cubic costs six numbers where a line costs two. It also starts at a
higher tolerance (1.5 against 0.75) — see [docs/illustration.md](docs/illustration.md) for
why, and for the measurements.

In `illustration`, a smooth ramp does not fit in a palette — a region has one colour and
that is all — so it arrives as a stack of flat bands. Where one gradient reproduces a
whole group of them, they are merged into a single shape painted with it: on a grainy
sky that is 121 shapes and 70.6 KB down to 3 and 5.9 KB, with the banding gone. The
gradient comes out `<linearGradient>` or `<radialGradient>` depending on which one
explains the group, because the shading of a round surface falls off with distance from
a point and through an axis it comes out smeared. A hard edge does not pass the test and
stays hard. `--no-ramps` turns it off.

When an image comes out wrong it is nearly always the grid: check the cell size
in the report and pin it by hand with `--scale`. Full option list in
[docs/cli.md](docs/cli.md).

## Library

```rust
let png = std::fs::read("sprite.png")?;
let out = img2svg::convert(&png, &img2svg::Config::default())?;
println!("{} colours in {} paths", out.colors, out.paths);
std::fs::write("sprite.svg", out.svg)?;
```

See [docs/library.md](docs/library.md) for `convert_rgba`, the full `Config` and
the cargo features.

## Documentation

| | |
| --- | --- |
| [docs/cli.md](docs/cli.md) | Every subcommand and option. |
| [docs/pixelart.md](docs/pixelart.md) | How grid detection, checkerboard removal and tracing work, and the shape of the SVG they produce. |
| [docs/library.md](docs/library.md) | Using it as a crate: entry points, `Config`, `Conversion`, cargo features. |
| [docs/illustration.md](docs/illustration.md) | The illustration mode: how its segmentation works, and the curve fitting still to come. |
| [docs/development.md](docs/development.md) | Building, the wasm package, tests and CI. |

Source comments and program output are in Spanish.
