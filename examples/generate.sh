#!/usr/bin/env bash
# Regenerate examples/comparison.png — three labelled variants per signature:
#   original  |  degraded (eID-like)  |  sigtrace
#
# Pipeline per signature (all from the public-domain SVGs in signatures/):
#   original  : render the clean SVG as black ink on white
#   degraded  : crush it to a ~250px grayscale JPEG (q90) — what an eID stores
#   sigtrace  : feed that through the tracer (jpeg -> pgm -> svg -> render)
#
# Pass names to override the built-in sample, e.g.:  ./generate.sh elon_musk pele
set -euo pipefail
cd "$(dirname "$0")"

BIN=../target/release/sigtrace
[ -x "$BIN" ] || (cd .. && cargo build --release)

# Render a signature SVG to black-ink-on-white PNG ($2) at width $3.
# Uses the ALPHA channel (render natural on transparent, then extract+negate) so
# it works for stroked paths (fill:none), filled paths, and any ink colour.
# A forced-black fill stylesheet would fill stroke-only paths into black blobs.
ink() {  # ink <svg> <out.png> <width>
  local tmp; tmp="$(mktemp).png"
  rsvg-convert -w "$3" -o "$tmp" "$1"
  magick "$tmp" -alpha extract -negate "$2"
  rm -f "$tmp"
}

# montage labels need a font; find one across platforms
FONT=""
for f in /System/Library/Fonts/Supplemental/Arial.ttf \
         /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
         /Library/Fonts/Arial.ttf; do
  [ -f "$f" ] && FONT="$f" && break
done
FONTARG=(); [ -n "$FONT" ] && FONTARG=(-font "$FONT")

SAMPLE=${*:-"barack_obama donald_trump joe_biden emmanuel_macron elon_musk \
lionel_messi cristiano_ronaldo taylor_swift vladimir_putin volodymyr_zelensky \
tom_hanks stephen_king"}

W="$(mktemp -d)"
cells=()
for k in $SAMPLE; do
  svg="signatures/$k.svg"
  [ -f "$svg" ] || { echo "skip $k (no svg)"; continue; }
  ink "$svg" "$W/${k}_o.png"   900
  ink "$svg" "$W/${k}_big.png" 1600
  magick "$W/${k}_big.png" -colorspace Gray -resize 250x -quality 90 "$W/${k}_deg.jpg"
  magick "$W/${k}_deg.jpg" -filter point -resize 900x "$W/${k}_dv.png"   # show true pixels
  magick "$W/${k}_deg.jpg" -colorspace Gray -depth 8 "$W/${k}.pgm"
  "$BIN" "$W/${k}.pgm" "$W/${k}.svg" --fill '#1a2740' >/dev/null
  rsvg-convert -w 900 -b white -o "$W/${k}_t.png" "$W/${k}.svg"
  cells+=( -label "$k — original" "$W/${k}_o.png" \
           -label "degraded (eID-like)" "$W/${k}_dv.png" \
           -label "sigtrace" "$W/${k}_t.png" )
done

magick montage "${FONTARG[@]}" -pointsize 22 -fill black "${cells[@]}" \
  -tile 3x -geometry 460x200+6+10 -background white comparison.png
rm -rf "$W"
echo "wrote examples/comparison.png"
