# How the pixel art mode works

Six stages, each in its own module.

## 1. Transparency checkerboard — [`src/checker.rs`](../src/checker.rs)

Screenshot an editor and the white/grey checkerboard behind the artwork gets
baked in as opaque pixels. The most frequent grey pairs are collected and, for
each, the runs of solid colour are measured: a real grid makes them all the same
length and all starting in the same phase. The pair covering the most image wins.

Only cells that match all the way through **and whose neighbours alternate** are
cleared. A flat white area of the artwork — a character's eye, say — also fits
the light cells perfectly, but its neighbours do not alternate. From the cells
that do qualify, the erasure spreads by contiguity.

This runs first, before grid detection, because the checkerboard is itself a
regular grid and a very well-marked one; left in place it would capture the
detector.

## 2. Grid detection — [`src/grid.rs`](../src/grid.rs)

Pixel art almost never arrives at 1:1: one drawn pixel covers NxN real ones.
Colour changes therefore land on a regular grid, which makes the image gradient a
periodic signal.

For each candidate period, the share of gradient energy concentrated at that
frequency is measured — `|Σ g(x)·e^(-2πix/c)| / Σ g(x)`, between 0 and 1 — and
the largest well-scoring period wins. Its divisors describe the same grid, just
subdivided, so taking the largest picks the fundamental rather than a harmonic.
The phase of that same sum gives the offset directly.

The period need not be a whole number, so images rescaled by an arbitrary factor
work too. Below a confidence floor the image is assumed to be 1:1 already.

A caveat worth knowing: a drawing with a strongly repeating motif can score a
false period. That is a real property of the method, not a bug — pin `--scale` if
it happens.

## 3. Downscaling

Only the middle of each cell is sampled, dodging the antialiasing along the
edges, and the majority colour is taken from a coarse histogram so compression
noise does not split a flat cell into several tones.

## 4. Palette — [`src/color.rs`](../src/color.rs)

Colours are walked from most to least frequent, and each merges into the first
palette entry within `--tolerance`. Near-identical colours — the typical
signature of compression noise — therefore collapse onto the dominant tone rather
than onto each other.

Distance is a perceptually weighted RGB approximation: `0.30·Δr² + 0.59·Δg² +
0.11·Δb² + Δa²`.

## 5. Background — [`src/background.rs`](../src/background.rs)

Only with `--remove-background`. The colour dominating the canvas border is taken
as the background and cleared by flooding inwards **from outside**, so the same
tone enclosed within the artwork survives. The canvas is then cropped to what is
left.

This happens after the palette step so a background left in several near-equal
tones by compression still comes away in one piece.

## 6. Tracing — [`src/trace.rs`](../src/trace.rs)

For each colour, the edges separating its pixels from everything else are
collected with a consistent orientation — the interior always on the left of
travel — and chained end to end until they close into loops.

Each loop is a subpath. Outlines and holes coexist in the same `<path>` thanks to
`fill-rule="evenodd"`. Collinear vertices are dropped, so a rectangular region
ends up as four points.

Two different connectivities are in play on purpose:

- **Grouping into regions uses 8-connectivity**, so a diagonal run of pixels —
  everywhere in pixel art — is one shape rather than a string of little squares.
- **Tracing uses 4-connectivity** at the crossings, so those diagonal pixels get
  separate loops. They still land in the same `<path>`.

## The SVG it produces

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

Since every segment is horizontal or vertical, the relative `h`/`v` commands are
used throughout — half the bytes of `L`.

The `viewBox` is in drawn pixels (1 unit = 1 pixel) and `width`/`height`
reproduce the original image size. `shape-rendering="crispEdges"` keeps the
pixels sharp at any zoom.
