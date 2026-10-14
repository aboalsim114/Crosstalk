# Décisions d'architecture

Chaque décision indique le contexte, le choix et ce qui le remettrait en cause. Les numéros sont stables : on ajoute, on ne renumérote pas.

## D-01 — Monolithe modulaire, une seule crate pour l'application
- **Contexte** : `src-tauri` est une crate de 21 000 lignes. Les modules (`audio`, `speech`, `translation`, `chat`…) sont déjà séparés et communiquent par des traits.
- **Choix** : on garde une crate unique, découpée en modules. Pas de workspace multi-crates, pas de microservices, pas de système de plugins.
- **Remise en cause** : un temps de compilation incrémental mesuré comme gênant, ou un module à publier séparément.

## D-02 — Couches conceptuelles, pas de dossiers par couche
- **Contexte** : une arborescence `domain/ application/ infrastructure/` éparpillerait chaque fonctionnalité dans quatre dossiers.
- **Choix** : les couches sont une **règle de dépendance** à l'intérieur de chaque module métier :

  | Couche | Rôle | Exemples |
  |---|---|---|
  | Domaine | Règles pures, sans E/S ni thread | `speech/segmenter.rs`, `speech/filter.rs`, `translation/models.rs`, `translation/glossary.rs`, `chat/lang.rs`, `chat/parser.rs` |
  | Application | Services : threads, files bornées, purge, état | `speech/service/`, `translation/service/`, `audio/service.rs`, `chat/mod.rs` (`ChatService`) |
  | Infrastructure | Système, modèles, réseau, disque | `audio/wasapi.rs`, `speech/engine.rs` (whisper.cpp), `translation/marian.rs`, `downloads.rs` (`HttpFetcher`) |
  | IPC | Commandes `#[tauri::command]` et adaptateurs `Tauri*Events` | `<module>/mod.rs` |
  | Composition | Création des services et colle entre eux | `lib.rs`, `wiring.rs` |

  Une couche ne dépend que des couches au-dessus d'elle dans ce tableau. Le domaine et l'application ne connaissent pas Tauri : ils publient par un trait (`SpeechEvents`, `TranslationEvents`, `ChatEvents`…).
- **Remise en cause** : —

## D-03 — Un module de composition (`wiring.rs`)
- **Contexte** : la colle entre services était répartie entre `lib.rs` (`CaptureEvents`), l'adaptateur Tauri de `speech`, celui de `chat` et la commande `translation_configure`. C'est elle qui créait le cycle `speech ↔ translation ↔ reply` ([audit A3](audit.md)).
- **Choix** : `wiring.rs` est le seul endroit où un service en appelle un autre en réaction à un événement. Les règles de routage (nommer le locuteur, puis émettre, puis traduire) sont des fonctions testées sans Tauri. Les modules métier ne dépendent plus les uns des autres pour ces réactions.
- **Remise en cause** : —

## D-04 — Suppression du module `pipeline/`
- **Contexte** : des traits jamais branchés (DeepL, Azure), sous `#[allow(dead_code)]` ([audit A5](audit.md)).
- **Choix** : suppression. Le contrat réel de traduction est `translation::Engine`. Le test supprimé (`skips_lines_already_in_french_when_enabled`) portait sur `needs_translation`, une fonction appelée nulle part. La règle réelle (une ligne déjà dans la langue de l'utilisateur est affichée telle quelle) est testée dans le service de traduction (`french_is_never_sent_to_a_model`, `a_russian_user_gets_the_other_languages_in_russian`).
- **Remise en cause** : —

## D-05 — `paths.rs`, `cpu.rs` et `version.rs` au premier niveau
- **Contexte** : le dossier des modèles, la priorité des threads et le numéro de build sont utilisés par plusieurs modules. Ils étaient rangés dans `speech` et `updater`, ce qui créait des cycles.
- **Choix** : des modules feuilles, sans dépendance interne :
  - `paths::models_dir()` ;
  - `cpu` : temps CPU du processus, priorité basse, `CpuMeter` ;
  - `version::current_build()`.

  `speech::models::models_dir` reste disponible (réexport), pour l'installateur et l'exemple `asr_bench`. Le calcul du chemin ne change pas.
- **Remise en cause** : —

## D-06 — On garde `lock().unwrap()`
- **Contexte** : 155 `lock().unwrap()` en production ([audit §1.3](audit.md)).
- **Choix** : on ne les réécrit pas. La release est compilée avec `panic = "abort"` (`src-tauri/Cargo.toml`). Un mutex n'est empoisonné que par une panique, qui a déjà terminé le processus. Récupérer l'empoisonnement (`PoisonError::into_inner`) ne changerait le comportement qu'en debug, et cacherait la panique d'origine. Les `expect` d'invariant gardent un message qui explique pourquoi ils ne peuvent pas échouer.
- **Remise en cause** : passage à `panic = "unwind"` en release.

## D-07 — Services découpés en dossiers, API publique inchangée
- **Contexte** : `speech/service.rs` (1 907 lignes) et `translation/service.rs` (1 107 lignes).
- **Choix** : `speech/service/{mod,queue,stats,front,asr}.rs` et `translation/service/{mod,config,cache,worker}.rs`. Les contrats des moteurs de traduction (`Engine`, `EngineLoader`, `Chain`) passent dans `translation/engine.rs`. Les chemins publics (`crosstalk_lib::speech::service::SpeechService`, `crosstalk_lib::translation::service::Engine`…) restent valides par réexport : les exemples et les tests ne changent pas. Le code est **déplacé**, pas réécrit. Les seuls blocs factorisés sont les copies strictement identiques.
- **Remise en cause** : —

## D-08 — Installateur : on garde `#[path]`, contrôlé par un script
- **Contexte** : l'installateur compile `downloads.rs`, `paths.rs` et les deux catalogues de l'application par `#[path]` ([audit A7](audit.md)). Les options sont une crate partagée (`crates/models`) ou un workspace Cargo.
- **Choix** : on garde `#[path]`. Coût de chaque option :
  - **Crate partagée** : un troisième `Cargo.toml` et des dépendances (`ureq`, `sha1_smol`, `serde`) à aligner entre trois manifestes.
  - **Workspace** : la fusion des profils et l'unification des fonctionnalités, qui ferait entrer `tauri/tray-icon` et la liste des API `windows` dans le binaire de l'installateur (compilé en `opt-level = "s"`).

  Le seul défaut concret de `#[path]` est l'absence de contrôle. On le corrige :
  - `scripts/check-architecture.mjs` vérifie que les fichiers partagés n'importent que des fichiers partagés ;
  - `validate.ps1` lance les tests de `src-installer`.
- **Remise en cause** : un quatrième fichier partagé, ou un besoin de dépendances différentes entre l'application et l'installateur.

## D-09 — Frontend : par fonctionnalité, appliqué progressivement
- **Contexte** : `src/` est organisé par nature technique (`pages/`, `state/`, `components/`, `backend/`). Une fonctionnalité comme le chat est répartie dans `backend/index.ts`, `state/chat.ts`, `pages/ChatCard.tsx` et `native.ts`.
- **Choix** : la cible est `app/`, `features/<fonctionnalité>/`, `shared/` et `windows/` (voir [overview.md](overview.md)). Ordre d'application :
  1. découper la couche IPC (`backend/<domaine>.ts`, avec `backend/index.ts` comme baril) ;
  2. corriger les inversions de couches ;
  3. déplacer les fichiers par `git mv`, sans modifier leur contenu hormis les chemins d'import.

  `src/installer/` garde son emplacement, imposé par `installer.html` et `vite.installer.config.ts`.
- **Remise en cause** : —

## D-10 — Contrat IPC figé dans le dépôt
- **Contexte** : une refactorisation peut renommer une commande Tauri ou un événement sans que `tsc` le voie, parce que les noms sont des chaînes.
- **Choix** : `docs/architecture/ipc-contract.txt` liste les commandes (application et installateur) et les événements. `npm run check:architecture` échoue si la liste change. Un changement voulu se fait explicitement avec `--write`, et apparaît dans la PR.
- **Remise en cause** : —

## D-11 — Aucun changement fonctionnel dans une PR de refactorisation
- **Contexte** : l'audit a relevé des défauts réels (F1 à F13).
- **Choix** : ils sont listés dans [audit.md §A16](audit.md) et seront corrigés dans des PR `fix/*` dédiées, chacune avec son test. Une PR de refactorisation ne change ni un résultat ni un message, ni l'ordre d'un événement, ni une capacité de file.
- **Remise en cause** : —

## D-12 — Contrats des moteurs d'IA : les traits existants, documentés
- **Contexte** : la mission demande des moteurs remplaçables (VAD, reconnaissance, amélioration, traduction, modèles, téléchargements, CPU/GPU, diagnostic), sans plateforme de plugins.
- **Choix** : les seams existent déjà et sont testés avec des faux :
  - `VoiceDetector` ;
  - `Recognizer` et `speech::EngineLoader` ;
  - `Engine` et `translation::EngineLoader` ;
  - `Fetcher` et `DownloadEvents` ;
  - `CaptureBackend`.

  On les documente ([modules.md](modules.md#contrats-des-moteurs-dia)) et on les rend visibles : `translation/engine.rs`. On n'en ajoute pas sans second moteur réel. TranslateGemma (PR #60, en attente) s'est branché sur `translation::EngineLoader` sans modifier le service, ce qui montre que le contrat suffit. Les fichiers de modèles installés et leurs chemins ne changent pas.
- **Remise en cause** : un second moteur de reconnaissance vocale.
