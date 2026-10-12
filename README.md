# Crosstalk

Crosstalk traduit en temps réel, en français, les voix et le chat de vos coéquipiers dans **Counter-Strike 2**. Les sous-titres s'affichent dans un overlay discret.

Crosstalk est une **application de bureau Windows** (Windows 10 et 11, x64), construite avec **Tauri v2** :

- **Cœur applicatif en Rust** : processus, fonctionnalités système, puis la capture audio, la reconnaissance vocale et la traduction.
- **Interface en React + TypeScript**, affichée par WebView2.

Ce n'est pas un site web : l'application se lance depuis un `.exe` et s'installe avec un installateur `.exe` (NSIS) ou `.msi`.

> Crosstalk est un outil externe. Il n'injecte rien dans `cs2.exe`, ne lit pas la mémoire du jeu et n'intercepte pas le trafic réseau.

## État du projet

- **Voix réelles** : capture du son de CS2 (WASAPI process loopback), détection de parole (Silero VAD), transcription russe/anglaise (whisper.cpp), puis **traduction en français sur le PC** (Opus-MT). Tout est local : aucune clé API, aucun abonnement, aucun envoi en ligne. Les modèles s'installent avec `scripts\download-models.ps1 -Translation`.
- **Chat écrit** : encore simulé dans l'interface (lignes d'exemple).

## Dépôt

| Dossier | Contenu |
| --- | --- |
| `src-tauri/` | Cœur Rust : application Tauri v2, commandes natives, configuration de l'installateur. |
| `src/` | Interface React + TypeScript (Tailwind CSS, Motion, Three.js), embarquée dans l'exécutable. |
| `docs/` | **Cahier des charges v2.0** ([cahier-des-charges.md](docs/cahier-des-charges.md)) et **architecture** ([architecture.md](docs/architecture.md)) : les documents de référence. |
| `design/` | Maquette d'origine (Claude Design), cahier des charges v1.0 et transcriptions de conception. Référence historique. |
| `scripts/` | Outillage local : `validate.ps1` (validation complète sous Windows), garde anti-triche, modèles Whisper, banc d'essai. |

## Intégration Windows

| Fonction | Comportement |
| --- | --- |
| Fenêtre | Fenêtre native sans bordures, redimensionnable, avec la barre de titre de la maquette. Apparaît dans la barre des tâches. |
| Zone de notification | Icône avec menu : afficher, démarrer/arrêter, overlay, pause, arrêt d'urgence, quitter. Fermer la fenêtre la masque, l'application continue de tourner. |
| Raccourcis globaux | `Ctrl+Alt+F8` overlay, `Ctrl+Alt+F9` pause, `Ctrl+Alt+F10` arrêt d'urgence. Actifs même quand CS2 a le focus (`RegisterHotKey`, sans hook clavier). Les conflits avec d'autres applications sont signalés dans *Diagnostic*. |
| Overlay en jeu | Seconde fenêtre transparente, toujours au premier plan, absente de la barre des tâches et click-through par défaut. Elle s'ancre dans le coin choisi et peut être repositionnée. Fonctionne avec CS2 en mode fenêtré sans bordures. |
| Démarrage avec Windows | Option dans *À propos* : lance Crosstalk réduit dans la zone de notification (`--minimized`). |
| Instance unique | Un second lancement remet la fenêtre existante au premier plan. |

## Installation (utilisateurs)

Téléchargez l'installateur depuis une Release GitHub :

- `Crosstalk_x.y.z_x64-setup.exe` (NSIS) : installation pour l'utilisateur courant, sans droits administrateur ;
- `Crosstalk_x.y.z_x64_fr-FR.msi` (WiX) : pour un déploiement géré.

Il n'y a rien d'autre à installer, ni Rust ni Node.js. Si le runtime WebView2 manque (rare sous Windows 10, toujours présent sous Windows 11), l'installateur l'installe automatiquement.

## Développement

Prérequis sous Windows 10/11 x64 :

- [Rust](https://rustup.rs) stable (toolchain MSVC) ;
- Node.js 22 ;
- les *Build Tools* de Visual Studio, avec la charge de travail « Développement desktop en C++ » ;
- LLVM (bindgen de whisper.cpp) et CMake.

```powershell
npm install
npm run app:dev     # lance l'application Tauri avec rechargement à chaud
npm test            # tests de l'interface (Vitest)
cd src-tauri; cargo test   # tests du cœur Rust
npm run app:build   # produit le .exe et les installateurs NSIS / MSI
```

Les installateurs sont générés dans `src-tauri/target/release/bundle/` (`nsis/` et `msi/`).

**Validation complète** (aucune CI : tout se vérifie en local) :

```powershell
powershell -ExecutionPolicy Bypass -File scripts\validate.ps1
```

Le script enchaîne garde anti-triche, typecheck, tests, rustfmt, clippy, build MSVC et installateurs, puis écrit `dist-local\<commit>\validation.md`, le rapport à joindre à la Pull Request.

> `npm run dev` sert uniquement l'interface dans un navigateur, pour des retouches visuelles rapides. Le produit livré est toujours l'application Tauri.

## Contribution

- `develop` est la branche de développement et la branche par défaut. `main` est la production.
- Chaque sujet a sa branche (`feat/…`, `fix/…`, `chore/…`), part de `develop` et y revient par Pull Request, accompagnée d'un rapport de validation locale.
- `main` n'accepte que des Pull Requests de release (`develop` → `main`), validées par le propriétaire du dépôt.
- Le projet n'utilise pas GitHub Actions : builds, tests et benchmarks s'exécutent sur un PC Windows. GitHub sert au code, aux branches, aux issues et aux Pull Requests.
- Les publications se font par tag `vx.y.z` sur `main` ; les installateurs, construits en local, sont joints à une Release GitHub en brouillon.

Le détail est dans [CONTRIBUTING.md](CONTRIBUTING.md).
