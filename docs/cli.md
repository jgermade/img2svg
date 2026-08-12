# CLI reference

```
img2svg <COMMAND>
```

The subcommand chooses **how the image is read**, because that decision changes
which options make sense. `pixelart` assumes the drawing sits on a regular grid
and recovers it; `illustration` groups the colours into a palette and traces the
connected regions of each entry.

Their options do not look alike because they are not measuring the same thing: a
tolerance of `12` in pixel art is an RGB distance between two tones of a discrete
palette, and one of `0.045` in illustration is an Oklab distance inside a continuous
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
| `--fit <pixel\|polygon\|spline>` | `pixel` on `pixelart`, `polygon` on `illustration` | How a contour becomes path data. See below. |
| `--fit-tolerance <N>` | per fitter | Maximum deviation in pixels: 0.75 for `polygon`, 1.5 for `spline`. |
| `-q, --quiet` | off | Silences the report on stderr. |

### `--fit`, the other axis

The subcommand chooses how the image becomes regions; `--fit` chooses how a
region's contour becomes path data. They are independent, which is why this one
is shared.

Its default is not, though, because the two subcommands disagree about what a
staircase *is*. In a sprite the staircase **is** the drawing and rounding it off
would be vandalism, so `pixelart` defaults to `pixel`. Off the grid there is no
staircase to preserve — only the one the pixel grid imposes — so `illustration`
defaults
to `polygon`, which took 23% off an illustration and 32% off a 5 Mpx painting with
nothing visible lost.

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

`spline` fits cubic Béziers, keeping the corners sharp and everything else
smooth. It is **not** a way to get a smaller file — measured against `polygon` at
the same tolerance it is 10–25% bigger, because a cubic costs six numbers where a
line costs two. What it buys is an outline that stays smooth however far you zoom
in, instead of facets frozen at the tolerance you picked.

It also needs more room than `polygon`, which is why `--fit-tolerance` has no
single default: the contour it starts from is a staircase with up to 0.707 px of
its own error baked in, and below about 1.0 the fitter spends its budget chasing
the steps rather than the shape. Hence 1.5. See [the illustration
notes](illustration.md#bézier-fitting-fitsplinesrs) for the measurements.

What it does **not** promise is that a feature taller than the tolerance
survives. RDP measures against whatever chord the recursion currently holds, not
against the vertex's neighbours, so a chord coming from far away swallows a
one-pixel bump that on its own would sit 1.0 away. The only guarantee is the
ceiling: no point of the contour ends up further than the tolerance from what
gets drawn.

The same ceiling is all `spline` promises too.

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

## `img2svg illustration <INPUT>`

Groups the colours into a palette, labels the connected regions of each entry,
merges away the ones that carry no drawing, and traces every boundary once.

`img2svg photo` is an alias: it is what this subcommand was called until
2026-08-11, when the mode was renamed after what it is for.

Takes the same [shared options](#shared-options). Its own:

| Option | Default | Description |
| --- | --- | --- |
| `--simplify <N>` | `5` | The smallest feature that survives, in per mille of the long side. It is what picks the resolution the image is segmented at. See below. |
| `--no-simplify` | off | Segments on the source's own lattice, choosing no working scale. |
| `-t, --tolerance <N>` | `0.045` | Maximum Oklab distance between a colour and the region that paints it. The scale is perceptual and runs 0 to 1: black to white is `1.0`. |
| `-c, --color-precision <N>` | `5` | Bits per channel the colour is cut to before grouping. |
| `--smoothing <N>` | `2` | Passes that regularise the palette assignment against each pixel's neighbourhood. `0` turns it off. |
| `--no-subpixel` | off | Keeps contour vertices on the integer lattice instead of where the image says the edge is. |
| `--relax <N>` | `0.75` | How far a contour vertex may move, in working pixels, to file off the staircase wobble. Corners do not move. |
| `--no-relax` | off | Leaves the contour exactly as the tracing produced it. |
| `--softness` | off | Prints how soft each boundary is and writes no SVG. Softness is what decides which seams may become a gradient with only two colours, and which groups get the radial model tried on them; this is how to check the measure against the drawing when a gradient shows up where it should not, or does not where it should. |
| `--no-ramps` | off | Keeps every band of a gradient as its own flat shape instead of merging them into a `<linearGradient>` or `<radialGradient>`. |
| `-a, --alpha-threshold <N>` | `128` | Minimum alpha for a pixel to count as visible. |
| `--filter-speckle <N>` | `9` | Area in pixels up to which a region merges into a neighbour. |
| `--min-thickness <N>` | `3` | Thickness below which a region **may** merge into a neighbour — only if its colour is a mixture of its two main neighbours. |
| `--gradient-step <N>` | `0.05` | Merges tones that differ only in lightness. On by a little, for split ink; raising it widens gradient bands. |
| `--min-color-share <N>` | `0.002` | What a colour has to be worth, as a fraction of the image, to get a palette entry of its own. `0` gives one to anybody. |
| `--max-colors <N>` | `0` | Cap on palette entries. `0` is no cap. |
| `-r, --remove-background` | off | Clears the flat background and crops the SVG to the artwork. |

### The working scale, which is the first thing to know

Every other constant in this subcommand is an absolute pixel count — an area, a
thickness, a deviation — so what they mean depends on how many pixels the image
spends on a feature, and two images never agree about that. A 300 px album cover
spends ~2 px on an ink stroke; a 1800×2823 airbrush scan spends ~200 px on a
feature and ~2 px on its grain. The same numbers simplify one and destroy the
other.

So `--simplify` asks for the thing you actually care about — the smallest feature
that survives, as a fraction of the long side — and the image is resampled so that
feature lands on three working pixels before anything else runs. Upscaling and
downscaling both fall out of that one rule: a small drawing goes up, which
recovers the edge the source's antialiasing wrote inside the pixel, and a big scan
comes down, which is what averages the grain away.

One consequence is worth stating because it is the whole idea in a line: since the
feature is asked for as a fraction, **the working canvas depends only on
`--simplify` and not on the size of the file**. At the default, both a thumbnail
and a 5 Mpx scan are segmented with their long side at 600 px. The report says
what happened:

```
lienzo 600x600 (escala x2.00), 1171 regiones
```

The SVG still announces itself at the source's size: the `viewBox` is in working
pixels, `width`/`height` in the image's own.

### The report

```
lienzo 600x600 (escala x2.00), 434 regiones, 6 degradados
16 colores, 440 paths, 688 subtrazados -> cover.svg (84.3 KB)
```

The region count is the number to watch when tuning the speck filters: the colour
count barely moves and this does. The `(escala …)` only appears when the image was
resampled, and so does `, N degradados`: on flat-colour artwork there are none and
the line has no reason to say so. `fondo … retirado y lienzo recortado` joins it
above when `--remove-background` found one.

### The two knobs that surprise people

**`--min-thickness` is the one nobody else has.** Thickness is `2 × area /
perimeter`, which stays near 0.5 for a one-pixel band however long it is and
grows as `s/2` for a compact block of side `s`. It exists because besides
isolated dots there are bands one pixel wide along every colour boundary — the
antialiasing fringe of the source — and a 1×8 band has eight pixels, so an area
threshold never sees it. On one corpus image: 12,498 regions unfiltered, 4,157
with area alone, 1,298 with both.

Being thin is not enough to be merged, though, because **an ink stroke is thin
too**: on a 300 px illustration the strokes are one or two pixels wide, so a
thickness threshold on its own takes the spectacles, the mouth and the eyebrows
with the fringes. What separates them is colour, not geometry:

> a fringe is a **mixture** of its two neighbours — its colour sits on the segment
> between them — and a stroke is not: black ink between skin and a yellow ground
> is nowhere near any mixture of skin and yellow.

So thickness only nominates and the mixture test decides. Raising it therefore
does not eat the drawing; what it does is admit fatter fringes. The case of a
stroke with a single neighbour — a mouth line inside a face — needs no separate
rule: the segment degenerates to a point, and the stroke is nowhere near it.

**Gradients come out as gradients, when they are gradients.** A smooth ramp does
not fit in a palette — a region has one colour and that is all — so it arrives as
a stack of bands whose boundaries draw nothing: they only mark where the ramp
crossed a quantisation threshold, following the noise of the source. A group of
bands that one gradient reproduces is merged into a single shape with that
gradient. On a 900×600 sky with photographic grain that is 121 shapes and 70.6 KB
down to 3 and 5.9 KB, with the banding gone. On flat-colour artwork it finds
almost nothing, which is correct, and then it costs a little — about 2 KB and 10%
of the conversion time on an album cover. A hard edge does not pass the test and
stays hard.

The gradient comes out `<linearGradient>` or `<radialGradient>` according to which
one explains the group — a sky is colour along an axis, and the shading of a round
surface is colour by distance from a centre, which through an axis comes out smeared
along a direction the drawing does not have. And a group of just **two** colours can
be a gradient too, but only where the seam between them is soft: a shading
terminator is two tones with a wide transition between them, and by colour count
alone it would never qualify, so a belly came out with a hard crescent across it.
`--softness` prints that measure.

It pulls against `--min-color-share`, which is worth knowing: that option drops
the palette entries that paint little, the middle of a ramp paints little, and
what is left are steps too big for a gradient to reproduce. Turning it off on a
5 Mpx drawing goes from 24 gradients to 98 — and still gives a *bigger* document,
because the finer palette costs more than the gradients save.

**`--gradient-step` flattens shading.** It merges tones that differ only in
lightness, leaving hue alone, so a smooth sky comes out in wider bands instead of
many thin ones — 74 colours down to 31 at `0.15` on one image. On artwork with
volume it does the opposite of what you want, because the shading *is* a
lightness ramp and flattening it flattens the modelling; past about `0.15` the
band boundaries start to mottle. Hence a default of `0.05`: a little, and not for
banding but for **split ink** — a thin stroke never reaches full ink, so the palette
splits one stroke into two tones and the same stroke shows up in patches of near
black and dark grey.
