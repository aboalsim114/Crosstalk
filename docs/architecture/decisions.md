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
  | Infrastructure | Système, modèles, réseau, disque | `audio/wasapi.rs`, `speech/engine.rs` (sherpa-onnx), `translation/marian.rs`, `downloads.rs` (`HttpFetcher`) |
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

  Les appelants (dont l'installateur et les exemples `asr_bench` et `translate`) importent directement ces modules : pas de réexport de compatibilité, la crate n'a pas d'utilisateur externe. Le calcul du chemin ne change pas.
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
- **Suite** : [D-14](#d-14--reconnaissance-vocale--parakeet-tdt-06b-v3-via-sherpa-onnx-whisper-retiré) a remplacé whisper.cpp par sherpa-onnx derrière les mêmes traits (`Recognizer`, `EngineLoader`, `VoiceDetector`). Le volet GPU de ces contrats n'existe plus.

## D-13 — Whisper en priorité normale
- **Contexte** : `speech-asr` tournait en priorité « inférieure à la normale » pour laisser le processeur à CS2. Journal du 16 octobre (build 276, whisper-base, processeur seul) : calcul de 1,2× à 29× la durée de voix, la plupart des segments jetés « trop tard ». whisper.cpp découpe chaque étape entre le fil appelant et ses fils de travail, qui l'attendent à chaque barrière : le fil appelant, affamé par CS2, ralentissait tout le calcul.
- **Choix** : `speech-asr` reste en priorité normale, comme le jeu ; le nombre de fils de calcul reste limité (moitié des processeurs logiques, 4 au plus). La traduction (requêtes au LLM) garde la priorité basse.
- **Remise en cause** : une baisse de FPS mesurée dans CS2 ; on réduirait alors le nombre de fils plutôt que la priorité.
- **Depuis D-14** : volet Whisper remplacé par D-14 (whisper.cpp retiré) ; le choix de priorité reste appliqué tel quel à sherpa-onnx / ONNX Runtime (`speech-asr` en priorité normale, moitié des processeurs logiques, de 1 à 4 fils). Son effet avec le nouveau moteur n'a pas été mesuré dans CS2.

## D-14 — Reconnaissance vocale : Parakeet TDT 0.6B v3 via sherpa-onnx, Whisper retiré
- **Contexte** : Whisper (whisper.cpp via `whisper-rs`, modèles `ggml-*`) demandait CMake et LLVM pour le build, une option GPU Vulkan difficile à livrer en MSVC (CT-002), une bascule automatique sur whisper-base quand le modèle choisi prenait du retard, et inventait du texte sur les bruits de combat. Un banc synthétique a comparé les moteurs locaux gratuits : 517 extraits, voix Piper (ru, en, fr) et bruit de combat synthétique, sur le processeur d'un conteneur (pas l'i7-1165G7 de référence). Détails : [rapport](../reports/asr-engine-replacement.md).
- **Résultats** :
  - WER : Parakeet TDT 0.6B v3 46,6 % ; FastConformer multilingue 57,2 % ; whisper-base 60,7 % ;
  - bruit seul : 1 fausse transcription sur 50 extraits pour Parakeet, 50 sur 50 pour whisper-base ;
  - p50 ~480 ms par extrait avec 4 fils, ~0,8 Go de mémoire, chargement ~6 s.
- **Choix** :
  - reconnaissance : NVIDIA Parakeet TDT 0.6B v3, export int8 ONNX (`parakeet-tdt-0.6b-v3-int8`, ~671 Mo, CC-BY-4.0, 25 langues européennes dont ru, en, fr, de, es, uk, pl ; **pas le turc**) ;
  - VAD : Silero VAD ONNX (`silero_vad.onnx`, MIT) ;
  - exécution : sherpa-onnx 1.13.8 (crates officielles, Apache-2.0), API C des bibliothèques précompilées et ONNX Runtime (MIT), DLL livrées à côté de `crosstalk.exe` ; processeur seul, moitié des processeurs logiques, de 1 à 4 fils ;
  - la langue est lue dans le texte transcrit, parmi les langues autorisées (`chat::lang::detect`) ;
  - Whisper est retiré entièrement : plus d'option GPU, plus de bascule, plus de moteur de secours, plus d'invite ni de glossaire pour la reconnaissance. Les anciens `ggml-*.bin` ne sont jamais supprimés d'office : Audio & IA propose « Libérer l'espace ».
- **Écartés** :
  - Qwen3-ASR : sortie en chinois sur de courts extraits russes, 2 à 3 s par extrait ;
  - Omnilingual 300M : translittère le russe en alphabet latin ;
  - GigaAM v2 : russe seulement ;
  - SenseVoice, Canary 180M : pas de russe ;
  - Vosk : téléchargement bloqué dans l'environnement de test, modèles plus faibles.
- **Licences** : vérifiées pour le modèle, la VAD, sherpa-onnx et ONNX Runtime.
- **Non vérifié** : reconnaissance avec CS2 lancé, latence et CPU en partie, FPS, voix réelles de joueurs.
- **Remise en cause** : un WER ou des fausses transcriptions nettement moins bons en partie réelle, un temps de calcul qui ne suit pas avec CS2 lancé, ou un besoin du turc à la voix.
