#!/usr/bin/env bash
# Publishes the showcase deliverables on the `media` branch, whose links (README.md)
# never change:
#   https://github.com/aboalsim114/Crosstalk/blob/media/showcase/crosstalk-showcase.mp4  (GitHub player)
#   https://github.com/aboalsim114/Crosstalk/raw/media/showcase/crosstalk-showcase.mp4   (file)
#   https://github.com/aboalsim114/Crosstalk/raw/media/showcase/crosstalk-showcase.gif
#
# Like `downloads`, the branch holds a single commit replaced at every publish: the
# video never enters the history of develop or main.
# Run from media/showcase after `npm run deliver`.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
repo=aboalsim114/Crosstalk
branch=media
commit=$(git -C "$root" rev-parse --short HEAD)
for f in crosstalk-showcase.mp4 crosstalk-showcase.gif crosstalk-showcase-cover.jpg; do
  [ -f "$here/out/$f" ] || { echo "manquant : out/$f (npm run deliver)" >&2; exit 1; }
done

tmp=$(mktemp -d)
trap 'git -C "$root" worktree remove --force "$tmp" >/dev/null 2>&1 || true; rm -rf "$tmp"' EXIT
git -C "$root" worktree add --detach "$tmp" >/dev/null 2>&1
cd "$tmp"
git checkout -q --orphan "media-$commit"
git rm -rfq . >/dev/null
mkdir -p showcase
cp "$here/out/crosstalk-showcase.mp4" "$here/out/crosstalk-showcase.gif" "$here/out/crosstalk-showcase-cover.jpg" showcase/
dur=$(ffprobe -v error -show_entries format=duration -of default=nw=1:nk=1 showcase/crosstalk-showcase.mp4)
{
  echo "# Crosstalk — médias de présentation"
  echo
  echo "Fichiers produits par [\`media/showcase\`](https://github.com/$repo/tree/develop/media/showcase) (commit \`$commit\`). Branche à commit unique, remplacée à chaque publication."
  echo
  echo "| Fichier | Taille | SHA-256 |"
  echo "|---|---|---|"
  for f in showcase/*; do
    echo "| [\`$(basename "$f")\`]($f) | $(du -h "$f" | cut -f1) | \`$(sha256sum "$f" | cut -c1-16)…\` |"
  done
  echo
  printf 'Vidéo : %.1f s, 1920 × 1080, 30 i/s, H.264 + AAC.\n' "$dur"
} > README.md
git add -A
git commit -qm "Médias de présentation ($commit)"
git push -qf origin "HEAD:refs/heads/$branch"
cd "$root"
git branch -D "media-$commit" >/dev/null 2>&1 || true
echo "publié : https://github.com/$repo/blob/$branch/showcase/crosstalk-showcase.mp4"
