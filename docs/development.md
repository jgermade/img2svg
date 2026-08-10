# Development

## Toolchain

The Rust version and targets come from [`rust-toolchain.toml`](../rust-toolchain.toml)
and nowhere else, so local and CI use exactly the same thing. rustup installs it
on the first `cargo` invocation.

An exact version is pinned rather than `stable`: with `stable`, a release landing
between a local build and a CI build would be enough for them to stop matching.

> If `rustc --version` says `(Homebrew)` or anything other than what the file
> says, a second Rust installation is shadowing rustup on your `PATH` and the
> file is being ignored. Put `$HOME/.cargo/bin` first, or remove the other one.

## Build and test

```sh
cargo test                 # debug: overflow checks on
cargo test --release       # what ships
cargo build --release      # CLI at target/release/img2svg
cargo fmt --check
cargo clippy --all-targets
```

## The web build

```sh
wasm-pack build --release --target web \
  --out-dir web/pkg --out-name img2svg \
  -- --no-default-features --features wasm,photo

cp img2svg.svg web/
python3 -m http.server 8765 --directory web
```

The wasm must be served over HTTP — opening `web/index.html` from the filesystem
will not work, because the page loads a module worker.

`--no-default-features` is deliberate: the browser decodes the image, so the Rust
image codecs are half a megabyte of dead weight in the bundle.

`photo` is in, so the page ships **one** bundle with both modes. Measured, it
costs 152 KB → 209 KB raw but only 52 KB → 68 KB brotli, and the rule of thumb
this was weighed against was written in raw bytes. Seventeen kilobytes over the
wire does not pay for two `pkg/` directories, a loader that picks between them,
two wasm builds in CI and two `.d.ts` files to keep an eye on. Revisit if the
curve fitters change the shape of that.

## Tests

| Suite | What it covers |
| --- | --- |
| `tests/checker.rs`, `grid.rs`, `trace.rs`, `background.rs`, `color.rs`, `palette.rs`, `cluster.rs`, `speckle.rs`, `boundary.rs` | Unit tests per module. |
| `tests/fit.rs` | The fitting axis, read back out of the emitted `d` attributes rather than from internals. Holds the seam check: every interior segment is drawn by exactly two faces, with the same endpoints. |
| `tests/golden.rs` | Snapshots of the pixel art path over a synthetic ASCII sprite. Input lives in the file, so it runs anywhere. |
| `tests/photo.rs` | Snapshots of the photo path over a synthetic drawing — a gradient, two flat blocks, a one-pixel line and a few loose dots, one motif per behaviour that was decided by looking at results. |
| `tests/corpus.rs` | Snapshots over the real images in `examples/`. |

### Snapshots

`tests/golden/` holds the committed output of each case. They exist so a refactor
can prove it did not change behaviour — they catch things no unit test sees, like
a lost `simplify` filling every path with collinear vertices without changing the
drawing.

After an intentional change:

```sh
UPDATE_GOLDEN=1 cargo test
```

and **read the diff** before committing. Each snapshot carries the conversion
metadata in a comment header, so the diff says *what* changed — the grid, the
path count — and not merely that something did. They are still valid SVGs; open
one in a browser.

On a mismatch the actual output is dumped alongside as `*.actual` for a real
`diff`.

### The corpus

`examples/` is not versioned — the three PNGs weigh 9 MB — so they hang off a
release of their own, `corpus-v1`, and come down with:

```sh
scripts/corpus.sh
```

That release is not a version of the program: nothing about it moves when a
`v0.x` ships, it is not marked as latest, and the script pins it by tag rather
than following the newest one. The day the corpus changes it takes a new tag, the
tag changed in the script, and the snapshots regenerated in the same commit:

```sh
scripts/corpus-release.sh corpus-v2
```

It packs the images `scripts/corpus.sha256` names, straight out of `examples/`,
and refuses to publish when they do not match those checksums. There is no
archive to hand it on purpose: that list is what the fetch script verifies and
what the tests read, so the only thing publishable is what the repository already
calls the corpus. It wants a token with `Contents: write`.

The script checks the unpacked PNGs against `scripts/corpus.sha256` and fetches
again when they do not match; without them `tests/corpus.rs` skips with a notice.
Their *output* snapshots are committed, so the regression signal is in git even
though the inputs are not, and each header carries the input's FNV hash to tell a
changed image apart from a code regression.

`REQUIRE_CORPUS=1` turns a missing corpus into a failure instead of a silent
pass. CI sets it, having fetched the corpus a step earlier.

## CI

**`build.yml`** runs on every push to `main`: format, clippy, the corpus fetch,
tests in debug and release, and the wasm build. It publishes nothing — it just
leaves the built `web/` as an artifact.

Tests run in debug *as well as* release because release turns overflow checks
off — that is exactly how a `u8` underflow in `checker.rs` survived unnoticed.

**`release.yml`** is manual: pick `patch`, `minor` or `major` from the
`workflow_dispatch` menu. It bumps the version in `Cargo.toml` and `Cargo.lock`,
commits as `release: vX.Y.Z`, tags, then reuses `build.yml` on that commit and
fans out into:

- **`pages`** — deploys the site. Pages must be enabled under **Settings → Pages
  → Source: GitHub Actions**.
- **`binaries`** — the CLI for five targets (macOS arm64/x86_64, Linux
  arm64/x86_64, Windows x86_64).
- **`publish`** — creates the GitHub release with all of it attached, the web
  package included.

**The site only updates on release**, not on every push to `main`. That is
deliberate: what is published then always corresponds to a tagged version.

Two details worth knowing before editing these:

- `build.yml` skips its own run when the commit message starts with `release:`,
  otherwise every release would trigger a duplicate build.
- The reusable call passes an explicit `ref`. A called workflow checks out the
  default branch, so without it the release would build the commit *before* the
  version bump.

## Session notes

`SESSIONS/` holds the design record: what was decided, why, and what it corrected
about the previous plan. One file per working session, named

```
YYYY-MM-DD-HHhMM.slug.md
```

The time is when the document was written, not when it was last edited, so a
document that keeps getting updated keeps its name. Sorting by filename therefore
sorts chronologically, which matters because several can land on one day, and the
newest one is the live list — each says at the top which of its predecessors it
supersedes.

They are **history, not documentation**: an older file records what was true when
it was written and is deliberately not corrected afterwards. Anything meant to stay
true belongs in `docs/`.

## Language

Source comments, test names and program output are in Spanish. Documentation —
this file, the README, `docs/` and `SESSIONS/` — is in English.
