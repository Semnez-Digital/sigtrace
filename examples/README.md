# Examples

`signatures/` holds 131 **public-domain** modern signatures (politicians, world
leaders, tech, sports, entertainment) harvested from Wikimedia Commons — the
test corpus. They contain no personal data. *(The author's real eID signature is
deliberately **not** in this repo.)*

`comparison.png` shows three labelled variants per signature:

| column | what it is |
|---|---|
| **original** | the clean source SVG, rendered as black ink |
| **degraded (eID-like)** | crushed to a ~250px grayscale JPEG (q90) — what an eID actually stores; shown blown up so the blocky low-res input is visible |
| **sigtrace** | that degraded JPEG run through the tracer |

Regenerate it (needs `rsvg-convert` + ImageMagick `magick` on PATH; builds the
binary if missing):

```sh
./generate.sh                  # built-in sample
./generate.sh elon_musk pele   # specific signatures
```
