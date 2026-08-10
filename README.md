# img2svg

<img src="img2svg.svg" alt="" width="88" align="right">

Turns images into SVG. The **pixel art** mode detects the drawing's grid and
merges every contiguous block of same-coloured pixels into a single `<path>`,
tracing its minimal outline, holes included — rather than emitting one rectangle
per pixel. The **photo** mode instead groups the colours into a palette and
traces the connected regions of each entry, for images that sit on no grid.

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
`photo` groups the colours into a palette and traces the connected regions of
each entry, which is what an image without a grid needs:

```sh
./target/release/img2svg photo label.png --remove-background
```

```
fondo #ffffff retirado y lienzo recortado
lienzo 662x1079, 1099 regiones
37 colores, 1099 paths, 1521 subtrazados -> label.svg (107.6 KB)
```

`--fit` is shared by both, because how a contour becomes path data is a separate
decision from how the image becomes regions. `pixel` writes the staircase of
pixel edges literally; `polygon` straightens it into segments, which takes 12–30%
off the file depending on the tolerance; `spline` fits cubic Béziers, keeping the
corners sharp:

```sh
./target/release/img2svg photo label.png --fit polygon
./target/release/img2svg photo label.png --fit spline
```

Pick `spline` for an outline that stays smooth however far you zoom, not for a
smaller file: at the same tolerance it comes out 10–25% *bigger* than `polygon`,
because a cubic costs six numbers where a line costs two. It also starts at a
higher tolerance (1.5 against 0.75) — see [docs/curves.md](docs/curves.md) for
why, and for the measurements.

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
| [docs/curves.md](docs/curves.md) | The photo mode: how its segmentation works, and the curve fitting still to come. |
| [docs/development.md](docs/development.md) | Building, the wasm package, tests and CI. |

Source comments and program output are in Spanish.
