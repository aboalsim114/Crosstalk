# Contribuer à Crosstalk

## Branches

```
main      (PRODUCTION)   versions validées, prêtes à publier
  ▲
  │  Pull Request de release (develop → main), uniquement avec l'accord du mainteneur
  │
develop   (DÉVELOPPEMENT) branche par défaut, intégration
  ▲
  ├── feat/<sujet>       nouvelle fonctionnalité
  ├── fix/<sujet>        correction
  ├── chore/<sujet>      outillage, dépendances, docs
  ├── release/x.y.z      stabilisation d'une version (optionnel)
  └── hotfix/<sujet>     correctif urgent de production (part de main)
```

| Branche | Rôle | Qui y écrit |
| --- | --- | --- |
| `main` | Production. Ne contient que du code validé et publié. | Uniquement par PR de release, depuis `develop`, `release/*` ou `hotfix/*`. |
| `develop` | Intégration de tout le développement. | Uniquement par PR depuis une branche de travail. |
| `feat/*`, `fix/*`, `chore/*` | Une branche par sujet. | Le développeur. |

Une PR vers `main` qui ne vient pas de `develop`, `release/*` ou `hotfix/*` est refusée par le mainteneur et fermée (aucun contrôle automatique : voir « Validation locale »).

## Cycle d'une fonctionnalité

1. Partir de `develop` à jour :
   ```bash
   git switch develop && git pull
   git switch -c feat/installer-design
   ```
2. Faire des commits clairs, au format [Conventional Commits](https://www.conventionalcommits.org/fr/) : `feat(overlay): …`, `fix(tray): …`, `chore(tooling): …`, `docs: …`, `test: …`.
3. Valider en local sur le commit exact à proposer (voir « Validation locale »).
4. Pousser la branche et ouvrir une **Pull Request vers `develop`**, avec le rapport de validation locale dans la description.
5. Le mainteneur relit le rapport et fusionne lui-même ; rien ne fusionne automatiquement.
6. Fusionner dans `develop` par *merge commit*. Pas de réécriture de l'historique partagé.
7. Une branche dépendante d'une autre peut cibler cette branche (PR empilée). Une fois la première fusionnée, on réoriente la PR suivante vers `develop`.

## Release (develop → main)

1. Sur `develop` (ou sur une branche `release/x.y.z`), passer la version à `x.y.z` dans `package.json`, `src-tauri/Cargo.toml` et `src-tauri/tauri.conf.json`.
2. Ouvrir la PR **`develop` → `main`**, intitulée « Release vx.y.z ».
3. Produire les installateurs avec `scripts\validate.ps1` et les tester sur Windows 10 et 11.
4. **Fusionner uniquement après validation explicite du mainteneur.** Rien ne fusionne automatiquement dans `main`.
5. Depuis `main` à jour, valider et construire la version : `powershell -ExecutionPolicy Bypass -File scripts\validate.ps1 -Release`. Le script vérifie que le commit est sur `origin/main` et que les trois versions concordent, construit les installateurs et affiche les commandes de publication.
6. Taguer le commit de merge sur `main` :
   ```bash
   git switch main && git pull
   git tag vx.y.z && git push origin vx.y.z
   ```
7. Créer la **Release GitHub en brouillon** avec ces installateurs (`gh release create vx.y.z --draft …`, commande affichée par le script), la relire, puis la publier à la main.

## Hotfix

Pour un correctif urgent en production : créer `hotfix/<sujet>` depuis `main`, ouvrir une PR vers `main`, puis fusionner `main` dans `develop` pour ne pas perdre le correctif.

## Validation locale (pas de GitHub Actions)

**Décision du mainteneur (12/10/2026), permanente jusqu'à nouvel ordre** : Crosstalk n'utilise pas GitHub Actions, ni aucun runner hébergé payant. GitHub sert au code, aux branches, aux issues et aux Pull Requests. Toute validation technique s'exécute sur un PC Windows 10/11 x64, par le mainteneur, Codex ou Claude Code. Ne réactivez pas de workflow sans l'accord explicite du mainteneur.

Prérequis : Rust stable (MSVC), Node.js 22, Build Tools Visual Studio (C++), LLVM (`LIBCLANG_PATH`), CMake. Lancez les commandes dans PowerShell, pas dans Git Bash : son `link.exe` masque celui de MSVC.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\validate.ps1              # complet
powershell -ExecutionPolicy Bypass -File scripts\validate.ps1 -SkipBundle  # sans installateurs
```

| Étape | Commande |
| --- | --- |
| Garde anti-triche | `npm run check:anticheat` (aucune API d'accès à un autre processus) |
| Interface | `npm run typecheck`, `npm test`, `npm run build` |
| Rust | `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --all-targets --locked` |
| Windows | `npx tauri build --target x86_64-pc-windows-msvc` (NSIS + MSI) |
| Outils | `crosstalk-audio-probe`, `crosstalk-asr-bench`, `crosstalk-translate` (release, MSVC) |

Le script s'arrête à la première erreur et écrit `dist-local\<commit>\` : exécutables, `SHA256SUMS.txt` et `validation.md`.

Restent manuels, selon le sujet : capture et reconnaissance vocale avec CS2, mesures Whisper (latence, CPU), FPS ; protocoles dans `docs/testing/`.

### Version de test téléchargeable

Après **chaque fusion dans `develop`**, republier la version de test depuis un `develop` à jour et propre :

```bash
scripts/publish-dev-build.sh
```

Le script construit le paquet portable Windows (`scripts/cross-build-windows-gnu.sh`) et le publie sur la branche `downloads`, qui ne contient qu'un commit (le zip et `VERSION.md` : commit, date, SHA-256), remplacé à chaque publication. Le lien du README ne change pas et l'historique du dépôt ne grossit pas. Ne fusionnez jamais `downloads` ailleurs.

### Rapport à joindre à chaque Pull Request

- le commit exact validé et la machine (Windows, processeur, RAM) ;
- les tests exécutés et leur résultat (le contenu de `validation.md` convient) ;
- les essais manuels réellement faits, avec leurs chiffres ;
- **ce qui n'a pas été vérifié**, et pourquoi.

N'indiquez jamais comme réussi un test qui n'a pas été exécuté. Un rapport produit sur un autre commit ne valide pas la PR.
