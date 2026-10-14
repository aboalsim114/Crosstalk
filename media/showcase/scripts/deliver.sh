#!/usr/bin/env bash
# Builds the deliverables (the MP4 and GIF are lightly denoised: grain costs bitrate) from out/video.mp4 (picture) and out/score.wav (sound):
#   out/crosstalk-showcase.mp4  H.264 High, 1920×1080, 30 i/s, AAC, faststart
#   out/crosstalk-showcase.gif  README preview (translation moment + logo), ≤ 5 MB
#   out/crosstalk-showcase-cover.jpg / .png  cover with play badge (needs out/cover.png from render.mjs --cover)
set -euo pipefail
cd "$(dirname "$0")/.."
test -f out/video.mp4 || { echo "out/video.mp4 missing: run npm run render"; exit 1; }
test -f out/score.wav || python3 scripts/score.py

ffmpeg -y -loglevel error -i out/video.mp4 -i out/score.wav \
  -map 0:v -map 1:a -vf "hqdn3d=3:2:8:8" \
  -c:v libx264 -preset slow -crf 22 -maxrate 6M -bufsize 12M -profile:v high -level 4.1 -pix_fmt yuv420p \
  -x264-params "aq-mode=3:aq-strength=0.9" -tune film \
  -colorspace bt709 -color_primaries bt709 -color_trc bt709 \
  -c:a aac -b:a 160k -ar 48000 -movflags +faststart -shortest \
  -metadata title="Crosstalk — Comprenez le call. Restez dans le jeu." \
  out/crosstalk-showcase.mp4

# GIF: the in-game translation (35.6 → 40.0 s) then the logo (46.4 → 49.0 s), 10 i/s, 640 px.
# The film grain is filtered out first: noise is what makes GIFs heavy.
ffmpeg -y -loglevel error -i out/video.mp4 -filter_complex "\
[0:v]trim=35.6:40.0,setpts=PTS-STARTPTS[a];\
[0:v]trim=46.4:49.0,setpts=PTS-STARTPTS[b];\
[a][b]concat=n=2:v=1[c];\
[c]hqdn3d=6:4:10:10,fps=10,scale=640:-1:flags=lanczos,split[x][y];\
[x]palettegen=max_colors=128:stats_mode=diff[p];\
[y][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" out/crosstalk-showcase.gif

if test -f out/cover.png; then
  ffmpeg -y -loglevel error -i out/cover.png -vf scale=1600:-1:flags=lanczos -q:v 3 out/crosstalk-showcase-cover.jpg
fi
ls -la out/crosstalk-showcase.* out/crosstalk-showcase-cover.* 2>/dev/null
