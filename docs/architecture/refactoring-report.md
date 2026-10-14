# Rapport de refactorisation

Refactorisation de l'architecture de Crosstalk, de `develop` @ `b84f4de` à la branche `refactor/professional-architecture`. **Aucun changement fonctionnel** ([D-11](decisions.md)) : mêmes commandes et événements Tauri, mêmes données persistées, mêmes files et politiques d'abandon, même rendu.

Documents liés : [audit.md](audit.md) (état de départ), [overview.md](overview.md) (cible), [decisions.md](decisions.md).

## 1. Ce qui a changé

### Cœur Rust
| Avant | Après | Audit |
|---|---|---|
| Cycle de 7 modules (`downloads ↔ logs ↔ reply ↔ speaker ↔ speech ↔ translation ↔ updater`) | Aucun cycle entre modules de premier niveau, vérifié par `npm run check:architecture` | §1.2, A3, A4 |
| Colle entre services dans quatre endroits (`lib.rs`, adaptateurs de `speech` et `chat`, commande `translation_configure`) | `wiring.rs`, avec ses règles de routage testées sans Tauri | A3 |
| `speech::models::models_dir`, `speech::cpu`, `updater::current_build` | `paths.rs`, `cpu.rs`, `version.rs` (modules feuilles) | A4 |
| `pipeline/` (traits morts) | supprimé | A5 |
| `speech/service.rs` (1 907 lignes) | `speech/service/{mod, queue, stats, front, asr, tests}.rs` (mod.rs : 498 lignes) | A1 |
| `translation/service.rs` (1 107 lignes) | `translation/engine.rs` (contrats), `translation/opus.rs`, `translation/service/{mod, types, cache, worker, tests}.rs` | A2 |
| Résumé du journal relu par noms de champs, sans test sur les vrais types | test de caractérisation sur `SpeechStatus`, `TranslationStatus`, `CaptureStatus` | A18 |

### Installateur
| Avant | Après | Audit |
|---|---|---|
| `steps` (126 lignes) | `check_space`, `extract_payload`, `commit_staging` ; bornes de phase nommées et vérifiées contre `src/installer/model.ts` | A8 |
| Jamais construit ni testé par `validate.ps1`, non formaté | rustfmt, clippy et tests dans `validate.ps1` ; `cargo fmt` appliqué | A7 |
| `#[path]` sans contrôle | contrôle des imports des fichiers partagés (`check:architecture`) ; `paths.rs` partagé | A7, [D-08](decisions.md) |

### Interface
| Avant | Après | Audit |
|---|---|---|
| `backend/index.ts` (670 lignes, 16 domaines), qui importait un type du store | `shared/ipc/<domaine>.ts`, baril `index.ts`, aucune dépendance vers l'interface | A9 |
| `native.ts` importait `pages/ReplyCard` | `recentLangs` dans `features/languages/langs.ts` | A10 |
| 4 copies du motif « clé → minuterie → configure », minuteries jamais annulées | `debouncedSync` (mêmes délais), annulé à l'arrêt du pont | A11, A12 |
| `pages/`, `state/`, `components/`, `backend/` | `app/`, `features/<nom>/`, `shared/{ipc,i18n,ui}`, `windows/` | [D-09](decisions.md) |

### Outillage et documentation
- `npm run check:architecture` vérifie :
  - les cycles Rust et TypeScript ;
  - les fichiers partagés avec l'installateur ;
  - l'indépendance de la couche IPC ;
  - le contrat IPC figé (`ipc-contract.txt`).
- `docs/architecture/` : audit, vue d'ensemble, modules, dépendances, flux, décisions, ce rapport.
- `docs/development/` : démarrer, conventions, tests.
- `docs/architecture.md` (cible produit) : il renvoie maintenant vers `docs/architecture/`, ses passages périmés sont corrigés.
- Mises à jour du README (arborescence) et de CONTRIBUTING (contrôles).

## 2. Déplacements de fichiers

Tous faits par `git mv`. Le contenu ne change pas, à l'exception des chemins d'import.

**Rust**
- `src-tauri/src/speech/cpu.rs` → `src-tauri/src/cpu.rs`
- `src-tauri/src/speech/service.rs` → `src-tauri/src/speech/service/` (6 fichiers)
- `src-tauri/src/translation/service.rs` → `src-tauri/src/translation/service/` (5 fichiers), plus `translation/engine.rs` et `translation/opus.rs`
- Nouveaux : `paths.rs`, `version.rs`, `wiring.rs`
- Supprimé : `pipeline/`

**Interface** (108 fichiers)

| Avant | Après |
|---|---|
| `src/main.tsx`, `App.tsx`, `config.ts`, `native.ts` (+ tests) | `src/app/` |
| `src/components/{TitleBar,Sidebar,PageHeader,Intro}.tsx` | `src/app/shell/` |
| `src/scene/` | `src/app/scene/` |
| `src/state/{store,data,persist}.ts` (+ tests) | `src/app/state/` |
| `src/pages/AudioPage.tsx`, `src/state/audio.ts` | `src/features/audio/` |
| `src/state/speech.ts` | `src/features/speech/` |
| `src/state/translation.ts` | `src/features/translation/` |
| `src/pages/{LangPage,LanguagesCard,UiLangCard}.tsx`, `src/state/langs.ts` | `src/features/languages/` |
| `src/pages/ChatCard.tsx`, `src/state/chat.ts` | `src/features/chat/` |
| `src/pages/{Dashboard.tsx,derive.ts}` | `src/features/session/` |
| `src/pages/OverlayPage.tsx` | `src/features/overlay/` |
| `src/pages/ReplyCard.tsx`, `src/reply/{ReplyBox,DictationPanel}.tsx` | `src/features/reply/` |
| `src/pages/HistoryPage.tsx`, `src/state/history.ts` | `src/features/history/` |
| `src/pages/{RecapPanel,GsiCard}.tsx`, `src/state/{recap,gsi}.ts`, `src/recap/card.ts` | `src/features/recap/` |
| `src/pages/StreamCard.tsx`, `src/state/stream.ts` | `src/features/stream/` |
| `src/pages/SpeakerCard.tsx`, `src/state/speaker.ts` | `src/features/speaker/` |
| `src/components/UpdateBanner.tsx`, `src/state/update.ts` | `src/features/update/` |
| `src/components/SetupBanner.tsx`, `src/state/setup.ts` | `src/features/setup/` |
| `src/pages/DiagPage.tsx` | `src/features/diagnostics/` |
| `src/pages/AboutPage.tsx` | `src/features/about/` |
| `src/backend/` | `src/shared/ipc/` (découpé) |
| `src/i18n/` | `src/shared/i18n/` |
| `src/components/{atoms,LevelMeter}.tsx` | `src/shared/ui/` |
| `src/overlay/`, `src/reply/{main,ReplyWindow}.tsx` | `src/windows/overlay/`, `src/windows/reply/` |

Les pages `index.html`, `overlay.html` et `reply.html` pointent vers les nouveaux points d'entrée. `src/installer/` ne bouge pas.

## 3. Résultats des tests

Linux (environnement de développement), sans GitHub Actions. Mêmes commandes avant et après.

| Contrôle | Avant (`b84f4de`) | Après |
|---|---|---|
| `cargo fmt --check` (src-tauri) | propre | propre |
| `cargo clippy --all-targets -D warnings` (src-tauri) | propre | propre |
| `cargo test --all-targets` (src-tauri) | 234 réussis | **239 réussis** |
| `cargo fmt --check` (src-installer) | **non conforme** (`install.rs`) | propre |
| `cargo clippy --all-targets -D warnings` (src-installer) | non mesuré | propre |
| `cargo test` (src-installer) | 23 réussis | **24 réussis** |
| `tsc --noEmit` | 0 erreur | 0 erreur |
| `vitest run` | 163 réussis (26 fichiers) | **164 réussis** (26 fichiers) |
| `npm run check:anticheat` | OK | OK |
| `npm run check:architecture` | — (n'existait pas ; 1 cycle Rust de 7 modules) | OK, aucun cycle |
| `vite build` | OK | OK |
| Compilation croisée Windows (MinGW) | — | voir §3.3 |

### 3.1 Tests supprimés, déplacés, ajoutés

Liste nominative comparée avant / après.

**Supprimé (1)**
- `pipeline::translation::tests::skips_lines_already_in_french_when_enabled`. Il testait `needs_translation`, appelée nulle part, dans un module mort supprimé ([D-04](decisions.md)). La règle réelle reste testée : `french_is_never_sent_to_a_model` et `a_russian_user_gets_the_other_languages_in_russian`.

**Déplacés avec leur code (même corps)**
- `speech::cpu::tests::{measures_busy_work, meter_needs_two_samples}` → `cpu::tests::…`
- `speech::models::tests::env_override_wins` → `paths::tests::env_override_wins`. Dans l'installateur : `whisper_models::…` → `paths::…`.
- Les tests de `speech/service.rs` et de `translation/service.rs` sont passés dans `service/tests.rs`. Leurs noms ne changent pas ; seuls leurs imports suivent le code.

**Modifiés (2)**
- `a_russian_user_gets_the_other_languages_in_russian` : il échouait environ une fois sur dix, par une course avec le thread de traduction. Il attend maintenant que les paires installées aient été examinées, et l'assertion ne change pas (40 exécutions sur 40 réussies).
- `ReplyBox.test.tsx` : seul le chemin d'import de `recentLangs` change.

**Ajoutés (8)**
- `wiring::tests` (5) : ordre locuteur → affichage → traduction ; ligne provisoire ; sans service de traduction ; chat ; langue inconnue.
- `logs::tests::summarizes_the_real_status_types` : statuts réels sérialisés.
- `install::tests::phase_bounds_match_the_interface` : bornes de phase Rust = `PH` de l'interface.
- `native.test.tsx` › `debouncedSync` : délais, déduplication, annulation.

### 3.2 Preuves de non-régression
- **Interface**, deux builds comparés octet pour octet avec `diff -r` : avant et après la réorganisation par fonctionnalité (commit `c33ade2`), le build de production (`vite build`, application et installateur) est **identique** (JS, CSS, polices).
- **Rust** : pour les deux services découpés, on a comparé les lignes de code triées, une fois retirés les `pub(super)` et les imports. Les seules différences sont attendues :
  - `translation` : aucune différence ;
  - `speech` : la factorisation `StatsAcc::transcribed`, qui remplace deux copies identiques, et `JobQueue::default()`.
- **Contrat IPC** : 64 commandes de l'application, 8 de l'installateur et 21 événements, inchangés (`ipc-contract.txt`).

### 3.3 Windows
- **Compilation croisée MinGW** : voir le rapport de la Pull Request (installateur et dossier portable produits, ou erreur).
- **Non exécuté** : aucun lancement sous Windows ni dans CS2 depuis cet environnement. Un test manuel est nécessaire pour la capture, la reconnaissance, la traduction, l'overlay et l'installateur (protocoles `docs/testing/`). Les zones qui le méritent le plus :
  - le câblage déplacé dans `wiring.rs` (transcription → locuteur → traduction ; chat → traduction) ;
  - l'installateur (phases extraites).

## 4. Risques restants

| Risque | Mesure |
|---|---|
| Une différence de comportement du câblage que les tests ne voient pas (ordre identique, mais exécuté via `try_state` avant l'émission) | Test manuel en partie : une voix nommée par « Qui parle ? » puis traduite ; un message du chat traduit |
| `noop` est maintenant exporté par la couche IPC | Sans effet ; même valeur |
| La compilation croisée MinGW n'est pas la version officielle (MSVC) | `validate.ps1` sous Windows avant la fusion |
| Défauts fonctionnels connus et non corrigés (F1 à F14) | PR `fix/*` dédiées, une par défaut |

## 5. Ce qui n'a pas été fait (volontairement)

- **Réécrire les `lock().unwrap()`** : inutile avec `panic = "abort"` ([D-06](decisions.md)).
- **Workspace Cargo ou crate partagée** : aucun gain mesuré ([D-08](decisions.md)).
- **Unifier les états en double du frontend** (niveau d'entrée, langue cible, simulation) : cela change le moment des rendus (A13).
- **Découper `OverlayPage`** pour que la fenêtre overlay n'embarque plus le store principal : le rendu en jeu doit d'abord être validé sur Windows (A10).
- **Factoriser les serveurs HTTP de `gsi` et `stream`, et le cache de moteurs de `reply`** (A19).
- **Corriger F1 à F14** : ce sont des changements fonctionnels.

## 6. Commandes pour reproduire

```bash
# graphe des modules Rust (hors tests)
node scripts/check-architecture.mjs
# tests
(cd src-tauri && cargo test --all-targets) && (cd src-installer && cargo test) && npx vitest run
# build identique de l'interface entre deux commits
git worktree add /tmp/avant <commit> && (cd /tmp/avant && npm ci && npx vite build --outDir /tmp/b1)
npx vite build --outDir /tmp/b2 && diff -r /tmp/b1 /tmp/b2
```
