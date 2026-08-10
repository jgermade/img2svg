# CLI reference

```
img2svg <COMMAND>
```

The subcommand chooses **how the image is read**, because that decision changes
which options make sense. `pixelart` assumes the drawing sits on a regular grid
and recovers it; `photo` groups the colours into a palette and traces the
connected regions of each entry.

Their options do not look alike because they are not measuring the same thing: a
tolerance of `12` in pixel art is an RGB distance between two tones of a discrete
palette, and one of `0.045` in photo is an Oklab distance inside a continuous
gradient.

## `img2svg pixelart <INPUT>`

Detects the grid, reduces the image to one logical pixel per cell, merges
near-identical colours and traces the outline of every region.

### Shared options

These do not depend on the segmentation and will be the same on every
subcommand.

| Option | Default | Description |
| --- | --- | --- |
| `<INPUT>` | | Input image. PNG, JPEG, GIF, BMP or WebP. |
| `-o, --output <FILE>` | input`.svg` | Output file. |
| `-b, --background <COLOUR>` | none | Adds a background rectangle, e.g. `"#ffffff"`. |
| `--fit <pixel\|polygon>` | `pixel` | How a contour becomes path data. See below. |
| `--fit-tolerance <N>` | `0.75` | Maximum deviation in pixels, for `--fit polygon`. |
| `-q, --quiet` | off | Silences the report on stderr. |

### `--fit`, the other axis

The subcommand chooses how the image becomes regions; `--fit` chooses how a
region's contour becomes path data. They are independent, which is why this one
is shared.

`pixel` writes the contour literally, as the staircase of pixel edges it is.
`polygon` runs Ramer–Douglas–Peucker over it and keeps only the vertices that
draw something, so a 45° staircase becomes one straight segment. On the corpus
images that is 12% off the file at the default tolerance and 30% at `1.5`:

| `--fit-tolerance` | vertices | SVG |
| --- | --- | --- |
| `pixel` | 30,231 | 117 KB |
| `0` | 30,231 | 117 KB |
| **`0.75` (default)** | **19,261** | **103 KB** |
| `1.0` | 12,036 | 85 KB |
| `1.5` | 10,943 | 82 KB |
| `3.0` | 9,059 | 76 KB |

The tolerance is a deviation in pixels, and 0.707 is the number that governs it:
that is how far the step of a 45° staircase sits from its own chord, so below it
nothing straightens at all, and `0` reproduces `pixel` exactly. The default sits
just above.

What it does **not** promise is that a feature taller than the tolerance
survives. RDP measures against whatever chord the recursion currently holds, not
against the vertex's neighbours, so a chord coming from far away swallows a
one-pixel bump that on its own would sit 1.0 away. The only guarantee is the
ceiling: no point of the contour ends up further than the tolerance from what
gets drawn.

Curve fitting — `spline` — is [not built yet](curves.md).

### Pixel art options

| Option | Default | Description |
| --- | --- | --- |
| `-s, --scale <N>` | detected | Cell size in real pixels. `1` disables downscaling. Accepts decimals. |
| `--offset <X> <Y>` | detected | Grid offset, for when detection gets the phase wrong. |
| `-t, --tolerance <N>` | `12` | Maximum distance for merging two colours. `0` keeps them all. |
| `-a, --alpha-threshold <N>` | `128` | Minimum alpha for a pixel to count as visible. |
| `-p, --pixel-size <N>` | cell size | Render size of each pixel, in SVG units. Only changes `width`/`height`; the `viewBox` is always in drawn pixels. |
| `-m, --merge-colors` | off | One path per colour instead of one per contiguous block. |
| `-k, --keep-checkerboard` | off | Skips looking for the transparency checkerboard. |
| `-r, --remove-background` | off | Clears the flat background and crops the SVG to the artwork. |

### The report

Written to stderr unless `--quiet`:

```
damero de transparencia #fefefe / #dadada, casilla 40.9x40.3 px: 16% a transparente
fondo #ffffff retirado y lienzo recortado
rejilla 80x126 (celda 20.45x20.36, offset 18.09,0.14)
43 colores, 385 paths, 1049 subtrazados -> sprite.svg (30.2 KB)
```

The first two lines only appear when something was actually removed. The grid
line is the one to read when output looks wrong — see below.

## When the output is wrong

Nearly always the grid.

**The drawing comes out blurry or doubled.** The detected cell is a fraction of
the real one, so several art pixels landed in one cell. Read `celda` in the
report and pin it: `--scale 20.45`.

**The drawing comes out shifted by a pixel.** The period is right but the phase
is not. Pin it with `--offset X Y`, using the reported values as a starting
point.

**Detection finds a grid where there is none.** Small or very regular images can
score a false period. `--scale 1` turns detection off entirely.

**Part of the artwork disappeared.** The checkerboard remover matched something
it should not have. `--keep-checkerboard` turns it off.

**Too many or too few colours.** `--tolerance` controls how aggressively
near-identical tones collapse together. `0` keeps every distinct colour, which on
a noisy JPEG means thousands.

## `img2svg photo <INPUT>`

Groups the colours into a palette, labels the connected regions of each entry,
merges away the ones that carry no drawing, and traces every boundary once.

Takes the same [shared options](#shared-options). Its own:

| Option | Default | Description |
| --- | --- | --- |
| `-t, --tolerance <N>` | `0.045` | Maximum Oklab distance between a colour and the region that paints it. The scale is perceptual and runs 0 to 1: black to white is `1.0`. |
| `-c, --color-precision <N>` | `5` | Bits per channel the colour is cut to before grouping. |
| `-a, --alpha-threshold <N>` | `128` | Minimum alpha for a pixel to count as visible. |
| `--filter-speckle <N>` | `4` | Area in pixels up to which a region merges into a neighbour. |
| `--min-thickness <N>` | `1` | Thickness below which a region merges into a neighbour, however large its area. |
| `--gradient-step <N>` | `0` | Widens the bands of a gradient by merging on lightness difference alone. |
| `--max-colors <N>` | `0` | Cap on palette entries. `0` is no cap. |
| `-r, --remove-background` | off | Clears the flat background and crops the SVG to the artwork. |

### The report

```
fondo #ffffff retirado y lienzo recortado
lienzo 662x1079, 1099 regiones
37 colores, 1099 paths, 1521 subtrazados -> label.svg (107.6 KB)
```

The region count is the number to watch when tuning the speck filters: the colour
count barely moves and this does.

### The two knobs that surprise people

**`--min-thickness` is the one nobody else has.** Thickness is `2 × area /
perimeter`, which stays near 0.5 for a one-pixel band however long it is and
grows as `s/2` for a compact block of side `s`. It exists because besides
isolated dots there are bands one pixel wide along every colour boundary — the
antialiasing fringe of the source — and a 1×8 band has eight pixels, so an area
threshold never sees it. On one corpus image: 12,498 regions unfiltered, 4,157
with area alone, 1,298 with both.

The default of `1` is exactly the thickness of a 2×2 block, so it removes
**everything one pixel wide**, a genuine hairline included. That is the price of
removing the fringes, and it is worth paying on a photo, where fringes outnumber
real one-pixel features by orders of magnitude. On fine line art, set it to `0`.

**`--gradient-step` flattens shading.** It merges tones that differ only in
lightness, leaving hue alone, so a smooth sky comes out in wider bands instead of
many thin ones — 74 colours down to 31 at `0.15` on one image. On artwork with
volume it does the opposite of what you want, because the shading *is* a
lightness ramp and flattening it flattens the modelling; past about `0.15` the
band boundaries start to mottle. Hence the default of 0.
