# The curves mode

**Half of it ships; the curves do not.** Segmentation for photos is built, tested
and reachable from all three surfaces — `Segmentation::Cluster` in the library,
`img2svg photo` on the CLI, the Foto tab on the page — but every contour still
comes out as an axis-aligned staircase, because the only fitter that exists is
`pixel`. What is missing is the curve fitters. This page explains why it is a
separate mode rather than a flag, what is built, and what it still needs.

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

The whole segmentation half — colour groundwork, clustering, speck filtering,
boundaries and background removal — all behind the `photo` cargo feature, so a
pixel-art-only build can leave it out, and all of it reachable through
`Segmentation::Cluster`, `img2svg photo`, the Foto tab and four snapshots of its
own in `tests/photo.rs`. (The published page ships one bundle with both: the gate
costs 57 KB raw but only 17 KB brotli, which is not worth two builds.)

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

**Cluster segmentation** (`cluster.rs`). Three stages: quantise, build a palette
by grouping the distinct colours in Oklab from most frequent to least, then label
the connected components of equal palette entry.

Deciding the palette *before* walking the image is what buys the guarantee worth
having: **no pixel is painted further than `tolerance` from its own colour**. A
clusterer that instead merges neighbouring regions as it sweeps cannot promise
that — every merge moves the group's colour, and down a smooth ramp the chain of
merges swallows the whole sky, which ends up one flat region matching neither of
its ends. With the palette fixed up front the error is bounded by construction
and does not depend on where the sweep started.

The labelling works on **runs** — horizontal spans of equal palette entry — not
on pixels. A flood fill over four million pixels is millions of stack pushes with
no locality; merging the runs of two adjacent rows is a two-pointer walk with
union-find. Neighbourhood is 8-connected, matching the pixel-art path, which costs
nothing more than widening the overlap test by one column.

Measured on the three corpus images at 4.2 Mpx: **250–350 ms** each, against a
target of a couple of seconds in wasm.

Two things those measurements say about what comes next. At the default tolerance
a real image yields 18k–31k regions, and **68–77% of them are specks of 4 pixels
or fewer** — so `filter_speckle` is not a refinement, it is what makes the output
a usable SVG at all. And raising the tolerance does *not* reliably reduce the
region count: past about 0.08 it starts going back up (9,409 regions at 0.08
against 11,690 at 0.15 on one image), because with few palette entries the pixels
along a band boundary alternate between two distant representatives and shatter
into fragments. Fewer colours is not fewer regions. The speck filter is the
load-bearing part, not the threshold.

**Boundary extraction** (`boundary.rs`). Turns the labelled image into the
half-edge IR, with **every boundary extracted once** and both its regions
recorded — which is the whole point, since a shared border fitted twice is what
opens a hairline between two curved regions.

It works on the lattice of pixel corners. Each unit segment between two adjacent
corners — a *crack* — separates two pixels, and is a boundary when their labels
differ; the outside and transparency count as one more label, so the image border
falls out for free. Corners where three or four regions meet are *nodes*, and a
chain of cracks between two nodes is exactly one half-edge.

What makes that work is a small lemma: at a corner with exactly two boundary
cracks, both separate the *same* pair of regions, whether the boundary runs
straight through or turns. So a chain has one well-defined `(left, right)` along
its whole length, which is what lets it be fitted once for both faces. A
`debug_assert` re-checks it crack by crack rather than trusting the argument — it
has now held across roughly 285,000 chains of a noisy 1.4 Mpx image.

Measured end to end on the 4.2 Mpx corpus image: 340 ms clustering, 112 ms
boundaries, 30 ms to write the SVG.

**Speck filtering** (`speckle.rs`). Without it the output is not usable: a real
image leaves clustering with 12k–31k regions, and each one is a `<path>`.

Looking at a magnified conversion shows two different kinds of speck, and only one
of them is what everybody filters. Isolated dots, which an area threshold removes.
And **bands one pixel wide** running along every colour boundary — the antialiasing
fringe of the source — where a 1×8 band has eight pixels and the area threshold,
which is all `--filter-speckle` does in VTracer, never sees it. So there are two
criteria here, area and **thickness**, estimated as `2 × area / perimeter`: that
ratio stays near 0.5 for a band however long it is, while for a compact block of
side `s` it is `s/2` and grows with size. Unlike measuring the bounding box, it
does not care about orientation — a one-pixel-wide diagonal has a box as tall as
it is long.

Measured on one corpus image, and the second criterion is the one doing the work:

| filter | regions | colours | SVG |
| --- | --- | --- | --- |
| none | 12,498 | 142 | 549 KB |
| area ≤ 4 only | 4,157 | 117 | 246 KB |
| **default: area ≤ 4, thickness < 1** | **1,298** | 74 | 117 KB |
| area ≤ 8, thickness < 1.5 | 787 | 58 | 72 KB |

A speck merges into the neighbour it **shares the most border with**, not the
biggest one. A fringe band is by definition the edge of the region it fringes;
picking by size would send it to whatever large area barely touches it, and the
fringe would come back as a step of the wrong colour right on the contour.

It runs on the label image, before boundaries are extracted. Merging afterwards
would mean surgery on the IR — dissolving a half-edge and re-chaining the rings of
both faces, with the interesting case being a speck whose removal joins two of its
neighbour's rings into one. Doing it earlier is the same result without any of
that.

One thing it deliberately does not do: a speck with no visible neighbour stays.
That is a lone dot on transparency, and the alternative is punching a hole where
there was drawing.

**Gradient banding, a colour cap, and a fixed palette.** A vector format has no
cheap per-region gradient, so a smooth ramp has to come out as steps; what
`gradient_step` chooses is how wide those steps are. Raising `tolerance` would
widen them too, but it would also merge distinct hues that happen to be close —
this widens **only along the lightness axis** and leaves hue where it was, which
is what having lightness on its own axis was for and cannot be expressed over
three RGB channels.

Measured, with the honest caveat first: it does what it says, and on shaded
artwork what it says is not what you want. At 0.05 it merges a spike's dark
shading into the body — the modelling flattens — and at 0.15 mottling appears
along band boundaries, the same fragmentation that raising the tolerance causes.
It is the tool for a smooth sky, not for a drawing with volume, which is why the
default is 0.

| setting | colours | regions | SVG |
| --- | --- | --- | --- |
| default | 74 | 1,298 | 117 KB |
| `gradient_step` 0.05 | 59 | 1,218 | 114 KB |
| `gradient_step` 0.15 | 31 | 787 | 112 KB |
| `max_colors` 16 | 16 | 1,043 | 103 KB |

Note the last row against the third: 16 colours give *more* regions than 31 do.
Fewer colours is not fewer regions — the same lesson the tolerance sweep taught.

`max_colors` caps the palette; the colours that do not fit go to the nearest entry
with no distance limit, and since the grouping runs in frequency order the entries
that survive are the most present ones. A non-empty `palette` is exactly the
palette: nothing is added and every colour goes to its nearest entry. Both drop
the "within `tolerance`" guarantee by construction, which is the point of asking
for them.

**Background removal.** `remove_background` empties the flat background and crops
to what is left, as the pixel-art path already did. On a label image it needs no
flood fill: the regions *are* connected blocks of one colour, so "what comes in
from outside" is exactly "the regions touching the border", and the same colour
enclosed inside the drawing stays because it is a different region. On the corpus
image the white background goes and the canvas crops from 992×1079 to 662×1079.

Specks are filtered *before* the background is removed, and the order matters: a
speck sitting on the background merges into it and disappears with it. The other
way round it would be left surrounded by transparency, with no neighbour to merge
into, and would survive as a dot floating in the void.

One correction to the plan, which asked for a colour *tolerance* here on the
grounds that exact comparison "never matches on a photo": it is not needed. Both
paths unify near-equal colours before this runs — `reduce_palette` on the grid
side, the palette on the clustering side — so by the time the background is looked
for it is one exact colour. Comparing by equality is not an inherited limitation;
the work is already done upstream.

## What is missing

One prediction in the plan turns out not to hold, and it is worth correcting
rather than repeating: unfiltered specks were supposed to make the SVG *bigger
than the PNG*. Measured, it comes out at 0.2–0.6× the PNG — but only because
these particular PNGs are pathologically noisy pixel art with 64k–159k distinct
colours, so they compress badly. The case for the speck filter stood on path
count, not on file size, and that is how it was judged.

**Bézier fitting.** Simplifying each contour, detecting the corners so they stay
sharp, and least-squares fitting curves to the rest. Until it lands, `photo`
output is a staircase: correct, and three to four times the path data it needs.

Two things the fitters have to touch on the way. `shape-rendering="crispEdges"`
is hardcoded in the `<svg>` element, which is right for a pixel staircase on
integer coordinates and would alias every Bézier — it has to become conditional
on the fit. And tangent continuity across a node is not guaranteed: each chain is
fitted independently, so two chains meeting at a node agree on the point but not
on the direction. At a three-region junction that is correct, it is a real
corner; where it will show is a node that is geometrically smooth.

**Real progress on the page.** The photo path is loops over rows and regions, so
unlike the sprite path it can report actual progress through a callback instead
of the indeterminate bar it shows today.

**Seam handling.** Two regions sharing a border must be fitted *once*, not once
per face — otherwise the two fits disagree and a hairline of background shows
through between them. This is why the intermediate representation has to carry
half-edges rather than independent loops, and it has to be decided before the
fitters are written, not after.

## Status

What is left, and in what order, lives in the newest file in
[`SESSIONS/`](../SESSIONS/) — currently
[`2026-08-10-01h40.remaining-after-wiring.md`](../SESSIONS/2026-08-10-01h40.remaining-after-wiring.md).
The original decision is in
[`2026-08-09-10h00.img2svg-two-axes.md`](../SESSIONS/2026-08-09-10h00.img2svg-two-axes.md);
the files between the two record how each stage was reasoned about, including
where it corrected the plan, and are not kept up to date on purpose.

In the web app, an image you load stays in memory in the worker, so switching
tabs reconverts it without reloading anything.
