# The illustration mode

**Both axes ship.** Segmentation for images that sit on no grid is built, tested
and reachable from all three surfaces — `Segmentation::Cluster` in the library,
`img2svg illustration` on the CLI, the Ilustración tab on the page — and so are
all three fitters: `pixel`, `polygon` and `spline`. This page explains why it is
a separate axis rather than a flag, and what each part is for.

The mode was called `photo` until 2026-08-11, and the name was wrong about what
it is for: the target is a **web illustration** — few colours, few curves, crisp
edges where the source has them. `photo` still works as an alias on the CLI, as
a cargo feature and as a URL hash. See
[`SESSIONS/2026-08-11_22h08`](../SESSIONS/2026-08-11_22h08.illustration-and-the-working-scale.md).

## Why it is not just an option

The pixel art mode assumes the image has a regular grid: it detects the period,
reduces the image to one logical pixel per cell, and traces axis-aligned
outlines. Every one of those steps is wrong for an image that has no grid.

So `img2svg` has two orthogonal axes, the same decomposition VTracer uses:

| Segmentation — image to regions | Fitting — contour to path |
| --- | --- |
| `grid` — the pixel art path above | `pixel` — the literal staircase, `h`/`v` |
| `cluster` — colour clustering, off the grid | `polygon` — simplified straight segments |
| | `spline` — cubic Béziers |

They compose. The interesting one is **`grid` + `spline`**: vectorising a sprite
with smooth curves *after* recovering the grid, so the curves follow the art
pixels instead of the upscaling artefacts. Feed an 8× sprite to a general
vectoriser and it traces the staircase of the rescale; this would trace the
drawing.

## What is built so far

The whole segmentation half — colour groundwork, clustering, speck filtering,
boundaries and background removal — all behind the `illustration` cargo feature,
so a pixel-art-only build can leave it out, and all of it reachable through
`Segmentation::Cluster`, `img2svg illustration`, the Ilustración tab and four
snapshots of its own in `tests/illustration.rs`. (The published page ships one bundle with both: the gate
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

That bound belongs to the palette with everything else off, and the stages that
**merge** are the ones that spend it, each at a stated price: `min_color_share`
up to `cluster::SNAP_CEILING` × the tolerance, and of that at most
`cluster::SNAP_HUE` × it in hue; regularisation up to
`smooth::CEILING` × it or as far as the pixel already was, and only towards a
colour already painted next to it; speck filtering with no bound at all, since a
speck leaves with whatever neighbour it merges into. The first two compose
without loosening — 4× stays 4× with both on — so **at the defaults no pixel is
painted further than `SNAP_CEILING × tolerance` from its colour, nor further than
`SNAP_HUE × tolerance` from it in hue**. Turn all of
them off and the narrow guarantee reads exactly as written above.
`tests/cluster.rs` checks both.

**Earning a palette entry** (`min_color_share`). Grouping walks the distinct
colours most-frequent-first, but frequency only *orders* the seeding — it never
gates it, so a colour occurring thirty times in the whole image founds an entry as
readily as the background. On a JPEG album cover that meant 65 entries of which
**42 painted under 0.2% each and 1.45% between them**: the ringing around the
black strokes, one entry per step.

Counting pixels is not enough on its own, because it cannot tell forty pixels of
ringing scattered along the edges from a forty-pixel red mole. What separates them
is how much error the entry *saves*: ringing sits next to a colour that already
exists and saves almost nothing, a mole is far from everything and saves a lot. So
an entry earns its place when `pixels × distance to the nearest entry ≥
min_color_share × visible pixels × tolerance` — the new entry has to remove at
least as much error as having that fraction of the image off by one tolerance.

That still needs a ceiling, or it has no bound: the criterion is pixels × distance,
so a colour with few pixels can be asked for an arbitrary distance, and on a 5 Mpx
image a saturated 30×30 mole would lose its colour. Past `SNAP_CEILING` × the
tolerance a colour always founds an entry however rare it is. Where to put it,
measured in palette entries:

| ceiling | cover.jpg | Sonic1.png |
| --- | --- | --- |
| 2× | 30 | 37 |
| 3× | 20 | 24 |
| **4×** | **20** | **21** |
| none | 20 | 18 |

One ceiling on the total distance is not enough, and the missing half is
`SNAP_HUE`: a colour absorbed 4× away can be 4× away *in hue*, which means being
painted a different colour rather than a worse shade of the same one. Measured on
the album cover, that was its most visible defect. The edge of a white letter over
the green panel is a six-pixel ramp — twelve at the working scale, since that image
is upscaled ×2 — whose middle pixels are clean mixtures of the two sides; none of
them earns an entry, and the nearest entry to `(186,209,183)` is not a pale green,
which the palette does not have, but a **skin tone** 1.4 tolerances away. So the
whole ramp was painted beige and every letter came out with an ochre fringe that is
not in the image, along with blue flecks in dark hair from the same mechanism.

So absorbing can cost lightness — up to `SNAP_CEILING` — and cannot cost hue, past
`SNAP_HUE` × the tolerance. What the shortcut existed to swallow still gets
swallowed, because ringing around a black stroke shares its hue with the black that
absorbs it. It is the mirror of `gradient_step`, which grants slack in lightness on
purpose and none in hue; both come from having the two axes apart, which is what
Oklab is for.

| hue ceiling | cover.jpg | Sonic1.png |
| --- | --- | --- |
| 1× | 30 colours, 461 paths, 94.1 KB | 25, 104, 30.9 KB |
| 1.5× | 23, 441, 91.5 KB | 20, 92, 30.2 KB |
| **2×** | **18, 438, 84.5 KB** | **20, 95, 30.0 KB** |
| 3× | 16, 440, 84.3 KB | 20, 98, 31.0 KB |
| none | 16, 440, 84.3 KB | 19, 95, 30.3 KB |

At 3× the fringe is back, so the ceiling has to bite; at 1× it bites into colours
that are doing work. At **2×** the fringe and the flecks are gone for two palette
entries and 0.2 KB — the two entries being exactly the pale green and the dark olive
that the ramps needed. The synthetic sky, which is a hue ramp from blue to olive and
therefore the case most exposed to this, comes out with the same stops and the same
geometry.

Entries against the share, over three images with nothing in common — a JPEG
illustration drawn in strokes, a 5 Mpx scanned airbrush, and an upscaled sprite:

| image | off | 0.001 | **0.002** | 0.005 |
| --- | --- | --- | --- | --- |
| cover.jpg | 68 | 27 | **20** | 12 |
| Sonic1.png | 86 | 24 | **18** | 16 |
| sprite | 80 | 18 | **16** | 13 |

All three agree on the same place, which is what makes it a default rather than a
setting: past it the palette has entries that paint nothing, before it nothing has
started to go missing.

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

**Spatial regularisation** (`smooth.rs`). The palette decides its entries by
looking at the whole image and then assigns **every pixel independently** to the
nearest one — nothing in that chain knows a pixel has neighbours. Measured on a
scanned airbrush illustration, over 24×24 windows the eye reads as one flat
colour, one pixel in ten sits further from its own window's mean than the whole
tolerance (p90 0.032, max 0.071, against 0.045). So adjacent pixels of a flat
area land on different entries, and connected components turns that flicker into
islands: 12,954 regions, 8,911 of them under 16 px, painting 6% of the canvas and
making up 97% of the paths.

The obvious fix — each pixel takes the commonest label around it — removes the
noise and takes every thin stroke with it: a one-pixel line is outvoted three to
six inside its own 3×3, however black it is on however white a background. What a
vote is missing is the other half of the question, how much the pixel *resembles*
the colour being proposed. So each pixel minimises

```text
    cost(c) = distance(pixel colour, c) + beta × neighbours that are not c
```

which is one ICM step over a Markov field, iterated a few times out of place. A
grain pixel sits halfway between two entries — that is why it flipped — so
coherence decides it; a pixel of black line is 0.9 away from the background entry,
which no majority can buy. Two guards keep it honest: a pixel may only move to an
entry **already present in its own neighbourhood**, and only if that leaves it
agreeing with *more* neighbours than it does now. Without the second, the colour
term breaks ties on straight boundaries and serrates them.

On the same illustration, subpaths against passes:

| passes | regions | subpaths | SVG |
| --- | --- | --- | --- |
| 0 | 12,954 | 41,548 | 2,468 KB |
| 1 | 12,148 | 22,267 | 1,754 KB |
| **2** | **10,151** | **17,003** | **1,468 KB** |
| 4 | 8,200 | 12,971 | 1,246 KB |

It pays for itself: at two passes the whole conversion runs 0.79 s against 0.84 s
with it off, because the boundary and document stages have that much less to do.

**What did not need retuning.** With regularisation and the palette floor in
place, the other three illustration defaults were re-measured and all stayed:

- `min_thickness: 1.0` still earns its keep — dropping it to `0.5` on the
  illustration takes paths from 551 back up to 921. Regularisation removes
  *flicker*, but a one-pixel antialiasing band along an edge is a coherent
  structure whose pixels agree with each other, so it survives the criterion by
  design and this is still what removes it.
- `filter_speckle: 4` is right for small images and only for those. At 16 the
  glasses on a 300×300 cover lose their rims, while a 5 Mpx painting takes 16 or
  32 with nothing visible lost and 2.7× fewer paths. The threshold is an absolute
  pixel count and so does not scale with the canvas — worth turning into a
  fraction the way `min_color_share` is, but that is an API change and not this
  session's.
- `tolerance: 0.045` sits where it did. The palette floor took over the job of
  keeping the entry count down, so raising the tolerance is now purely a
  fidelity–flatness choice rather than a way to fight the entry count.

**Speck filtering** (`speckle.rs`). Regularisation flattens the flicker but not
compact blobs — a blob's interior has no neighbours to disagree with, so it is a
local minimum of the criterion and only erodes a ring per pass. That is what this
is for, and the two compose: at two passes plus `--filter-speckle 32` the same
image goes to 1,974 regions and 3,397 subpaths, where the filter alone — at 64,
twice as hard — left 1,881 regions carrying 14,405 subpaths behind.

Without it the output is not usable: a real
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

**Gradients as gradients** (`ramp.rs`). The paragraph above says a vector format
has no cheap per-region gradient, so a ramp has to come out as steps. That is true
per region and false per document: SVG does have `<linearGradient>`, and a group
of bands that one linear gradient reproduces can be merged into a single shape
painted with it. The band boundaries then disappear, and they are the raggedest
contours in the document, since they trace where a smooth ramp happened to cross a
quantisation threshold.

The criterion is exactly what the element can express — colour as a function of
the projection of position onto one axis — plus one condition that is not
obvious and is doing most of the work:

> A group of neighbouring regions is a ramp when one linear gradient reproduces
> every one of their colours to within `ramp::CEILING` tolerances **and within a
> `ramp::GAIN`-th of the colour range it spans**.

Without the second half, a flat colour would already qualify: it reproduces any
group to within the range the group spans, so a gradient that does not beat that
by a good factor is explaining nothing. On an album cover, the six near-black
entries that JPEG ringing leaves around a stroke all sit **inside the ceiling**,
so any axis "explains" them, and 24 gradients came out of pure noise — two of them
smearing a face into a diagonal that does not exist.

One stop per **colour**, not per band. Per band there would be one parameter per
data point: the gradient would hit each band's centre by construction and a speck
is short along every axis, so the test could not fail. That is the difference
between fitting and interpolating, and only the first can be checked. Per colour,
a ramp can still bend through Oklab freely — which two stops could not — and the
error still means something: within a band the gradient runs from the midpoint
with one neighbour to the midpoint with the other, so the worst it strays from the
flat colour is half a step. The ceiling therefore limits the step *between*
adjacent bands, not the width of a band, and a flag of hard stripes is rejected
while a smooth ramp passes.

The error is measured against what a browser actually draws: SVG interpolates
stops in **sRGB**, so the model is the sRGB interpolation and only the distance is
taken in Oklab. Checking a curve that is straight in Oklab would be checking a
gradient nobody is going to render.

Merging costs nothing in machinery. Every half-edge already knows the region on
each side, so the union's outline is the edges with exactly one side inside the
group, and `boundary::rings` — which already turns a set of oriented edge uses
into closed rings — does the rest. No polygon clipping anywhere. This is the third
thing the half-edge IR has paid for.

Where it pays, measured on a 900×600 sky with photographic grain:

| | colours | paths | anchors | SVG |
| --- | --- | --- | --- | --- |
| bands | 9 | 121 | 13,799 | 70.6 KB |
| one gradient | 9 | **3** | **1,854** | **5.9 KB** |

And where it does not: on flat-colour artwork it finds almost nothing, which is
correct, and then it costs — 17 small gradients and 2 KB on an album cover, and
10% of the conversion time. The two images that started this work are both flat
artwork, so on them this is roughly a wash; the case it is for had to be built to
be measured, which is worth saying plainly.

It also pulls against `min_color_share`. That option drops palette entries that
paint little, the middle of a ramp paints little, and what survives are steps
larger than a gradient can reproduce. Turning it off on a 5 Mpx drawing goes from
24 gradients to 98 — and still gives a bigger document, because the finer palette
costs more than the gradients save. Each wins in its own place and you cannot have
both.

**Soft seams, and the second gradient model** (`softness.rs`). The criterion above
needs three palette colours before it calls a group a ramp, and the reason is
sound: any two neighbouring regions can be split by some tendered gradient — the
model lands halfway between them and each is off by half — and that is not a
gradient, it is averaging, which is what `tolerance` already does.

But the terminator of a round surface shaded with an airbrush is usually *two*
tones, and it was coming out with a hard crescent across the middle of a volume —
a belly, a muzzle. Two colours meeting is not enough to tell a shading from an
edge; what tells them apart is what the original painted *between* them:

> **Softness** is how many pixels a boundary takes to go from one face's colour to
> the other's, counted along its normal.

Measuring it needs no edge detector, and a generic one would in fact get it wrong.
A detector looks at the image without knowing what each boundary separates, so all
it can ask is *where colour changes* — and crossing a two-pixel stroke over skin
the colour moves for six or eight pixels straight (skin, inside the stroke, outside
the stroke), so a straight scan reports one wide transition where there are two
hard edges back to back. Measured that way, an album cover — flat poster art with
black linework — comes out soft on 58% of its boundaries, which is the opposite of
the truth.

Here it is well posed, because by this stage **both colours are already decided**:
a boundary separates region `A` from region `B` and their palette entries are known
before anything is measured. The question becomes *how many pixels along this
normal are a mixture of these two particular colours*, which is the same projection
`speckle` uses to tell a fringe from a stroke. Third time that projection has paid.

The threshold came from looking at the distribution on three drawings that have
nothing in common, weighted by boundary length:

| softness | `Sonic1.png` | `cover.jpg` | a synthetic sky |
| --- | --- | --- | --- |
| 0–1 px | 28% | 19% | 34% |
| 2–4 px | 20% | **72%** | 0% |
| 5–8 px | 24% | 2% | 0% |
| 9–16 px | 20% | 1% | **64%** |

The poster is flat art and **91% of its boundaries stay under 5**, which is what
has to happen. The sky is a pure ramp and saturates the measure's reach. The drawing
with volume has both, with a valley right at 5: its ink edges measure 0 and 1, and
the shading seams of the belly and the muzzle, 7 and 15. So `softness::SOFT = 5`
working pixels — not "one pixel", which is what the criterion said before it was
measured: an upscaled JPEG has no one-pixel edges and is still hard.

A soft seam does two things. It lets a **pair** be a ramp, which is the only way
past the three-colour rule and one opened by a property of the image rather than of
the fit. And it is the gate to the **radial** model: a gradient is colour as a
function of a height, and the height can be the projection onto an axis or the
distance to a centre. A cliff of light on a wall is the first; the terminator of a
sphere is the second, and forcing it through an axis smears it along a direction the
drawing does not have.

Which model wins is decided by the error of each, with ties going to the axis
(`ramp::PREFER`) — a `<linearGradient>` is what an editor and a human expect of
tendered shading, and a seam with a 300-pixel radius of curvature is explained just
as well by both. The centre of a radial comes from the **curvature of the seam** and
not from the colours: the level set of a radial gradient is a circle, so the seam
between two of its bands is an arc, and an algebraic circle fit gives the centre
directly. Taking it from the centroid of the group's lightest colour — the first
attempt — fails as soon as the group does not reach the centre, because the centroid
of an arc sits on the arc.

Two things about that model have to be exact, and both were wrong first:

- **The extent of a band** cannot be bounded from outside. The bands of a radial
  gradient are annuli, and any convex hull of an annulus contains its centre, so
  the minimum comes out 0 instead of the inner radius, the gradient is evaluated at
  the centre colour, and every band's error explodes. With the bounding box, a disc
  shaded from a point inside it produced **zero** gradients.
- **A stop goes at the mean height over the pixels**, not at the height of the
  centroid. For a distance the two differ as much as the shape wants: for a full
  annulus the centroid *is* the centre, so the inner colour's stop lands on top of
  the outer one's. For an axis they are the same thing, because a projection is
  linear.

Both mean measuring over pixels, one pass per centre — and that has to be paid for
carefully. Measuring whole images cost 46 passes on the airbrush drawing and 33 on
the cover, or 20 and 40 ms of the 130 and 90 the conversions take. Grouping the
pixels by band once and measuring a band the first time it is asked about brings it
back to nothing: a group with a soft seam is two or three bands out of four hundred.
On flat artwork the index is never even built.

What it buys, measured on the airbrush drawing: the belly and the muzzle fall off
smoothly instead of breaking at a crescent, and the document goes from 137 paths and
34.6 KB to **95 and 30.3 KB**. The synthetic sky still comes out as one *linear*
gradient, and the cover keeps its four flat faces — the model choice does not
over-fire.

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

**Polygon fitting** (`fit.rs`). Ramer–Douglas–Peucker over each contour, keeping
only the vertices that draw something. It stays in integers — RDP *selects*
vertices from the polyline it is given, and those are lattice corners, so nothing
new is invented and `Point` does not change. Floats and coordinate quantisation
belong to the spline.

The tolerance is a maximum deviation in pixels, and 0.707 is the number that
governs it: that is how far the step of a 45° staircase sits from its own chord,
so below it nothing straightens and `0` reproduces the pixel fitter byte for
byte. On one corpus image, against 30,231 vertices and 117 KB unfitted:

| tolerance | vertices | SVG |
| --- | --- | --- |
| **0.75 (default)** | **19,261** | **103 KB** |
| 1.0 | 12,036 | 85 KB |
| 1.5 | 10,943 | 82 KB |
| 3.0 | 9,059 | 76 KB |

Note where the knee is, and what it costs. Most of the win is between 0.75 and
1.0, and it is not free: a shallow staircase — three across, one down — sits 0.95
from its chord, and a genuine one-pixel bump on a straight edge sits 1.0. **RDP
cannot tell them apart, because to it they are the same feature.** There is no
tolerance that removes the artefact and keeps the detail; only context could
separate them, and RDP does not see context. Hence the conservative default: the
smallest value that does anything at all.

Being precise about what the tolerance promises matters here, because the
obvious reading is wrong. It does *not* mean a feature taller than the tolerance
survives — RDP measures against whatever chord the recursion currently holds, not
against the vertex's neighbours, so a chord coming from far away swallows a bump
that on its own would sit 1.0 away. What it promises is the ceiling: no point of
the contour ends up further than the tolerance from what gets drawn. That is what
the test asserts.

Two things it dragged in on the way, both from the plan's list for the spline,
and one of them earlier than expected. `shape-rendering="crispEdges"` is now
conditional on the fit: it is right for a staircase on integer coordinates and it
would leave every oblique segment jagged — the exact staircase the fit had just
removed. That was filed under the spline, but the polygon needs it too, because
what breaks it is an oblique segment, not a curve. And the path writer, which
only knew `h`/`v`, now emits `l` when a segment is neither.

**The fit happens per half-edge, and the rings are assembled afterwards.** This
is the seam handling below, and it was nearly lost at the last step. The fit used
to run on the assembled ring, which with `pixel` on integer coordinates is
invisible — collapsing collinear runs on either side of a node gives the same
staircase — so nothing had caught it. RDP over an assembled ring is not
invisible: it simplifies *through* the bifurcations, treating a node where three
regions meet as an ordinary interior vertex and dropping it for looking straight.
The two faces of a shared boundary would then be simplified inside two different
rings, with different neighbours beyond the node, and come out disagreeing — the
hairline the whole IR exists to prevent. The IR kept its promise and the last
stage threw it away.

So `fit::Fitted` fits every `EdgeId` once and only then chains rings out of
fitted edges. One step still looks at the whole ring: dropping vertices that lie
*exactly* on the line between their neighbours, which is what a node the boundary
runs straight through leaves behind. That is not fitting, it is declining to
write a point that draws nothing — the line is identical either way, so the two
faces still agree even if one drops it and the other keeps it. `tests/fit.rs`
checks the property directly on the emitted document: every interior segment is
drawn by exactly two faces, with the same endpoints.

One correction to the plan, which asked for a colour *tolerance* here on the
grounds that exact comparison "never matches on a photo": it is not needed. Both
paths unify near-equal colours before this runs — `reduce_palette` on the grid
side, the palette on the clustering side — so by the time the background is looked
for it is one exact colour. Comparing by equality is not an inherited limitation;
the work is already done upstream.

**Bézier fitting** (`fit/spline.rs`). Schneider's method: split the contour at
its corners, chord-length parameterise each piece, least-squares fit a cubic with
the end tangents held fixed, reparameterise, and subdivide at the worst point
while it still misses. Straight runs come back out as lines — a cubic costs six
numbers and a line two, and the canvas border has no business being a curve.

**It does not make files smaller, and it was expected to.** Measured against the
polygon at the same tolerance it is bigger on every input tried, including the
ones it should suit:

| | polygon | spline |
| --- | --- | --- |
| corpus photo, tol 1.5 | 84.6 KB | 103 KB |
| corpus sprite via `pixelart`, tol 1.5 | 16.8 KB | 18.7 KB |
| three flat overlapping discs, tol 1.5 | 1,033 B | 1,281 B |

That is not a defect to be tuned away, it is arithmetic: a cubic replaces a
handful of segments but costs three times the numbers each. What the spline buys
is that **the outline stays smooth however far you zoom**, where a polygon's
facets are baked into the file at the tolerance you picked. That is the reason to
choose it, and the only one.

At their *own* defaults the two land in the same place — 105.6 KB for `polygon`
at 0.75 against 103.2 KB for `spline` at 1.5 on the corpus photo — but that is
the larger tolerance paying for the extra numbers, not the curves being cheaper.
Compare like for like and the row above is what happens.

**It needs a larger tolerance than the polygon, and gets its own default: 1.5.**
The contour it starts from is a lattice staircase, so it arrives with its own
error already in it — a step sits up to 0.707 from the smooth shape it stands
for. The polygon never notices, because its vertices *are* lattice corners and at
tolerance 0 it reproduces the staircase exactly. A curve has to pass near all
those steps at once, and below 1.0 it chases them: it subdivides, fails again,
and ends up emitting line after line. On a digitised circle of radius 30:

| tolerance | segments | SVG |
| --- | --- | --- |
| 0.75 | 2 curves + 108 lines | 618 B |
| 1.0 | 6 + 36 | 472 B |
| 1.25 | 14 + 12 | 568 B |
| **1.5 (default)** | **4 + 4** | **290 B** |
| 3.0 | 4 + 4 | 290 B |

`--fit-tolerance` therefore has no fixed default any more: it takes the one
belonging to the fitter you named, and the page resets the slider when you switch
between them.

**Corners stay sharp**, which is what separates a usable curve fitter from one
that turns every drawing into a blob. A vertex is a corner when the contour turns
more than 60° across a four-pixel window: a rectangle's right angle reads as 90°,
a 45° staircase reads as 0°, and there is room to spare between them.

**Where the plan said the kink would be, and where it actually is.** Three
successive notes carried the same worry — that tangents are not shared across a
node, so a node that is geometrically smooth would show a kink. It is nearly a
non-issue: a node is a lattice corner where *three or four* regions meet, which
is a genuine corner essentially every time.

The real case was two lines further down in `boundary.rs`. A loop that passes
through no node — an isolated region on a uniform background — is closed, and it
gets split *wherever the sweep happened to reach it first*. That is an arbitrary
point on the smoothest contour in the picture. Treat it as a corner and every
floating blob in every photo gets one kink, in a different place each time the
segmentation shifts by a pixel. So a closed chain is fitted as periodic: corners
are searched with wraparound, and if the seam is not one of them its tangent is
estimated from the points on *both* sides, so the curve leaves in the direction
it arrived. `tests/fit.rs` measures the turn at every joint of a digitised
circle: 0.00 as it stands, 0.60 with the seam treated as a corner.

**The seam survives curves**, which needed one thing to be exact. A fitted chain
is a list of vertices carrying the control point arriving at each and the one
leaving it; reversing it reverses the list and swaps those two per vertex — the
same four numbers in the other order, so the two faces of a shared boundary get
identical geometry, not merely identical endpoints. The seam test compares whole
curves, controls included, because two cubics that start and end together can
still bulge apart, and the gap between them would show the background exactly as
two disagreeing polylines would.

**Real progress on the page.** The illustration path now reports how far along it is
through a callback, and the page draws a real bar instead of a pulsing one. The
weights are measured, not guessed — on a 2730×1536 corpus image, of 471 ms: 148
palette, 172 labelling, 46 specks, 79 boundaries, 26 document. Two thirds of the
time is two passes over the image, and those report per row; the rest reports per
stage. The sprite path reports nothing but completion, because after the first
step it is working on a drawing a few dozen pixels across.

## The working scale, which comes before everything else

Added 2026-08-12, and it reframes every constant on this page.

Each simplification constant in this mode is an **absolute pixel count** — a speck
area, a thickness, a fit deviation — and the lattice they are compared against is
the source's own. What those numbers mean therefore depends on how many pixels the
image spends on a feature, and two images never agree about that:

| image | a feature measures | its grain measures | so the constants are |
| --- | --- | --- | --- |
| a 300×300 album cover | ~2 px | ~1 px | the size of the drawing |
| a 1800×2823 airbrush scan | ~200 px | ~2 px | the size of the noise |

On the first there is no lattice for a curve to live on; on the second nothing is
simplified at all. That single misfit explains everything this page and the
sessions had been fighting: why upscaling the cover 4× fixed it, why sub-pixel
contours helped it and did nothing for the scan, why `filter_speckle: 4` "does not
scale with the canvas", and why the tolerance ladder had no rung to stand on — the
rungs are at 0.5 and √2/2 **absolute pixels**, so they stop biting as soon as the
lattice is fine compared to the tolerance.

So the mode now picks a working resolution before it looks at a pixel. The knob is
not the resolution but the thing one actually wants: `simplify`, the smallest
feature that survives, in per mille of the long side. From it, the image is
resampled so that feature lands on `resample::FEATURE` = 3 working pixels, and
every other constant is defined at that scale — `filter_speckle` is `FEATURE²`,
`min_thickness` is `FEATURE`, and the polygon tolerance went back to the shared
0.75 because the rungs no longer reach it.

The consequence worth writing down: since the feature is asked for as a fraction of
the image, **the working canvas depends only on `simplify` and not on the size of
the file**. A thumbnail and a 5 Mpx scan with the same `simplify` are segmented at
the same size, which is what makes one number mean the same thing in both.

Measured on the two images in `examples/results-to-improve/`, everything else
unchanged at the time:

| | colours | paths | SVG |
| --- | --- | --- | --- |
| `cover.jpg` on its own lattice | 20 | 501 | 77.3 KB |
| the same at ×2 | 16 | 318–450 | 60–94 KB, and the strokes are whole |
| `Sonic1.png` on its own lattice | 21 | 6,895 | 1,870 KB |
| the same at 383×600 | 19 | 137 | 35 KB |

Resampling premultiplies alpha, filters separably, and picks its kernel by
direction: a triangle of radius `1/scale` going down, which is an area average and
is what removes grain, and Catmull-Rom going up, which reconstructs the edge the
source's antialiasing wrote inside the pixel instead of leaving it blunt. Upscaling
is capped at 4×: past that there is no edge left to recover.

### Two stages that the working scale made possible

**Ink is not fringe** (`speckle.rs`). A thickness threshold removes antialiasing
fringes, and it removes ink strokes too, because on a small drawing they measure
the same. What separates them is colour: a fringe's entry sits on the segment
between its two neighbours' entries — it is a mixture of them — and a stroke's does
not. Thickness now only nominates; the mixture test decides, and the ceiling is
three tolerances because the fringe's own entry, both endpoints and `min_color_share`
each carry error. At one tolerance the halo survives; at five, palette entries start
being merged. A stroke with a single neighbour needs no special case: the segment
degenerates to a point and black ink is nowhere near skin.

**Filing off the wobble** (`wobble.rs`). The simplifier undoes *regular*
staircases, but a real drawing's are irregular — a nearly straight edge alternates
two- and three-pixel steps following the source's noise — and that alternation
leaves the chord by more than the tolerance, so the simplifier is obliged to write
it. At 6× that reads as a contour trembling pixel by pixel, which is what "it looks
pixelated" was about. No tolerance fixes it, because the wobble and the drawing
measure the same; what fixes it is *moving* vertices rather than choosing which
ones stay. A binomial pass with a hard cap does it, and corners are recognised at
scale — the turn of an arc is spread over many vertices and a corner's is
concentrated in one — so they stay exactly where they were. It lives in the vertex
displacements, next to `subpixel`'s, so `Fit::Pixel` ignores it for the same
reason. On the cover: 94.4 KB → 84.3 KB with the same 450 paths.

That last stage is also what finally made the spline fitter competitive, though
still not the default: with a smooth contour it beats the polygon by 16% of the
anchors on the airbrush scan and still loses by 14% on the cover, which is corners
rather than arcs. The rule of thumb from two sessions ago survives: `spline` is for
smooth drawings, `polygon` for everything else.

## What the plan got wrong along the way

One prediction turns out not to hold, and it is worth correcting rather than
repeating: unfiltered specks were supposed to make the SVG *bigger than the PNG*.
Measured, it comes out at 0.2–0.6× the PNG — but only because these particular
PNGs are pathologically noisy pixel art with 64k–159k distinct colours, so they
compress badly. The case for the speck filter stood on path count, not on file
size, and that is how it was judged.

**Seam handling** turned out to be a matter of *when* as much as of *what*: the
half-edge IR was the right type from the start, and the fitting stage was still
about to undo it by simplifying assembled rings. See the polygon fitter above.

## Status

What is left, and in what order, lives in the newest file in
[`SESSIONS/`](../SESSIONS/) — currently
[`2026-08-11_22h08.illustration-and-the-working-scale.md`](../SESSIONS/2026-08-11_22h08.illustration-and-the-working-scale.md).
The original decision is in
[`2026-08-09-10h00.img2svg-two-axes.md`](../SESSIONS/2026-08-09-10h00.img2svg-two-axes.md);
the files between the two record how each stage was reasoned about, including
where it corrected the plan, and are not kept up to date on purpose.

In the web app, an image you load stays in memory in the worker, so switching
tabs reconverts it without reloading anything.
