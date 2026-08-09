# Using img2svg as a crate

## Entry points

```rust
// From an encoded image. Needs the `formats` feature, on by default.
let png = std::fs::read("sprite.png")?;
let out = img2svg::convert(&png, &img2svg::Config::default())?;

// From already-decoded pixels. Always available, and the path the web build
// uses: decoding is work the browser already knows how to do.
let out = img2svg::convert_rgba(width, height, &rgba, &config)?;

// From an `image::RgbaImage` you already have.
let out = img2svg::convert_image(&img, &config)?;
```

All three return the same `Conversion`.

## `Config`

| Field | Default | Meaning |
| --- | --- | --- |
| `scale: Option<f64>` | `None` | Cell size in real pixels. `None` detects it. |
| `offset: Option<(f64, f64)>` | `None` | Grid offset. `None` uses the detected phase. |
| `tolerance: f64` | `12.0` | Maximum distance for merging two colours. `0` keeps them all. |
| `alpha_threshold: u8` | `128` | Minimum alpha for a pixel to count as visible. |
| `pixel_size: Option<u32>` | `None` | Render size per pixel. `None` reproduces the original size. |
| `background: Option<String>` | `None` | Background rectangle colour. |
| `grouping: Grouping` | `Region` | `Region` = one path per block, `Color` = one per colour. |
| `remove_checkerboard: bool` | `true` | Look for the transparency checkerboard and clear it. |
| `remove_background: bool` | `false` | Clear the flat background and crop. |

## `Conversion`

```rust
pub struct Conversion {
    pub svg: String,
    pub grid: (usize, usize),        // logical pixels
    pub cell: (f64, f64),            // detected or forced cell, in real pixels
    pub offset: (f64, f64),
    pub colors: usize,
    pub paths: usize,
    pub subpaths: usize,
    pub checkerboard: Option<checker::Checkerboard>,
    pub background: Option<color::Rgba>,
}
```

`checkerboard` and `background` are `Some` only when something was actually
removed, which makes them useful for reporting:

```rust
if let Some(found) = out.checkerboard {
    println!("removed a {:.0} px transparency grid", found.cell.0);
}
```

## Errors

`Error::Decode` (unrecognised or corrupt file), `EmptyImage`, `BadBufferSize`
(the RGBA buffer does not match the given dimensions) and `InvalidScale`.

## Cargo features

| Feature | What it pulls in |
| --- | --- |
| `cli` (default) | The binary and the image decoders. |
| `formats` | Just the decoders, for `convert`. |
| `wasm` | The JavaScript bindings ([`src/wasm.rs`](../src/wasm.rs)). |

The web build takes none of them except `wasm`, which is what keeps the wasm
around 150 KB: the browser decodes, so the image codecs are half a megabyte of
dead weight there.

```sh
cargo add img2svg --no-default-features              # library only
cargo add img2svg --no-default-features -F formats   # plus decoders
```

## The JavaScript API

With `--features wasm`, `wasm-pack` produces `convertRgba(width, height, data,
options)`, where `options` is a plain object using the same keys as `Config` in
camelCase, all optional:

```js
import init, { convertRgba } from "./pkg/img2svg.js";
await init();

const out = convertRgba(width, height, rgba, {
  tolerance: 12,
  alphaThreshold: 128,
  removeCheckerboard: true,
  mergeColors: false,
});
console.log(out.svg, out.gridWidth, out.colors);
out.free();
```

`out` lives in wasm memory: read what you need, then call `free()`.
