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
setup_bytes=$(stat -c %s "$setup")
# Same number as the one compiled into the app (src-tauri/build.rs).
build_no=$(git rev-list --count HEAD)
version=$(node -p "require('./src-tauri/tauri.conf.json').version")
# What's new, for the players (release-notes.json, written with each pull
# request): each entry gets the build that first shipped it, remembered in
# news.json on the downloads branch. No commit, no pull request number.
git fetch -q origin "$branch" 2>/dev/null || true
prev_news=$(git show "origin/$branch:news.json" 2>/dev/null || echo '[]')
news_json=$(node scripts/release-notes.mjs "$build_no" "$prev_news")
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
  echo "| Build | $build_no (mise à jour depuis l'application : \`latest.json\`) |"
  echo "| Commit | [\`$commit\`](https://github.com/$repo/commit/$sha) |"
  echo "| Construite le | $date |"
  echo "| SHA-256 de l'installateur | \`$setup_sha\` |"
  echo "| SHA-256 des outils (zip) | \`$zip_sha\` |"
  echo
  echo "Installateur Windows (x64), design « Crosstalk Installer » : installe Crosstalk (menu Démarrer, bureau) et télécharge le modèle de reconnaissance vocale pendant l'installation (Parakeet TDT 0.6B v3 et la détection de voix Silero : environ 671 Mo, une seule fois, puis tout fonctionne hors ligne). La reconnaissance tourne sur le processeur (sherpa-onnx, ONNX Runtime) ; la traduction passe par le LLM choisi au premier lancement. Compilé avec MinGW/gcc (\`scripts/cross-build-windows-gnu.sh\`). La version officielle reste celle compilée avec MSVC par \`scripts\\validate.ps1\`. Non signée : SmartScreen affiche un avertissement (« Informations complémentaires » > « Exécuter quand même »)."
  echo
  echo "Tout est local : aucune clé API, aucun abonnement, aucun envoi en ligne. Le zip contient les outils de test en ligne de commande (\`LISEZMOI-TEST.txt\`)."
} > VERSION.md
# Update feed read by the app (src-tauri/src/updater.rs).
node -e '
  const [build, version, commit, date, sha256, bytes, url, newsArg] = process.argv.slice(1);
  const { notes, news } = JSON.parse(newsArg);
  const feed = { build: Number(build), version, commit, date, notes, news: news.slice(0, 12), setup: { url, sha256, bytes: Number(bytes) } };
  require("fs").writeFileSync("latest.json", JSON.stringify(feed, null, 1) + "\n");
  require("fs").writeFileSync("news.json", JSON.stringify(news, null, 1) + "\n");
' "$build_no" "$version" "$commit" "$date" "$setup_sha" "$setup_bytes" "https://raw.githubusercontent.com/$repo/$branch/$setup_asset" "$news_json"
git add "$asset" "$setup_asset" VERSION.md latest.json news.json
git commit -qm "Version de test $commit ($date)"
git push -qf origin "HEAD:refs/heads/$branch"
cd "$root"
git branch -D "publish-$commit" >/dev/null 2>&1 || true
echo "publié : $commit -> https://github.com/$repo/raw/$branch/$setup_asset"
