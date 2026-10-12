#!/usr/bin/env bash
# Publishes the test build of the current commit to the GitHub pre-release
# `develop-latest`, whose download link (in README.md) never changes:
#   https://github.com/aboalsim114/Crosstalk/releases/download/develop-latest/crosstalk-windows-x64-portable.zip
#
# Run after every merge into develop, from a clean checkout of develop:
#   scripts/publish-dev-build.sh            # build (scripts/cross-build-windows-gnu.sh) then publish
#   scripts/publish-dev-build.sh --no-build # publish the package already built for HEAD
# Requires gh (authenticated) and the cross-build requirements.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
repo=aboalsim114/Crosstalk
tag=develop-latest
asset=crosstalk-windows-x64-portable.zip
commit=$(git rev-parse --short HEAD)
sha=$(git rev-parse HEAD)

[ "${1:-}" = "--no-build" ] || bash scripts/cross-build-windows-gnu.sh
zip="dist-test/crosstalk-$commit-windows-x64-gcc-test.zip"
[ -f "$zip" ] || { echo "paquet introuvable : $zip" >&2; exit 1; }
cp "$zip" "dist-test/$asset"
zip_sha=$(sha256sum "dist-test/$asset" | cut -d' ' -f1)
date=$(date -u '+%Y-%m-%d %H:%M UTC')

notes="Version de test **portable** de Windows (x64), construite automatiquement depuis \`develop\`.

- Commit : [\`$commit\`](https://github.com/$repo/commit/$sha) · construite le $date
- SHA-256 du zip : \`$zip_sha\`
- Compilée avec MinGW/gcc (scripts/cross-build-windows-gnu.sh). La version officielle reste celle compilée avec MSVC par \`scripts\\\\validate.ps1\`.
- Non signée : SmartScreen affiche un avertissement (« Informations complémentaires » > « Exécuter quand même »).
- Mode d'emploi dans \`LISEZMOI-TEST.txt\` (dans le zip). Modèles : \`download-models.ps1 -Models base -Translation\`.
- Tout est local : aucune clé API, aucun abonnement, aucun envoi en ligne."

# Move the tag to this commit (created on first publish).
if gh api "repos/$repo/git/refs/tags/$tag" >/dev/null 2>&1; then
  gh api -X PATCH "repos/$repo/git/refs/tags/$tag" -f sha="$sha" -F force=true >/dev/null
else
  gh api "repos/$repo/git/refs" -f ref="refs/tags/$tag" -f sha="$sha" >/dev/null
fi
id=$(gh api "repos/$repo/releases/tags/$tag" --jq .id 2>/dev/null || true)
if [ -z "$id" ]; then
  id=$(gh api "repos/$repo/releases" -f tag_name="$tag" -f name="Crosstalk — version de test (develop)" \
    -f body="$notes" -F prerelease=true --jq .id)
else
  gh api -X PATCH "repos/$repo/releases/$id" -f body="$notes" >/dev/null
fi
# Replace the asset under the same name, so the README link stays valid.
for old in $(gh api "repos/$repo/releases/$id/assets" --jq ".[] | select(.name == \"$asset\") | .id"); do
  gh api -X DELETE "repos/$repo/releases/assets/$old" >/dev/null
done
gh api -X POST "https://uploads.github.com/repos/$repo/releases/$id/assets?name=$asset" \
  -H "Content-Type: application/zip" --input "dist-test/$asset" --jq '.browser_download_url'
echo "publié : $commit"
