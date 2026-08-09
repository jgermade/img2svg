# px2svg

<img src="px2svg.svg" alt="" width="88" align="right">

Turns pixel art into SVG. Instead of emitting one rectangle per pixel, it merges
every contiguous block of same-coloured pixels into a single `<path>`, tracing
its minimal outline (holes included).

Three ways to use it: **web**, **CLI** and **library**.

## Web

<https://jgermade.github.io/px2svg/> — the whole conversion runs in the browser
(Rust compiled to WebAssembly); the image is never uploaded anywhere.

## CLI

```
cargo build --release
./target/release/px2svg examples/sonic.png
```

Writes `examples/sonic.svg` and reports what it did (the program speaks Spanish):

```
damero de transparencia #fefefe / #dadada, casilla 40.9x40.3 px: 16% a transparente
rejilla 80x126 (celda 20.45x20.36, offset 18.09,0.14)
43 colores, 385 paths, 1049 subtrazados -> examples/sonic.svg (30.2 KB)
```

| Option | Description |
| --- | --- |
| `-o, --output <FILE>` | Output file (defaults to the input with an `.svg` extension). |
| `-s, --scale <N>` | Cell size in real pixels. Auto-detected by default; `1` disables downscaling. Accepts decimals. |
| `--offset <X> <Y>` | Grid offset, for when detection gets it wrong. |
| `-t, --tolerance <N>` | Maximum distance for merging two colours (default `12`; `0` keeps them all). |
| `-a, --alpha-threshold <N>` | Minimum alpha for a pixel to count as visible (default `128`). |
| `-p, --pixel-size <N>` | Render size of each pixel, in SVG units. |
| `-b, --background <COLOUR>` | Adds a background rectangle. |
| `-m, --merge-colors` | One path per colour instead of one per contiguous block. |
| `-k, --keep-checkerboard` | Skips looking for the transparency checkerboard. |
| `-r, --remove-background` | Clears the flat background and crops the SVG to the artwork. |
| `-q, --quiet` | Silences the report. |

Input formats: PNG, JPEG, GIF, BMP and WebP.

## Library

```rust
let png = std::fs::read("sprite.png")?;
let out = px2svg::convert(&png, &px2svg::Config::default())?;
println!("{} colours in {} paths", out.colors, out.paths);
if let Some(checkerboard) = out.checkerboard {
    println!("removed a {:.0} px transparency grid", checkerboard.cell.0);
}
std::fs::write("sprite.svg", out.svg)?;
```

`convert_rgba(width, height, &rgba, &config)` takes already-decoded pixels, which
is the path the web build uses. Cargo features keep each consumer to what it
actually needs:

| Feature | What it pulls in |
| --- | --- |
| `cli` (default) | The binary and the image decoders. |
| `formats` | Just the decoders, for `convert`. |
| `wasm` | The JavaScript bindings (`src/wasm.rs`). |

## How it works

1. **Transparency checkerboard** ([`src/checker.rs`](src/checker.rs)). Screenshot
   an editor and the white/grey checkerboard behind the artwork gets baked in as
   opaque pixels. The most frequent grey pairs are collected and, for each, the
   runs of solid colour are measured: a real grid makes them all the same length
   and all starting in the same phase. The pair covering the most image wins.
   Only cells that match all the way through **and whose neighbours alternate**
   are cleared — a flat white area of the artwork (a character's eye, say) also
   fits the light cells perfectly, but its neighbours don't alternate. From those
   cells the erasure spreads by contiguity.
2. **Grid detection** ([`src/grid.rs`](src/grid.rs)). Pixel art almost never
   arrives at 1:1: one drawn pixel covers NxN real ones. Colour changes therefore
   land on a regular grid, which makes the image gradient a periodic signal. For
   each candidate period, the share of gradient energy concentrated at that
   frequency is measured, and the largest well-scoring period wins (its divisors
   describe the same grid, just subdivided). The phase gives the offset, and the
   period need not be a whole number, so rescaled images work too.
3. **Downscaling**. Only the middle of each cell is sampled — dodging the
   antialiasing along the edges — and the majority colour is taken.
4. **Palette** ([`src/color.rs`](src/color.rs)). Near-identical colours, the
   typical signature of compression noise, collapse onto the dominant tone.
5. **Background** ([`src/background.rs`](src/background.rs)), only with
   `--remove-background`. The colour dominating the canvas border is taken as the
   background and cleared by flooding inwards from outside, so the same tone
   enclosed within the artwork survives. The canvas is then cropped to what's
   left.
6. **Tracing** ([`src/trace.rs`](src/trace.rs)). For each colour, the edges
   separating its pixels from everything else are collected with a consistent
   orientation and chained until they close into loops. Each loop is a subpath;
   outlines and holes coexist in the same `<path>` thanks to
   `fill-rule="evenodd"`. Collinear vertices are dropped, so a rectangular region
   ends up as four points.

## SVG structure

Each contiguous block of pixels is a `<path>`, and all the blocks of one colour
live inside a `<g fill="…">`:

```xml
<g fill="#000000">
  <path d="M29 31v1h-1v1h1v2h-1v-1h-1v7h1v2h1v1h1v-1h1v-1h1v-9h-1v-2z"/>
  <path d="M8 34v1h1v1h-1v5h1v2h1v1h1v-1h1v-5h-1v-1h-1v-3z"/>
</g>
```

Every shape in the document can be selected and moved on its own in a vector
editor. `--merge-colors` goes back to a single path per colour with all its
blocks as subpaths: 20–30% smaller, but selecting one shape selects everything
sharing its colour.

Blocks are grouped with 8-connectivity, so a diagonal run of pixels — everywhere
in pixel art — is one shape rather than a string of little squares.

The `viewBox` is in drawn pixels (1 unit = 1 pixel), and `width`/`height`
reproduce the original image size.

## Development

```
cargo test
cargo build --release                     # CLI at target/release/px2svg
wasm-pack build --release --target web \
  --out-dir docs/pkg --out-name px2svg \
  -- --no-default-features --features wasm # web package in docs/pkg
```

To try the web build locally, serve `docs/` once the wasm is compiled:

```
python3 -m http.server 8765 --directory docs
```

`.github/workflows/pages.yml` runs the tests, compiles the wasm and publishes
`docs/` on every push to `main`. It needs Pages enabled on the repository under
**Settings → Pages → Source: GitHub Actions**.

What gets published is `docs/` and nothing else: the wasm, the page and the
logo. The images under `examples/` stay out, so the site never carries megabytes
of PNG around.

When an image comes out wrong it's nearly always the grid: check the cell size
the program reports and pin it by hand with `--scale`. And if the checkerboard
removal eats something it shouldn't, `--keep-checkerboard` turns it off.

Source comments and program output are in Spanish.
