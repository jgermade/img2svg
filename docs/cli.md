# CLI reference

```
img2svg <COMMAND>
```

The subcommand chooses **how the image is read**, because that decision changes
which options make sense. `pixelart` assumes the drawing sits on a regular grid
and recovers it; `photo`, which clusters colours instead, is
[not built yet](curves.md).

## `img2svg pixelart <INPUT>`

Detects the grid, reduces the image to one logical pixel per cell, merges
near-identical colours and traces the outline of every region.

### Shared options

These do not depend on the segmentation and will be the same on every
subcommand.

| Option | Description |
| --- | --- |
| `<INPUT>` | Input image. PNG, JPEG, GIF, BMP or WebP. |
| `-o, --output <FILE>` | Output file. Defaults to the input with an `.svg` extension. |
| `-b, --background <COLOUR>` | Adds a background rectangle, e.g. `"#ffffff"`. |
| `-q, --quiet` | Silences the report on stderr. |

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
