## Résumé

<!-- Quoi et pourquoi, en quelques lignes. -->

## Type

- [ ] Fonctionnalité (`feat/*` → `develop`)
- [ ] Correction (`fix/*` → `develop`)
- [ ] Outillage / docs (`chore/*` → `develop`)
- [ ] Release (`develop` → `main`), validée par le mainteneur

## Validation locale

Pas de GitHub Actions : tout est vérifié sur un PC Windows (voir CONTRIBUTING.md, « Validation locale »).

- Commit validé : <!-- hash complet -->
- Machine : <!-- Windows build, processeur, RAM -->

- [ ] `scripts\validate.ps1` (anti-triche, typecheck, tests UI, build, rustfmt, clippy, tests Rust, build MSVC, installateurs) : coller `validation.md`
- [ ] Essais manuels sous Windows / CS2, si le comportement natif change : protocole suivi et chiffres

### Non vérifié

<!-- Ce qui n'a pas été exécuté, et pourquoi. Ne jamais cocher un test non exécuté. -->

## Notes pour la revue

<!-- Risques, limites, captures d'écran. -->
