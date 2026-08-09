# The curves mode

**Not built yet.** The web app shows the tab and the CLI will grow a `photo`
subcommand; neither has an engine behind it. This page explains why it is a
separate mode rather than a flag, and what it needs.

## Why it is not just an option

The pixel art mode assumes the image has a regular grid: it detects the period,
reduces the image to one logical pixel per cell, and traces axis-aligned
outlines. Every one of those steps is wrong for a photo.

So `img2svg` has two orthogonal axes, the same decomposition VTracer uses:

| Segmentation — image to regions | Fitting — contour to path |
| --- | --- |
| `grid` — the pixel art path above | `pixel` — the literal staircase, `h`/`v` |
| `cluster` — colour clustering, for photos | `polygon` — simplified straight segments |
| | `spline` — cubic Béziers |

They compose. The interesting one is **`grid` + `spline`**: vectorising a sprite
with smooth curves *after* recovering the grid, so the curves follow the art
pixels instead of the upscaling artefacts. Feed an 8× sprite to a general
vectoriser and it traces the staircase of the rescale; this would trace the
drawing.

## What is built so far

Two pieces of colour groundwork, both under the `photo` cargo feature so the
pixel-art wasm bundle can leave the photo code out:

**Oklab** (`color.rs`). Clustering needs a colour distance where one threshold
means the same thing everywhere, and the existing weighted-RGB distance is not
it: its weights are luminance coefficients, so what it really measures is how
much the *brightness* changed. On saturated colours that inverts the eye's
ordering — pushing a full channel of blue into saturated yellow scores 19.9 while
a dark blue visibly shifting hue scores 13.3, and Oklab puts the second nearly
six times further apart than the first. No single threshold works on the first
metric: the one that respects a sky's blues shatters every saturated surface, and
the one that does not shatter them smears the sky into a blob.

Worth being precise about what this does *not* buy, since it is easy to oversell:
down a greyscale ramp the RGB distance is already reasonable — sRGB's gamma is
itself roughly perceptual, and Oklab only corrects shadows against highlights by
about 50%. The win is in chroma, not in light. Separately, having lightness on
its own axis is what gradient banding will need: quantising the light while
leaving the hue alone cannot be expressed over three RGB channels.

**Channel quantisation** (`Rgba::quantize`). Run boundaries have to be stable, so
colours are cut down to `2^bits` levels per channel *before* clustering — two
pixels differing only in last-bit noise then land on the same value instead of
opening a boundary where there is no edge.

It rounds to the **nearest** level rather than keeping the top bits, which is the
usual shortcut and what VTracer's `--color-precision` does. Truncation always
rounds down, so every channel loses half a level on average — four values out of
255 at 5 bits — and the whole image comes out slightly duller. The bias is
invisible in any one colour and plain across an image, which is why the test for
it checks the mean over the ramp rather than individual values.

## What is missing

**Clustering.** Grouping a photo's colours into regions, filtering out specks,
and banding gradients — a vector format has no cheap per-region gradient, so a
smooth ramp has to become discrete layers on purpose.

**Bézier fitting.** Simplifying each contour, detecting the corners so they stay
sharp, and least-squares fitting curves to the rest.

**Seam handling.** Two regions sharing a border must be fitted *once*, not once
per face — otherwise the two fits disagree and a hairline of background shows
through between them. This is why the intermediate representation has to carry
half-edges rather than independent loops, and it has to be decided before the
fitters are written, not after.

## Status

Design and staged plan live in
[`SESSIONS/2026-08-09.img2svg-two-axes.md`](../SESSIONS/2026-08-09.img2svg-two-axes.md)
and
[`SESSIONS/2026-08-09.remaining-work.md`](../SESSIONS/2026-08-09.remaining-work.md).

In the web app, an image you load stays in memory in the worker, so once the
engine lands, switching to the tab will convert without reloading.
