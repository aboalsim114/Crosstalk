# Crosstalk — Showcase · storyboard

Film produit de 50 s, 1920 × 1080, 30 i/s. Compréhensible sans le son : chaque idée passe par l'image et le texte.

## Direction artistique

Reprise de l'identité visuelle de l'application (`src/styles/global.css`, maquette `design/maquette`, couverture `docs/assets/crosstalk-readme-cover.svg`).

| Rôle | Valeur |
| --- | --- |
| Fond | encre `#020203`, halos chauds `#141113` → `#08080a` |
| Accent principal | ambre `#ffa24a`, ambre clair `#ffc68a` |
| Accent secondaire | glace `#7fd4ff` (chat, froid) · vert `#7fe0a8` (état actif) |
| Texte | ivoire `#f3ede3` / `#ece6dc`, gris chauds `#a49d92` → `#5a5650` |
| Titres | Instrument Serif, italique ambre sur le mot clé (comme le logo « Cross*talk* ») |
| Interface | Geist Sans |
| Étiquettes, texte original | JetBrains Mono, capitales espacées (le russe est toujours en mono, comme dans l'app) |

Principes : lumière douce et additive, pas de néon ni de glitch ; profondeur par la parallaxe, le flou des plans éloignés et les particules de poussière ; caméra lente, toujours en mouvement ; un fil conducteur visuel (l'onde sonore) relie les six scènes.

## Fidélité au produit

- Les fenêtres de l'application sont de **vraies captures** de l'interface (`scripts/capture-app.mjs`, version web en mode démonstration).
- Les blocs de l'overlay reprennent les styles exacts de `OverlayStack` (`src/features/overlay/OverlayPage.tsx`) : en-tête mono, original en mono gris, traduction en Geist, état « (PROVISOIRE) » avant la version finale.
- Les répliques viennent des données de démonstration de l'app (`src/app/state/data.ts`).
- Les étapes du pipeline sont celles de l'app : capture WASAPI de cs2.exe → Silero VAD → whisper.cpp → LLM (cloud ou local) → overlay.
- Aucun chiffre de performance n'est affiché comme une promesse ; les fonctions expérimentales (Qui parle ?, Focus voix) ne sont pas montrées.
- Le jeu n'est pas filmé : il est évoqué comme dans l'aperçu de l'overlay de l'app (zone sombre, rayures diagonales, réticule vert). Aucun asset de Counter-Strike 2 n'est utilisé.

## Scènes

### 1 · The Intro — 0 → 5 s
Noir. Une onde sonore en 3D (sept rubans, dégradé ambre → glace) naît au centre et s'étend. La caméra avance et tourne légèrement. L'onde s'aplatit en une ligne de lumière qui devient le soulignement du logo. Le logo « Cross*talk* » apparaît lettre par lettre, du flou au net, puis **EVERY VOICE. ONE LANGUAGE.**
*Son : nappe qui s'ouvre, impact grave doux à l'apparition du logo.*

### 2 · The Problem — 5 → 12 s
Un sol tactique (grille, cercles d'écoute) s'étend dans l'obscurité. Des messages flottants, russes et anglais (voix et chat), traversent l'espace en sens opposés, à plusieurs profondeurs ; les plus éloignés sont flous. **Different languages.** puis ***One team.*** À la fin, les messages se dissolvent en particules aspirées vers le centre.
*Son : pulsations graves, souffles.*

### 3 · The Technology — 12 → 22 s
Travelling latéral à travers cinq stations, avec une barre d'étapes en bas reprise de l'Accueil de l'app.
1. **Capture audio** · WASAPI, son de cs2.exe uniquement : onde brute, tirs (gris) et voix (ambre).
2. **Détection vocale** · Silero VAD : les bruits tombent et s'éteignent, la parole est encadrée.
3. **Transcription** · whisper.cpp : les particules se condensent en « Двое идут на B, двое на B ».
4. **Traduction** · le LLM connecté (cloud ou local) : les caractères se réorganisent en « Deux joueurs arrivent sur B, deux sur B. »
5. **Sous-titres** · overlay : la phrase se pose dans un bloc d'overlay.
Titre : « Tout se passe *sur votre PC.* »
*Son : rythme léger, tics à chaque station.*

### 4 · The Application — 22 → 33 s
Raccord sur la même phrase, affichée en grand dans l'Accueil de l'app : la caméra recule et révèle la vraie fenêtre en 3D, avec reflet au sol et balayage lumineux. Les fenêtres suivantes entrent une à une, les précédentes reculent en arc :
1. **Accueil** — « Le call, traduit en direct. »
2. **Audio & IA** — « Whisper en local, sur la carte graphique ou le processeur. »
3. **Overlay** — « Discret, repositionnable, clics traversants. »
4. **Langues** — « 6 langues de lecture, 8 langues de coéquipiers. »
*Son : la musique s'ouvre.*

### 5 · The Experience — 33 → 42 s
Plein cadre sur la zone de jeu. Un indicateur de voix s'anime ; la phrase russe s'écrit au rythme de l'onde. Le bloc d'overlay apparaît en haut à droite, d'abord **provisoire** (« Deux joueurs arrivent sur… »), puis **final**. La caméra s'approche : la lecture est immédiate. Un message de chat anglais arrive et s'empile, traduit. « Lisible *en un regard.* »
*Son : plus intime, une note claire à chaque traduction.*

### 6 · The Finale — 42 → 50 s
La caméra recule ; la zone de jeu et les fenêtres de l'app se rassemblent en éventail au-dessus d'un sol réfléchissant, puis s'effacent dans la lumière. L'onde de l'intro revient et se fige sous le logo. **Crosstalk** · « Comprenez le call. Restez dans le jeu. » · **AVAILABLE FOR WINDOWS** (Windows 10 / 11 · x64) · github.com/aboalsim114/Crosstalk. Fondu au noir.
*Son : montée, impact final, longue résonance.*
