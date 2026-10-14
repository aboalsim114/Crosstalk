# Crosstalk · vidéo de présentation

Film produit de 50 s (1920 × 1080, 30 i/s, H.264 + AAC) affiché en tête du README du dépôt. Tout est généré par du code : scènes Three.js, typographie HTML, bande-son synthétisée. Aucun service payant, aucun asset de Counter-Strike 2.

- Storyboard et direction artistique : [`STORYBOARD.md`](STORYBOARD.md)
- Vidéo, GIF et couverture publiés : branche [`media`](https://github.com/aboalsim114/Crosstalk/tree/media/showcase) (hors de l'historique de `develop`)

## Contenu du dossier

| Chemin | Rôle |
| --- | --- |
| `index.html`, `src/main.js` | Scène de 1920 × 1080 : rendu WebGL, calque de texte, grain, vignette, fondu. `window.__seek(t)` dessine l'image de l'instant `t`. |
| `src/scenes/s1-intro.js` … `s6-finale.js` | Les six scènes, avec leurs minutages, mouvements de caméra et textes. |
| `src/materials.js` | Shaders : particules, rubans lumineux, fenêtres (coins arrondis, reflet, balayage), cartes, fond, sol. |
| `src/text.js` | Textures dessinées sur canvas : messages, blocs d'overlay (styles exacts de `OverlayStack`), étiquettes, texte échantillonné en particules. |
| `src/wave.js`, `src/windows.js`, `src/dom.js`, `src/util.js` | Onde sonore, fenêtres de l'app en 3D, révélations typographiques, courbes d'animation et palette. |
| `public/screens/*.jpg` | Vraies captures de l'interface (version web de l'app en mode démonstration). |
| `scripts/capture-app.mjs` | Refait ces captures. |
| `scripts/score.py` | Compose la bande-son (`out/score.wav`). |
| `scripts/render.mjs` | Rend le film image par image et l'envoie à FFmpeg (`out/video.mp4`), ou des images fixes, ou la couverture. |
| `scripts/deliver.sh` | Produit les livrables : MP4 final avec le son, GIF, couverture JPEG. |
| `scripts/publish-media.sh` | Publie les livrables sur la branche `media`. |

## Régénérer

Prérequis : Node.js 22, Python 3 avec numpy, FFmpeg, Chromium (`CHROMIUM=/chemin/vers/chrome` si ce n'est pas celui de Playwright).

```bash
cd media/showcase
npm ci

# 1. (facultatif) Refaire les captures de l'app : lancer l'app web à la racine du dépôt
#    (npx vite --port 5199), puis :
npm run capture
for f in 02-dashboard 03-overlay 04-audio 05-languages; do ffmpeg -y -i public/screens/$f.png -q:v 2 public/screens/$f.jpg; done

# 2. Bande-son, film, couverture, livrables
npm run score
npm run render                       # ≈ 30 min sur 4 cœurs sans GPU (SwiftShader)
node scripts/render.mjs --cover
npm run deliver                      # → out/crosstalk-showcase.mp4 / .gif / -cover.jpg

# 3. Publier sur la branche media (liens du README inchangés)
bash scripts/publish-media.sh
```

Pendant le travail :

- `npm run dev` puis `http://localhost:5300/?play` pour voir le film en direct (sans le son), ou `?t=27.5` pour figer un instant ;
- `node scripts/render.mjs --stills 3,12.5,40` pour des images fixes dans `out/stills/` ;
- `node scripts/render.mjs --from 22 --to 33` pour rendre une seule scène.

Le rendu est déterministe : chaque image ne dépend que de son instant. Ne modifiez pas les sources pendant un rendu.

## Modifier

- **Textes** : dans chaque scène (`CAPTIONS` de la scène 4, `STATIONS` de la scène 3, `MESSAGES` de la scène 2…).
- **Minutage** : chaque scène déclare `from` / `to` ; à l'intérieur, les instants sont absolus (en secondes). Les repères de la bande-son sont dans `scripts/score.py` : déplacez-les avec l'image.
- **Couleurs et polices** : `src/util.js` (`C`, `FONT`), identiques à `src/styles/global.css` de l'app.
- **Fenêtres** : remplacez les captures de `public/screens/` (même nom, format 2640 × 1680).

## Fidélité

Les fenêtres sont de vraies captures de l'app. Les répliques viennent de ses données de démonstration (`src/app/state/data.ts`). La scène en jeu est une mise en scène : elle reproduit le fonctionnement de l'overlay (ligne provisoire puis traduction finale, empilement des blocs), pas un enregistrement de partie. Aucune mesure de performance n'est présentée, et les fonctions expérimentales n'apparaissent pas.

## Licences

Polices Instrument Serif, Geist et JetBrains Mono (SIL Open Font License, via Fontsource), Three.js (MIT). La musique et les effets sonores sont synthétisés par `scripts/score.py`, sans échantillon externe.
