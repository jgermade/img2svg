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
