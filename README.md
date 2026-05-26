# sigtrace

**Turn a tiny, low-quality eID signature JPEG into clean, resolution-independent
vector ink.**

eID cards store the holder's handwritten signature as a tiny, heavily-compressed
grayscale JPEG (≈250 px wide, a few KB). Dropped straight into a document it
looks blocky and shaky. `sigtrace` cleans it up and re-draws it as smooth
cubic-Bézier paths you can embed in a PDF — sharp at any zoom or print size.

It is **signature-specialised, not a general image tracer**: it assumes
dark-ish ink on a light background and is tuned hard for that one job.

```
   blocky 250px JPEG            sigtrace
   ┌───────────────┐          ┌───────────────┐
   │  ~╱╲╮ shaky    │   ──►    │   ╱╲  smooth   │   →  <path d="M…C…Z"/>
   │  staircased    │          │   vector ink   │
   └───────────────┘          └───────────────┘
```

See [`examples/comparison.png`](examples/comparison.png) for *original → degraded
→ sigtrace* across a dozen well-known signatures.

## Why it's easy to ship

**Zero dependencies.** The entire pipeline is hand-rolled `std` Rust — no image
crates, no C libraries, no GPL. That means it cross-compiles cleanly to
**macOS, iOS, Android, Linux and Windows** with no build chain. JPEG decoding is
left to the host (every platform already has one); the core takes grayscale
pixels and returns an SVG/`CGPath`-style path string.

It also deliberately avoids GPL tools like Potrace, so it's safe in a closed
App Store / Play Store binary.

## How it works

```
bicubic upscale → auto-contrast → pre-threshold blur → fixed threshold
→ despeckle → pixel-edge contour → resample → Schneider cubic-Bézier fit → SVG
```

Each step earns its place (validated on a 131-signature benchmark):

- **Fixed threshold, not Otsu.** Otsu lands the cut up in the JPEG halo and
  over-thickens strokes ~28 %; a fixed level on the true stroke edge keeps the
  weight faithful.
- **Auto-contrast guard.** If an input has no genuinely dark ink (faded / low
  contrast), `[p2,p98]` is stretched to `[0,255]` so it still thresholds instead
  of tracing to nothing. Normal dark-ink eID inputs are detected and left
  untouched — no regression. (`Options::auto_contrast`, default on.)
- **Pre-threshold blur** (`sigma = blur_k · upscale_factor`). Upscaling a ~250 px
  source makes the threshold crossing wiggle once per source pixel — a regular
  "staircase" tremor. A symmetric blur removes it *without* shifting edges
  inward (smoothing the traced contour instead would shrink the strokes).
- **Pixel-edge contours** (boundaries *between* pixels, like Potrace) — correct
  stroke width with no half-pixel inset, and holes handled via even-odd fill.
- **Schneider least-squares Bézier fit** (Graphics Gems, 1990) — permissive, no
  GPL — turns the staircased boundary into smooth cubic curves.

## Results

Across **131 modern signatures** (Wikimedia Commons) degraded to eID quality
(~250 px, grayscale, JPEG), scored by IoU against the clean original:

| metric | value |
|---|---|
| mean IoU | **0.94** |
| every signature | **≥ 0.80** |
| ≥ 0.90 | 89 % |
| under heavy (Q40/Q30) compression | mean still ~0.93 |
| trace time | ~20 ms per signature |

That's on par with GPL Potrace's fidelity, while staying permissive and
dependency-free. Faded/light-ink inputs (which would otherwise trace to nothing)
recover to ~0.95 mean thanks to the auto-contrast guard.

## Library usage

```rust
use sigtrace::{trace_signature_gray, svg_document, Options};

// `gray` = row-major 8-bit grayscale (dark ink on light paper), host-decoded.
let res = trace_signature_gray(&gray, width, height, &Options::default());

let svg = svg_document(&res, "#1a2740");  // standalone SVG, or…
let path = &res.path_d;                    // …feed the "M…C…Z" data to a CGPath / Path
```

`Options` exposes `thresh`, `target_w`, `blur_k`, `auto_contrast`, and the
fit/smooth tolerances. `TraceResult` carries `path_d`, `width`, `height`,
`subpaths`.

## CLI (testing / benchmarking)

```sh
cargo build --release
target/release/sigtrace input.pgm output.svg --fill '#1a2740'
# flags: --thresh --width --blur-k --smooth --fit-err --despeckle --no-auto-contrast
```

(The CLI takes a binary PGM so the crate stays dependency-free; in an app you'd
pass decoded pixels to the library directly.)

## Cross-platform build & FFI

```sh
cargo build --release                                   # native: macOS / Linux / Windows

rustup target add aarch64-apple-ios
cargo build --release --target aarch64-apple-ios        # -> libsigtrace.a  (link from Swift)

rustup target add aarch64-linux-android
cargo build --release --target aarch64-linux-android    # -> libsigtrace.so (JNI from Kotlin)
```

The crate builds `rlib` + `staticlib` + `cdylib`. Wrap `trace_signature_gray`
in a small `extern "C"` shim for Swift / JNI — it's allocation-light and returns
a path string.

## Tests & examples

```sh
cargo test               # contour topology, resample, Bézier-fit, auto-contrast guard
examples/generate.sh     # rebuilds examples/comparison.png (needs rsvg-convert + ImageMagick)
```

## License

Licensed under the **MIT License** — see [LICENSE](LICENSE).
Copyright (c) 2026 Backup Experts S.R.L.

The signatures in [`examples/signatures/`](examples/signatures/) are
public-domain works from Wikimedia Commons, included only as test fixtures.
