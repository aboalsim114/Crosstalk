#!/usr/bin/env bash
# Publishes the Windows installer of the current commit on the `downloads`
# branch, whose download links (in README.md) never change:
#   https://github.com/aboalsim114/Crosstalk/raw/downloads/Crosstalk-setup.exe
#   https://github.com/aboalsim114/Crosstalk/raw/downloads/crosstalk-windows-x64-portable.zip (test tools)
#
# The branch holds a single commit (installer, tools zip, VERSION.md), replaced at
# every publish: the repository history does not grow with binaries.
#
# Run after every merge into develop, from a clean checkout of develop:
#   scripts/publish-dev-build.sh            # build (scripts/cross-build-windows-gnu.sh) then publish
#   scripts/publish-dev-build.sh --no-build # publish the package already built for HEAD
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
repo=aboalsim114/Crosstalk
branch=downloads
asset=crosstalk-windows-x64-portable.zip
setup_asset=Crosstalk-setup.exe
commit=$(git rev-parse --short HEAD)
sha=$(git rev-parse HEAD)

[ "${1:-}" = "--no-build" ] || bash scripts/cross-build-windows-gnu.sh
zip="dist-test/crosstalk-$commit-windows-x64-gcc-test.zip"
setup="dist-test/Crosstalk-$commit-setup.exe"
[ -f "$zip" ] && [ -f "$setup" ] || { echo "paquet ou installateur introuvable pour $commit" >&2; exit 1; }
zip_sha=$(sha256sum "$zip" | cut -d' ' -f1)
setup_sha=$(sha256sum "$setup" | cut -d' ' -f1)
date=$(date -u '+%Y-%m-%d %H:%M UTC')

tmp=$(mktemp -d)
trap 'git -C "$root" worktree remove --force "$tmp" >/dev/null 2>&1 || true; rm -rf "$tmp"' EXIT
git worktree add --detach "$tmp" >/dev/null 2>&1
cd "$tmp"
git checkout -q --orphan "publish-$commit"
git rm -rfq . >/dev/null
cp "$root/$zip" "$asset"
cp "$root/$setup" "$setup_asset"
{
  echo "# Crosstalk — version de test (develop)"
  echo
  echo "**[Télécharger l'installateur $setup_asset](https://github.com/$repo/raw/$branch/$setup_asset)** · outils de test : [$asset](https://github.com/$repo/raw/$branch/$asset)"
  echo
  echo "| | |"
  echo "|---|---|"
  echo "| Commit | [\`$commit\`](https://github.com/$repo/commit/$sha) |"
  echo "| Construite le | $date |"
  echo "| SHA-256 de l'installateur | \`$setup_sha\` |"
  echo "| SHA-256 des outils (zip) | \`$zip_sha\` |"
  echo
  echo "Installateur Windows (x64), design « Crosstalk Installer » : installe Crosstalk (menu Démarrer, bureau) et télécharge ses modèles pendant l'installation (environ 780 Mo, une seule fois). Compilé avec MinGW/gcc (\`scripts/cross-build-windows-gnu.sh\`). La version officielle reste celle compilée avec MSVC par \`scripts\\validate.ps1\`. Non signée : SmartScreen affiche un avertissement (« Informations complémentaires » > « Exécuter quand même »)."
  echo
  echo "Tout est local : aucune clé API, aucun abonnement, aucun envoi en ligne. Le zip contient les outils de test en ligne de commande (\`LISEZMOI-TEST.txt\`)."
} > VERSION.md
git add "$asset" "$setup_asset" VERSION.md
git commit -qm "Version de test $commit ($date)"
git push -qf origin "HEAD:refs/heads/$branch"
cd "$root"
git branch -D "publish-$commit" >/dev/null 2>&1 || true
echo "publié : $commit -> https://github.com/$repo/raw/$branch/$setup_asset"
