# Tests

## Automatiques (locaux, sans GitHub Actions)

| Contrôle | Commande | Où |
|---|---|---|
| Anti-triche | `npm run check:anticheat` | toutes les sources Rust |
| Architecture | `npm run check:architecture` | cycles Rust et TS, fichiers partagés, couche IPC, contrat IPC |
| Types | `npm run typecheck` | `src/` |
| Interface | `npm test` (Vitest + Testing Library) | `src/**/*.test.ts(x)`, à côté du code testé |
| Cœur Rust | `cd src-tauri && cargo test --all-targets` | modules `#[cfg(test)]` ; gros fichiers : `tests.rs` du module (`speech/service/tests.rs`) |
| Installateur | `cd src-installer && cargo test` | `install.rs`, plus les tests des fichiers partagés |
| Style | `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` | les deux crates |

`scripts/validate.ps1` les enchaîne sous Windows et écrit le rapport `dist-local\<commit>\validation.md`.

### Écrire un test
- **Rust** :
  - les services se testent avec des faux (voir [modules.md](../architecture/modules.md#contrats-des-moteurs-dia)) ;
  - les règles de routage entre services sont des fonctions testées sans Tauri (`wiring.rs`) ;
  - pas d'attente fixe : on attend une condition avec une échéance (voir `settle` dans `translation/service/tests.rs`).
- **Interface** :
  - les règles pures se testent directement ;
  - les composants avec Testing Library ;
  - `@tauri-apps/api` est absent des tests (`isTauri` faux), et un appel natif se simule avec `vi.mock('../../shared/ipc')`.
- **Caractérisation** : avant de déplacer un code risqué, un test fige son comportement actuel, même imparfait. Le défaut est noté dans `docs/architecture/audit.md` (§A16), pas corrigé en même temps.

## Manuels (Windows, CS2)

Ce qu'aucun test automatique ne couvre : la capture réelle de `cs2.exe`, Whisper sur de vraies voix, la latence, le CPU, les FPS, l'overlay en jeu, l'installateur sur une machine propre. Les protocoles sont dans [`docs/testing/`](../testing/), un par fonctionnalité.

**Ils ne sont réussis que s'ils ont été exécutés.** Le rapport de PR dit lesquels l'ont été, sur quelle machine, et lesquels ne l'ont pas été.

## Après une refactorisation

Une PR de refactorisation garde au moins les tests de référence :
- aucun test supprimé sans justification écrite dans la PR ;
- la liste nominative des tests avant et après est comparée ;
- le build de production de l'interface est comparé octet pour octet quand seul l'emplacement des fichiers change.

Voir [refactoring-report.md](../architecture/refactoring-report.md) pour un exemple.
