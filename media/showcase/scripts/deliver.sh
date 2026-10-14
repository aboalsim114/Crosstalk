#!/usr/bin/env bash
# Builds the deliverables from out/video.mp4 (picture) and out/score.wav (sound):
#   out/crosstalk-showcase.mp4  H.264 High, 1920×1080, 30 i/s, AAC, faststart
#   out/crosstalk-showcase.gif  README preview (translation moment + logo), ≤ 5 MB
#   out/crosstalk-showcase-cover.jpg / .png  cover with play badge (needs out/cover.png from render.mjs --cover)
set -euo pipefail
cd "$(dirname "$0")/.."
test -f out/video.mp4 || { echo "out/video.mp4 missing: run npm run render"; exit 1; }
test -f out/score.wav || python3 scripts/score.py

ffmpeg -y -loglevel error -i out/video.mp4 -i out/score.wav \
  -map 0:v -map 1:a -c:v libx264 -preset slow -crf 19 -profile:v high -level 4.1 -pix_fmt yuv420p \
  -x264-params "aq-mode=3:aq-strength=0.9" -tune film \
  -colorspace bt709 -color_primaries bt709 -color_trc bt709 \
  -c:a aac -b:a 160k -ar 48000 -movflags +faststart -shortest \
  -metadata title="Crosstalk — Comprenez le call. Restez dans le jeu." \
  out/crosstalk-showcase.mp4

# GIF: the in-game translation (35.6 → 40.0 s) then the logo (46.7 → 49.2 s), 12 i/s, 720 px.
ffmpeg -y -loglevel error -i out/video.mp4 -filter_complex "\
[0:v]trim=35.6:40.0,setpts=PTS-STARTPTS[a];\
[0:v]trim=46.7:49.2,setpts=PTS-STARTPTS[b];\
[a][b]concat=n=2:v=1[c];\
[c]fps=12,scale=720:-1:flags=lanczos,split[x][y];\
[x]palettegen=max_colors=192:stats_mode=full[p];\
[y][p]paletteuse=dither=sierra2_4a" out/crosstalk-showcase.gif

if test -f out/cover.png; then
  ffmpeg -y -loglevel error -i out/cover.png -vf scale=1600:-1:flags=lanczos -q:v 3 out/crosstalk-showcase-cover.jpg
fi
ls -la out/crosstalk-showcase.* out/crosstalk-showcase-cover.* 2>/dev/null
