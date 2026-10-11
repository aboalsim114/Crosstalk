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
| `design/` | Maquette d'origine (Claude Design), cahier des charges et transcriptions de conception. Sert de référence uniquement. |

## Contribution

- Aucun développement directement sur `main`.
- Une branche par fonctionnalité (`feat/…`, `chore/…`, `fix/…`), puis une Pull Request.
- Aucune fusion sans validation du propriétaire du dépôt.
