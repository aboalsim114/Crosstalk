# Crosstalk

Crosstalk traduit en temps réel, en français, les voix et le chat de vos coéquipiers dans **Counter-Strike 2**. Les sous-titres s'affichent dans un overlay discret.

Crosstalk est une **application de bureau Windows** (Windows 10 et 11, x64), construite avec **Tauri v2** :

- **Cœur applicatif en Rust** : processus, fonctionnalités système, puis la capture audio, la reconnaissance vocale et la traduction.
- **Interface en React + TypeScript**, affichée par WebView2.

Ce n'est pas un site web : l'application se lance depuis un `.exe` et s'installe avec un installateur `.exe` (NSIS) ou `.msi`.

> Crosstalk est un outil externe. Il n'injecte rien dans `cs2.exe`, ne lit pas la mémoire du jeu et n'intercepte pas le trafic réseau.

## État du projet

Phase 1 : l'interface complète, les animations et les interactions fonctionnent avec des données simulées. L'architecture Rust est prête pour la capture audio, la traduction et l'intégration CS2.

## Dépôt

| Dossier | Contenu |
| --- | --- |
| `src-tauri/` | Cœur Rust : application Tauri v2, commandes natives, configuration de l'installateur. |
| `src/` | Interface React + TypeScript (Tailwind CSS, Motion, Three.js), embarquée dans l'exécutable. |
| `design/` | Maquette d'origine (Claude Design), cahier des charges et transcriptions de conception. Sert de référence uniquement. |
| `.github/workflows/` | CI GitHub Actions sur runner Windows : tests, puis génération des installateurs. |

## Installation (utilisateurs)

Téléchargez l'installateur depuis les artefacts de la CI (`crosstalk-windows-x64`) ou depuis une Release :

- `Crosstalk_x.y.z_x64-setup.exe` (NSIS) : installation pour l'utilisateur courant, sans droits administrateur ;
- `Crosstalk_x.y.z_x64_fr-FR.msi` (WiX) : pour un déploiement géré.

Il n'y a rien d'autre à installer, ni Rust ni Node.js. Si le runtime WebView2 manque (rare sous Windows 10, toujours présent sous Windows 11), l'installateur l'installe automatiquement.

## Développement

Prérequis sous Windows 10/11 x64 :

- [Rust](https://rustup.rs) stable (toolchain MSVC) ;
- Node.js 22 ;
- les *Build Tools* de Visual Studio, avec la charge de travail « Développement desktop en C++ ».

```powershell
npm install
npm run app:dev     # lance l'application Tauri avec rechargement à chaud
npm test            # tests de l'interface (Vitest)
cd src-tauri; cargo test   # tests du cœur Rust
npm run app:build   # produit le .exe et les installateurs NSIS / MSI
```

Les installateurs sont générés dans `src-tauri/target/release/bundle/` (`nsis/` et `msi/`).

> `npm run dev` sert uniquement l'interface dans un navigateur, pour des retouches visuelles rapides. Le produit livré est toujours l'application Tauri.

## Contribution

- Aucun développement directement sur `main`.
- Une branche par fonctionnalité (`feat/…`, `chore/…`, `fix/…`), puis une Pull Request.
- Aucune fusion sans validation du propriétaire du dépôt.
