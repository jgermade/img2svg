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

## `Config`: two orthogonal axes

**Segmentation** decides how the image becomes a set of regions; **fitting**
decides how a region's contour becomes path data. They are separate stages of the
same pipeline, so they combine freely.

```rust
pub struct Config {
    pub segmentation: Segmentation,   // Grid(GridOptions) | Cluster(ClusterOptions)
    pub fit: Fit,                     // Pixel | Polygon { tolerance }
    pub background: Option<String>,   // belongs to neither axis
}
```

`Config::default()` is `Grid` segmentation with `Pixel` fitting — the pixel art
path. To vary it:

```rust
use img2svg::{ClusterOptions, Config, GridOptions, Grouping};

let config = Config::grid(GridOptions {
    tolerance: 24.0,
    grouping: Grouping::Color,
    ..GridOptions::default()
});

// Or tweak in place. `grid_options_mut` returns `None` on any other
// segmentation, so it is an `Option`, not a panic waiting to happen.
let mut config = Config::default();
config.grid_options_mut().unwrap().remove_background = true;

// The photo path, behind the `photo` feature:
let config = Config::cluster(ClusterOptions {
    tolerance: 0.06,
    remove_background: true,
    ..ClusterOptions::default()
});
```

### `GridOptions`

| Field | Default | Meaning |
| --- | --- | --- |
| `scale: Option<f64>` | `None` | Cell size in real pixels. `None` detects it. |
| `offset: Option<(f64, f64)>` | `None` | Grid offset. `None` uses the detected phase. |
| `tolerance: f64` | `12.0` | Maximum distance for merging two colours. `0` keeps them all. |
| `alpha_threshold: u8` | `128` | Minimum alpha for a pixel to count as visible. |
| `pixel_size: Option<u32>` | `None` | Render size per pixel. `None` reproduces the original size. |
| `grouping: Grouping` | `Region` | `Region` = one region per contiguous block, `Color` = one per colour. |
| `remove_checkerboard: bool` | `true` | Look for the transparency checkerboard and clear it. |
| `remove_background: bool` | `false` | Clear the flat background and crop. |

### `ClusterOptions`

Needs the `photo` feature, on by default through `cli`. Nothing here is shared
with `GridOptions`: they are two readings of what the image is, and the numbers
are not on the same scale. See [the curves mode](curves.md) for why each one
exists.

| Field | Default | Meaning |
| --- | --- | --- |
| `color_precision: u8` | `5` | Bits per channel the colour is cut to before grouping. |
| `tolerance: f64` | `0.045` | Maximum Oklab distance between a colour and its palette entry. Black to white is `1.0`. |
| `smoothing: usize` | `2` | Passes that regularise the palette assignment against each pixel's neighbourhood. `0` turns it off. Loosens the tolerance guarantee to `smooth::CEILING` × it. |
| `alpha_threshold: u8` | `128` | Minimum alpha for a pixel to count as visible. |
| `filter_speckle: usize` | `4` | Area up to which a region merges into a neighbour. `0` merges nothing. |
| `min_thickness: f64` | `1.0` | Thickness (`2 × area / perimeter`) below which a region merges into a neighbour. `1.0` is a 2×2 block, so it removes everything one pixel wide. |
| `gradient_step: f64` | `0.0` | How much *lightness* difference is merged past the tolerance. Widens gradient bands; flattens shading. |
| `max_colors: usize` | `0` | Cap on palette entries. `0` is no cap. Drops the tolerance guarantee. |
| `palette: Vec<Rgba>` | empty | An imposed palette. Non-empty means exactly this palette, nothing added. |
| `remove_background: bool` | `false` | Clear the flat background and crop. |

### `Fit`

The other axis, and the one that combines with either segmentation.

| Variant | Meaning |
| --- | --- |
| `Fit::Pixel` | The literal staircase of pixel edges, in `h`/`v` commands. The default. |
| `Fit::Polygon { tolerance }` | Straight segments, keeping only the vertices that draw something (Ramer–Douglas–Peucker). `Fit::polygon()` is the default tolerance, `Fit::TOLERANCE` = 0.75 px. |
| `Fit::Spline { tolerance }` | Cubic Béziers fitted by least squares, split at the corners (Schneider). `Fit::spline()` is the default tolerance, `Fit::SPLINE_TOLERANCE` = 1.5 px. |

`tolerance` is a maximum deviation in pixels: no point of the contour ends up
further than that from what gets drawn, and `0.0` reproduces `Fit::Pixel`
exactly. Below 0.707 nothing straightens at all — that is how far a 45°
staircase step sits from its own chord. See [the CLI reference](cli.md#--fit-the-other-axis)
for the measured trade-off.

The two defaults differ because the floors do: the polygon picks lattice
vertices and honours 0 exactly, while a curve is fitted against a staircase that
already carries 0.707 px of quantisation error, and below about 1.0 it chases the
steps instead of the shape. `Fit::default_tolerance(name)` is what the CLI and
the wasm reader use to pick one from a fitter's name.

`Fit::Spline` does not produce smaller files than `Fit::Polygon` — it is 10–25%
bigger at the same tolerance — and that is not a defect: it buys an outline that
stays smooth at any zoom. See [the curves notes](curves.md).

Fitting happens **once per half-edge**, before rings are assembled, so the two
faces of a shared boundary get identical geometry. `fit::Fitted` is the type that
enforces the order: build it from the whole `Regions`, then ask it for a ring.

## The intermediate representation

Between the two stages sits [`region::Regions`](../src/region.rs): a list of
regions, each with a colour, an area and rings, plus a pool of `HalfEdge`s. A
half-edge is a stretch of boundary with a region on each side.

Boundaries are stored as half-edges rather than per-region loops on purpose. Two
regions sharing a border must be fitted **once**, not once per face — otherwise
the two fits disagree and a hairline shows through between them. With `h`/`v` on
integer coordinates this cannot happen, but with Béziers it can, and the type has
to be right before the curve fitters are written.

Grid segmentation currently leaves `right` at `None`: it traces each region on
its own and shares no geometry. Populating it comes with cluster segmentation,
which also needs it to know which neighbour to merge a speck into.

## `Conversion`

```rust
pub struct Conversion {
    pub svg: String,
    pub canvas: (usize, usize),      // the viewBox, in the units the paths use
    pub colors: usize,
    pub paths: usize,
    pub subpaths: usize,
    pub background: Option<color::Rgba>,
    pub detail: Detail,
}

pub enum Detail {
    Grid {
        cell: (f64, f64),            // detected or forced cell, in real pixels
        offset: (f64, f64),
        checkerboard: Option<checker::Checkerboard>,
    },
    Cluster { regions: usize },
}
```

What means something for every segmentation is a field; what only means something
for one is in `Detail`. `canvas` is a field rather than two names inside `Detail`
because it is the same number either way — what goes in the `viewBox` — and it is
not the size of the input image: the grid path reports drawn pixels rather than
real ones, and removing the background crops on both paths.

`background` is `Some` only when something was actually removed. For the grid
data there are accessors that save writing a `match` with an arm you do not care
about, each `None` on any other segmentation:

```rust
if let Some(found) = out.checkerboard() {
    println!("removed a {:.0} px transparency grid", found.cell.0);
}
let cell = out.cell();       // Option<(f64, f64)>
let offset = out.offset();   // Option<(f64, f64)>
```

## Errors

`Error::Decode` (unrecognised or corrupt file), `EmptyImage`, `BadBufferSize`
(the RGBA buffer does not match the given dimensions) and `InvalidScale`.

## Cargo features

| Feature | What it pulls in |
| --- | --- |
| `cli` (default) | The binary, the image decoders and the photo path. |
| `formats` | Just the decoders, for `convert`. |
| `photo` | Cluster segmentation: `cluster`, `speckle`, `boundary` and `Segmentation::Cluster`. |
| `wasm` | The JavaScript bindings ([`src/wasm.rs`](../src/wasm.rs)). |

`photo` is separate so a pixel-art-only wasm bundle can leave the photo code out:
whoever comes to convert a sprite has no reason to download it.

The web build takes none of them except `wasm`, which is what keeps the wasm
around 150 KB: the browser decodes, so the image codecs are half a megabyte of
dead weight there.

```sh
cargo add img2svg --no-default-features              # library only
cargo add img2svg --no-default-features -F formats   # plus decoders
```

## The JavaScript API

With `--features wasm`, `wasm-pack` produces one function per segmentation, each
taking a plain object of camelCase keys, all optional:

```js
import init, { convertRgba, convertPhoto } from "./pkg/img2svg.js";
await init();

const out = convertRgba(width, height, rgba, {
  tolerance: 12,
  alphaThreshold: 128,
  removeCheckerboard: true,
  mergeColors: false,
});
console.log(out.svg, out.gridWidth, out.colors);
out.free();

// Adding `photo` to the features adds this one, returning a PhotoConversion.
const photo = convertPhoto(width, height, rgba, {
  tolerance: 0.045,
  colorPrecision: 5,
  filterSpeckle: 4,
  minThickness: 1,
  gradientStep: 0,
  maxColors: 0,
  removeBackground: false,
});
console.log(photo.svg, photo.canvasWidth, photo.regions);
photo.free();
```

Two functions rather than one with a `mode` key, because the two option sets do
not overlap: each reads only its own, and the `.d.ts` says which returns what.

`out` lives in wasm memory: read what you need, then call `free()`.
