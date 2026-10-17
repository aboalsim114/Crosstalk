# Consignes pour Claude Code

## Après chaque fusion : publier la mise à jour

Après **chaque fusion** d'une Pull Request (dans `develop`, ou une release dans `main` suivie de `main` → `develop`), sans attendre qu'on le demande :

1. Mettre à jour `develop` et s'y placer, arbre propre :
   ```bash
   git fetch origin +refs/heads/develop:refs/remotes/origin/develop
   git checkout develop && git reset --hard origin/develop
   ```
2. Publier la version de test :
   ```bash
   scripts/publish-dev-build.sh
   ```
3. Vérifier que `latest.json` de la branche `downloads` annonce le nouveau commit et le nouveau numéro de build.

L'application lit ce `latest.json` : la section **À propos** propose alors la mise à jour, installée depuis l'application. Une fusion non publiée n'arrive pas chez l'utilisateur.

`scripts/cross-build-windows-gnu.sh` télécharge les bibliothèques Windows précompilées de sherpa-onnx (`scripts/fetch-sherpa-onnx.sh windows`) et copie leurs DLL à côté de `crosstalk.exe`. Si l'espace disque manque, supprimer `src-tauri/target/debug` et les anciens paquets de `dist-test/`.

## Nouveautés pour les joueurs

Chaque PR qui change quelque chose de visible pour les joueurs ajoute une entrée **en tête** de `release-notes.json` :
- `id` unique (minuscules, chiffres, tirets), `date` AAAA-MM-JJ, `title` court, `items` : 1 à 6 phrases simples ;
- écrit pour un joueur : ce qu'il gagne et comment s'en servir, sans numéro de PR, commit, nom de fichier, « feat/fix » ni lien GitHub ;
- pas de `build` : `scripts/publish-dev-build.sh` attribue le build à la première publication.

L'application affiche ces nouveautés avant la mise à jour (À propos, bannière) et une fois après (« Quoi de neuf ? »). `npm run check:release-notes` vérifie le fichier.

## Règles du dépôt

- Ne jamais modifier `main` directement, ni fusionner sans demande explicite.
- Pas de GitHub Actions : validation locale (voir `CONTRIBUTING.md`).
- `npm run check:anticheat` doit passer : aucune lecture mémoire, injection ou hook de `cs2.exe`.
- Ne jamais affirmer qu'un test a réussi sans l'avoir exécuté.
- Tickets Linear : `/linear-ticket SAB-XX [--dry-run]`, lancé uniquement par l'utilisateur (voir `docs/development/linear-workflow.md`).
