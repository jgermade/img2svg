# img2svg

<img src="img2svg.svg" alt="" width="88" align="right">

Turns images into SVG. The **pixel art** mode detects the drawing's grid and
merges every contiguous block of same-coloured pixels into a single `<path>`,
tracing its minimal outline, holes included — rather than emitting one rectangle
per pixel.

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
`photo`, which clusters colours instead, is [not built yet](docs/curves.md).

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
| [docs/curves.md](docs/curves.md) | The planned mode for photos and smooth artwork. |
| [docs/development.md](docs/development.md) | Building, the wasm package, tests and CI. |

Source comments and program output are in Spanish.
