# Contribuer à Crosstalk

## Branches

```
main      (PRODUCTION)   versions validées, prêtes à publier, protégée
  ▲
  │  Pull Request de release (develop → main), uniquement avec l'accord du mainteneur
  │
develop   (DÉVELOPPEMENT) branche par défaut, intégration continue
  ▲
  ├── feat/<sujet>       nouvelle fonctionnalité
  ├── fix/<sujet>        correction
  ├── chore/<sujet>      outillage, CI, dépendances, docs
  ├── release/x.y.z      stabilisation d'une version (optionnel)
  └── hotfix/<sujet>     correctif urgent de production (part de main)
```

| Branche | Rôle | Qui y écrit |
| --- | --- | --- |
| `main` | Production. Ne contient que du code validé et publié. | Uniquement par PR de release, depuis `develop`, `release/*` ou `hotfix/*`. |
| `develop` | Intégration de tout le développement. | Uniquement par PR depuis une branche de travail. |
| `feat/*`, `fix/*`, `chore/*` | Une branche par sujet. | Le développeur. |

Le job CI `release-gate` refuse toute PR vers `main` qui ne vient pas de `develop`, `release/*` ou `hotfix/*`.

## Cycle d'une fonctionnalité

1. Partir de `develop` à jour :
   ```bash
   git switch develop && git pull
   git switch -c feat/installer-design
   ```
2. Faire des commits clairs, au format [Conventional Commits](https://www.conventionalcommits.org/fr/) : `feat(overlay): …`, `fix(tray): …`, `chore(ci): …`, `docs: …`, `test: …`.
3. Vérifier en local avant de pousser :
   ```bash
   npm run typecheck && npm test && npm run build
   cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
   ```
4. Pousser la branche et ouvrir une **Pull Request vers `develop`**.
5. La CI (`windows-build`) doit être verte : tests, lint, installateurs NSIS et MSI.
6. Fusionner dans `develop` par *merge commit*. Pas de réécriture de l'historique partagé.
7. Une branche dépendante d'une autre peut cibler cette branche (PR empilée). Une fois la première fusionnée, on réoriente la PR suivante vers `develop`.

## Release (develop → main)

1. Sur `develop` (ou sur une branche `release/x.y.z`), passer la version à `x.y.z` dans `package.json`, `src-tauri/Cargo.toml` et `src-tauri/tauri.conf.json`.
2. Ouvrir la PR **`develop` → `main`**, intitulée « Release vx.y.z ».
3. Faire tester l'installateur produit par la CI (artefact `crosstalk-windows-x64`) sur Windows 10 et 11.
4. **Fusionner uniquement après validation explicite du mainteneur.** Rien ne fusionne automatiquement dans `main`.
5. Taguer le commit de merge sur `main` :
   ```bash
   git switch main && git pull
   git tag vx.y.z && git push origin vx.y.z
   ```
6. Le workflow `Release` vérifie que le tag est sur `main` et correspond à la version. Il construit les installateurs et crée une **Release GitHub en brouillon**, qu'il reste à relire et à publier.

## Hotfix

Pour un correctif urgent en production : créer `hotfix/<sujet>` depuis `main`, ouvrir une PR vers `main`, puis fusionner `main` dans `develop` pour ne pas perdre le correctif.
