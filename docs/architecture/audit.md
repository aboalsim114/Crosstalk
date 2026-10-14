# Audit d'architecture — Crosstalk

Audit réalisé sur `develop` @ `b84f4de` (14 octobre 2026), avant toute refactorisation. Chaque constat est **mesuré** dans le dépôt : fichier, ligne, commande. Aucune valeur n'est estimée.

Légende :
- **Gravité** : 🔴 haute (risque de bug ou de régression réel), 🟠 moyenne (frein à l'évolution), 🟢 basse (confort, lisibilité).
- **Priorité** : P1 (traité dans cette refactorisation), P2 (traité si le risque reste faible), P3 (documenté, reporté).

## 0. Référence avant refactorisation

Commandes lancées sur `b84f4de`, sous Linux, sans GitHub Actions :

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` (src-tauri) | propre |
| `cargo clippy --all-targets -- -D warnings` (src-tauri) | propre |
| `cargo test --all-targets` (src-tauri) | 234 réussis |
| `cargo test` (src-installer) | 23 réussis |
| `tsc --noEmit` | 0 erreur |
| `vitest run` | 163 réussis (26 fichiers) |
| `npm run check:anticheat` | OK |

La liste nominative des tests de référence (Rust, installateur, Vitest) sert à vérifier qu'aucun test ne disparaît pendant la refactorisation.
Les chiffres complets, avant et après, sont dans [`refactoring-report.md`](refactoring-report.md).

## 1. Mesures

### 1.1 Taille des fichiers (lignes, tests inclus)

| Fichier | Lignes | dont production | Remarque |
|---|---|---|---|
| `src-tauri/src/speech/service.rs` | 1 907 | 1 147 | file de travaux, boucle VAD, boucle ASR, statistiques, chargement du modèle, API publique |
| `src-tauri/src/translation/service.rs` | 1 107 | 762 | configuration, cache LRU, file, boucle de traduction, lexique, chargement Opus-MT, moteur `Chain` |
| `src-tauri/src/logs.rs` | 812 | 648 | journal, panique, rapport de diagnostic, commandes |
| `src-installer/src/install.rs` | 731 | 425 | types, politique, espace libre, extraction, téléchargements, registre, raccourcis |
| `src-tauri/src/downloads.rs` | 676 | 408 | partagé avec l'installateur |
| `src/installer/Installer.tsx` | 714 | — | 4 écrans + atomes + progression |
| `src/backend/index.ts` | 670 | — | toutes les commandes IPC de 16 domaines |
| `src/pages/AudioPage.tsx` | 465 | — | 6 cartes |
| `src/state/store.ts` | 453 | — | session, fil, simulateur, réglages |
| `src/native.ts` | 275 | — | 4 ponts « config → configure » copiés |

### 1.2 Dépendances entre modules Rust (`src-tauri/src`, premier niveau)

Graphe obtenu par recherche de `crate::<module>` dans chaque module, tests exclus. Trois cycles :

| Cycle | Arête aller | Arête retour |
|---|---|---|
| `logs ↔ speech` | `logs.rs:635` lit `crate::speech::models::models_dir()` | `speech/gpu_guard.rs:187,204` écrit dans `crate::logs` |
| `speech ↔ translation` | `speech/mod.rs:194-204` soumet chaque transcription à `TranslationService` | `translation/mod.rs:39,75` (`models_dir`) et `translation/service.rs:494` (`speech::cpu::lower_current_thread_priority`) |
| `reply ↔ translation` | `translation/mod.rs:60` appelle `ReplyService::set_languages` | `reply/` utilise `translation::models`, le glossaire et `Engine` |

Cycles plus longs, qui passent par les mêmes arêtes :
- `speech → speaker → reply → translation → speech` ;
- `speech (gpu_guard.rs:301 → updater) → downloads → speech::models`.

Autres dépendances transverses :
- `speech/gpu_guard.rs` → `settings`, `updater` ;
- `chat/mod.rs:333` → `speaker` et `chat/mod.rs:341` → `translation::models` ;
- `gsi` et `logs` → `history`, uniquement pour `now_ms` et `utc`.

Les arêtes « aller » de `speech`, `chat` et `gsi` vers les autres services passent toutes par leurs adaptateurs Tauri (`TauriEvents`, `TauriChatEvents`, `TauriGsiEvents`). Les cœurs `speech/service.rs` et `translation/service.rs` ne dépendent pas de Tauri : ils publient par des traits (`SpeechEvents`, `TranslationEvents`).

### 1.3 `unwrap` / `expect` en production (hors `#[cfg(test)]`)

163 `unwrap()` et 25 `expect()`. Répartition :

| Catégorie | Nombre | Exemple |
|---|---|---|
| `Mutex::lock().unwrap()` et `Condvar::wait*().unwrap()` | 149 + 6 | `speech/service.rs:151-156`, `translation/service.rs:512-517` |
| `thread::Builder::spawn().expect(...)` | 12 | `speech/service.rs:405,413,547`, `chat/mod.rs:228` |
| Invariant local prouvé par le code juste au-dessus | 8 | `translation/service.rs:177` (`"present"`), `spm.rs:188` |
| Données embarquées à la compilation | 2 | `gsi/mod.rs:44` (`maps.json`), `chat/parser.rs:86` |
| `tauri::Builder::run().expect` | 1 | `lib.rs:274` |

Point décisif : `src-tauri/Cargo.toml:98` compile la release avec `panic = "abort"`. Un mutex ne peut être empoisonné que par une panique pendant qu'il est tenu, et cette panique termine déjà le processus. **En release, aucun `lock().unwrap()` ne peut échouer.** Voir la décision [D-06](decisions.md).

### 1.4 Frontend

- **Cycles d'import** : aucun cycle à l'exécution (algorithme de Tarjan sur les imports relatifs non `type`). Des cycles **de types seulement** existent (`store.ts ↔ audio.ts`, `store.ts ↔ backend/index.ts`) ; TypeScript les efface, mais ils couplent la couche IPC au store UI.
- **`any`** : 0. **`as never`** : 0 en production. `as unknown as` : 2 (`state/persist.ts:82,85`).
- **Abonnements** : chaque pont lancé par `App.tsx:59-86` renvoie sa fonction de désabonnement et `App.tsx` les appelle toutes.

## 2. Constats

### A1 — `speech/service.rs` : un fichier, six responsabilités
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** : 1 147 lignes de production mêlent la file de travaux bornée (`JobQueue`, l. 108-168), l'accumulation des statistiques (`StatsAcc`, l. 181-215), l'état partagé (`Inner`, l. 228-338), l'API publique (`SpeechService`, l. 340-583), le puits audio temps réel (`SpeechSink`, l. 585-636), la boucle VAD (`front_loop`, `run_vad`, l. 638-866) et la boucle ASR (`asr_loop`, `transcribe*`, l. 873-1147). Une modification de la boucle ASR oblige à relire le chemin temps réel, et inversement.
- **Fichiers** : `src-tauri/src/speech/service.rs`
- **Correction** : `speech/service/` avec `mod.rs` (API publique et `Inner`), `queue.rs`, `stats.rs`, `front.rs` (puits et VAD), `asr.rs`. Tests déplacés tels quels dans `service/tests.rs`. Aucune signature publique ne change.
- **Risque de régression** : faible si le code est **déplacé**, pas réécrit. Les 760 lignes de tests existants couvrent la file, la purge, les partiels et le repli.

### A2 — `translation/service.rs` : configuration, cache, file, moteurs dans un seul fichier
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** : 762 lignes de production. Le cache LRU (l. 160-200), la configuration et son assainissement, la boucle de traduction, le chargement Opus-MT (`OpusLoader`) et le moteur pivot `Chain` (l. 700-760) sont mélangés. `Engine` et `EngineLoader` sont déjà des traits : le service est conçu pour des moteurs interchangeables, mais ces contrats sont noyés dans l'implémentation.
- **Fichiers** : `src-tauri/src/translation/service.rs`
- **Correction** : `translation/service/` avec `mod.rs` (service), `config.rs`, `cache.rs`, `worker.rs` (boucle et file) ; `translation/engine.rs` pour les contrats `Engine`, `EngineLoader` et le moteur `Chain` ; `translation/opus.rs` pour `OpusLoader`.
- **Risque de régression** : faible si le code est déplacé à l'identique. Les tests existants couvrent le cache par cible, la purge, le pivot et le lexique.

### A3 — Cycle `speech ↔ translation` et colle entre services dans les modules métier
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** : le récepteur Tauri de `speech` (`speech/mod.rs:170-215`) nomme le locuteur (`speaker`) puis soumet la transcription à la traduction. La reconnaissance vocale connaît donc la traduction et l'identification du locuteur. `lib.rs` contient déjà une colle du même genre (`CaptureEvents`, l. 50-68). Il y a deux endroits pour câbler les services.
- **Fichiers** : `src-tauri/src/speech/mod.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/translation/mod.rs`, `src-tauri/src/translation/service.rs`
- **Correction** : un module de composition `src-tauri/src/wiring.rs` qui regroupe toute la colle entre services :
  - `CaptureEvents` ;
  - le récepteur de transcriptions et d'activité vocale (`speech`) ;
  - le récepteur des messages du chat (`chat/mod.rs:327-350`).

  `speech` et `chat` n'émettent plus que leurs propres événements, toujours par leurs traits. `TauriGsiEvents` reste dans `gsi` (P3) : il ne crée pas de cycle par lui-même.
- **Risque de régression** : moyen. L'ordre des actions doit rester identique : nom du locuteur, émission `speech://transcript`, puis soumission (partielle ou finale) à la traduction. Couvert par un test de caractérisation ajouté avant le déplacement.

### A4 — `models_dir()` et la priorité des threads sont rangés dans `speech`
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** : `speech::models::models_dir()` désigne le dossier de **tous** les modèles (Whisper, Opus-MT, VAD). `translation`, `reply`, `logs` et l'installateur l'importent depuis `speech`. De même, `speech/cpu.rs` (temps CPU du processus, priorité basse du thread, `CpuMeter`) n'a rien de propre à la parole. C'est la cause des cycles `logs ↔ speech` et `translation → speech`.
- **Fichiers** : `speech/models.rs:86`, `speech/cpu.rs`, `translation/mod.rs:39,75`, `translation/service.rs:494`, `logs.rs:635`, `lib.rs:226`, `src-installer/src/main.rs:52`, `examples/asr_bench.rs:38`
- **Correction** : `src-tauri/src/paths.rs` (module feuille, sans dépendance interne) pour `models_dir()` ; `git mv speech/cpu.rs cpu.rs`. `speech::models` réexporte `models_dir` pour la compatibilité de l'API publique. L'installateur inclut `paths.rs` comme il inclut déjà `downloads.rs`.
- **Risque de régression** : faible. Le chemin calculé est inchangé (même code, même variable `CROSSTALK_MODELS_DIR`) ; un test le vérifie.

### A5 — Module `pipeline/` mort
- **Gravité** : 🟢 · **Priorité** : P1
- **Explication** : `pipeline/mod.rs` (42 lignes) et `pipeline/translation.rs` (26 lignes) décrivent des traits « pour les phases à venir » (DeepL, Azure). Ils ne sont utilisés nulle part : seule la déclaration `mod pipeline;` existe (`lib.rs:31`, sous `#[allow(dead_code)]`). La traduction réelle est locale et passe par `translation::service::Engine`. `docs/architecture.md` §3 et §7 présentent encore ces traits comme le pipeline réel. Ce module trompe le lecteur sur la conception.
- **Fichiers** : `src-tauri/src/pipeline/`, `src-tauri/src/lib.rs:13,29-31`, `docs/architecture.md`
- **Correction** : suppression, preuve à l'appui (`grep -rn "pipeline::" src-tauri` ne trouve aucun usage). Le seul test du module (`needs_translation`) teste une fonction qui n'est appelée nulle part. Sa règle (ne pas traduire une ligne déjà dans la langue cible) est implémentée et testée dans `translation/service.rs`. Voir [D-04](decisions.md).
- **Risque de régression** : nul (code non compilé dans l'application).

### A6 — `lock().unwrap()` en masse
- **Gravité** : 🟢 · **Priorité** : P3
- **Explication** : 155 occurrences, mais inaccessibles en release (`panic = "abort"`, voir §1.3). Les remplacer par une récupération (`unwrap_or_else(PoisonError::into_inner)`) changerait le comportement en debug et dans les tests, et masquerait une panique au lieu de la signaler.
- **Fichiers** : surtout `speech/service.rs` (62), `translation/service.rs` (20), `speaker/` (15)
- **Correction** : aucune réécriture en masse. Les nouveaux modules gardent la convention existante, documentée dans `docs/development/conventions.md`. Les `expect` d'invariant ont déjà un message.
- **Risque de régression** : —

### A7 — Installateur : réutilisation par `#[path]` non vérifiée par la validation officielle
- **Gravité** : 🔴 · **Priorité** : P1
- **Explication** : `src-installer/src/main.rs:19-34` compile tels quels `downloads.rs` et les deux catalogues de l'application, avec deux modules d'adaptation (`mod speech { use crate::whisper_models as models; }`). Si l'un de ces fichiers importe un autre module de l'application (`crate::logs`, `crate::settings`…), l'installateur ne compile plus. Or `scripts/validate.ps1` ne construit ni ne teste jamais `src-installer`. Ses tests (`install.rs:426-731`) ne sont lancés par aucun script.
- **Fichiers** : `src-installer/src/main.rs`, `scripts/validate.ps1`
- **Correction** : ajouter `cargo test` de `src-installer` à `validate.ps1`, et un script local `scripts/check-architecture.mjs` qui vérifie que les fichiers partagés n'importent que des modules autorisés. Le passage à une crate partagée (`crates/models`) est évalué dans [D-08](decisions.md) et reporté : sans bénéfice mesuré à ce jour.
- **Risque de régression** : nul (outillage).

### A8 — `install.rs::steps` : 126 lignes, quatre phases
- **Gravité** : 🟠 · **Priorité** : P2
- **Explication** : `steps` (l. 234-360) enchaîne l'espace libre, l'extraction deflate avec progression, les téléchargements, la mise en place et le registre, puis les raccourcis. Les pourcentages de phase (12, 62, 64, 88, 92) sont écrits en dur dans le code et répétés dans `src/installer/model.ts`.
- **Fichiers** : `src-installer/src/install.rs`, `src/installer/model.ts`
- **Correction** : extraire `check_space`, `extract_payload` et `commit_staging` en fonctions privées, et regrouper les pourcentages dans des constantes nommées. Ni l'ordre ni les messages ne changent.
- **Risque de régression** : faible. Les tests existants pilotent un `Platform` factice et vérifient la séquence.

### A9 — `backend/index.ts` : 670 lignes pour 16 domaines
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** : toutes les commandes `invoke`, événements et types IPC sont dans un seul fichier. Il importe aussi le type `StreamOpts` depuis le store UI (`backend/index.ts:581,591`), ce qui inverse les couches.
- **Fichiers** : `src/backend/index.ts`
- **Correction** : `src/backend/<domaine>.ts`, un fichier par section existante. `index.ts` devient un baril de réexport, si bien qu'aucun import des composants ne change. `StreamOpts` passe dans `backend/stream.ts`.
- **Risque de régression** : faible. `tsc` vérifie chaque import, et les tests mockent `@tauri-apps/api` au même endroit.

### A10 — Inversions de couches dans le frontend
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** :
  - `native.ts:5` importe `recentLangs` depuis `pages/ReplyCard.tsx`. Le pont natif embarque ainsi React et `ReplyBox`.
  - `overlay/OverlayWindow.tsx:6` importe `pages/OverlayPage.tsx`, qui importe `useApp`. Le bundle de l'overlay instancie donc tout le store principal.
- **Fichiers** : `src/native.ts`, `src/pages/ReplyCard.tsx`, `src/overlay/OverlayWindow.tsx`, `src/pages/OverlayPage.tsx`
- **Correction** : `recentLangs` passe dans `src/state/langs.ts`, avec les autres règles de langue. Le découpage de `OverlayPage` est reporté (P3) : il touche le rendu de l'overlay en jeu, à valider visuellement sur Windows.
- **Risque de régression** : faible pour le déplacement de `recentLangs`.

### A11 — Pont « clé de config → configure → setState » copié quatre fois
- **Gravité** : 🟢 · **Priorité** : P2
- **Explication** : `native.ts` répète le même motif (clé JSON, minuterie de 300 ms, appel natif, mise à jour du store) pour l'installation, le chat, la traduction et la parole (l. 120-240). Les minuteries ne sont pas annulées quand le pont est arrêté.
- **Fichiers** : `src/native.ts`
- **Correction** : un utilitaire `debouncedSync(select, apply)` qui renvoie un désabonnement annulant aussi la minuterie. Les quatre ponts s'en servent.
- **Risque de régression** : faible. `native.test.tsx` couvre les quatre ponts.

### A12 — Minuteries non annulées
- **Gravité** : 🟢 · **Priorité** : P2 (les deux premières) / P3 (les autres)
- **Explication** : `native.ts:133,164,196,232` (voir A11) ; `main.tsx:26` jette le désabonnement de `startSettingsSync()`. `store.ts:333,337,344,355,365` concerne le simulateur, `ReplyWindow.tsx:22,48`, `DiagPage.tsx:136` et `Installer.tsx:190,253` concernent des actions ponctuelles. En pratique, ces fenêtres vivent aussi longtemps que l'application.
- **Fichiers** : voir ci-dessus
- **Correction** : A11 pour `native.ts`. Les autres sont documentées et non modifiées, car elles concernent des fenêtres qui ne se démontent jamais.
- **Risque de régression** : —

### A13 — État en double et valeurs de repli dispersées
- **Gravité** : 🟠 · **Priorité** : P3
- **Explication** :
  - Le niveau d'entrée est stocké deux fois (`useAudio.level`, `useApp.level`).
  - La langue cible est copiée de `useApp` vers `useTargetLang` (`store.ts:451`).
  - Le simulateur et l'état réel cohabitent dans `useApp` (`speaking`/`stage`, `seg`/`drop`, `dl`, `test`).
  - Le repli `'fr'` est écrit à 8 endroits.
- **Fichiers** : `src/state/store.ts`, `src/native.ts`, `src/state/translation.ts`, `src/state/setup.ts`
- **Correction** : documentée, non appliquée. Unifier ces états change le moment des rendus, ce qui sort du cadre d'une refactorisation sans changement fonctionnel. Une constante `DEFAULT_TARGET` remplace seulement les littéraux `'fr'` (même valeur).
- **Risque de régression** : élevé si on unifie les états, nul pour la constante.

### A14 — Documentation périmée
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** : `docs/architecture.md` §3 et §7 décrivent `pipeline/` comme le pipeline réel et l'état de session comme une simulation React, alors que parole, traduction, chat, GSI et locuteur sont en Rust. `lib.rs:1-13` liste encore `speech` « (ru, en) » et `pipeline`. Les en-têtes de `docs/testing/{audio-capture,speech-recognition,translation}.md` nomment des branches `feat/*`.
- **Fichiers** : `docs/architecture.md`, `src-tauri/src/lib.rs`
- **Correction** : `docs/architecture/` (vue d'ensemble, modules, dépendances, flux, décisions) remplace `docs/architecture.md`, qui devient un renvoi. Le commentaire de tête de `lib.rs` est mis à jour.
- **Risque de régression** : nul.

### A15 — Pas de contrôle automatique des dépendances
- **Gravité** : 🟠 · **Priorité** : P1
- **Explication** : rien n'empêche un nouveau cycle entre modules Rust ou un import du store depuis la couche IPC. Les cycles mesurés au §1.2 sont apparus sans que personne ne s'en rende compte.
- **Fichiers** : —
- **Correction** : `scripts/check-architecture.mjs` (`npm run check:architecture`), sans dépendance externe :
  - cycles entre modules Rust de premier niveau ;
  - cycles d'import du frontend ;
  - imports autorisés des fichiers partagés avec l'installateur ;
  - liste des commandes Tauri et des événements, comparée à `docs/architecture/ipc-contract.txt`.

  Le script est lancé par `validate.ps1`.
- **Risque de régression** : nul.

### A17 — Chemins de données recalculés à sept endroits
- **Gravité** : 🟢 · **Priorité** : P3
- **Explication** : `%LOCALAPPDATA%` / `%APPDATA%` + `Crosstalk` est recalculé dans `settings.rs:19-27`, `chat/mod.rs:97`, `history.rs:43-51`, `logs.rs:164-172`, `speech/models.rs:86-96`, `speech/gpu_guard.rs:80-85` et `src-installer/src/system.rs:22`. Chaque copie a sa variable de surcharge (`CROSSTALK_CONFIG_DIR`, `CROSSTALK_HISTORY_DIR`…) et son repli (`temp_dir()` ou `.`), qui diffèrent légèrement.
- **Fichiers** : voir ci-dessus
- **Correction** : seul `models_dir` rejoint `paths.rs` (A4). Unifier les replis changerait l'emplacement des fichiers dans des cas limites (variable absente). C'est un changement de comportement, donc documenté et non appliqué. `chat/mod.rs:97` qui ignore `CROSSTALK_CONFIG_DIR` est reporté en F9.
- **Risque de régression** : —

### A18 — Journal : `logs::summarize` relit les statuts des autres modules par noms de champs
- **Gravité** : 🟠 · **Priorité** : P2 (test) / P3 (réécriture)
- **Explication** : `logs.rs:245-448` (204 lignes) analyse le JSON des statuts de capture, de parole, de traduction, du chat et de GSI par leurs noms sérialisés (`"fallbackReason"`, `"vadReady"`…). Si un champ serde est renommé, le journal se dégrade sans erreur de compilation.
- **Fichiers** : `src-tauri/src/logs.rs`
- **Correction** : reportée. Désérialiser vers les types concrets ferait dépendre `logs` de tous les modules, ce qui aggraverait les cycles. Le test existant (`logs.rs` › `summarizes_service_statuses`) part de JSON écrits à la main, donc il ne détecte pas un renommage. Un test de caractérisation est ajouté : il sérialise les **vrais** types de statut et vérifie le résumé, si bien qu'il échoue si un champ est renommé.
- **Risque de régression** : —

### A19 — Logique dupliquée dans le cœur Rust
- **Gravité** : 🟢 · **Priorité** : P2 (dans les fichiers découpés) / P3 (ailleurs)
- **Explication** :
  - Dans `speech/service.rs`, ces blocs sont copiés à l'identique entre `transcribe` et `transcribe_partial` :
    - rejet des segments périmés (903-912 et 982-991) ;
    - préparation de `recognize` (997-1005 et 1096-1104) ;
    - fenêtre de latence (937-940 et 1050-1053).
  - Conversion des annonces de carte en termes de glossaire : `translation/service.rs:292-305`, `587-595` et `reply/mod.rs:204-212`.
  - Libellé de route `"A + B"` : `translation/service.rs:430-435` et `reply/mod.rs:155`.
  - Serveur HTTP local : `gsi` (162-199, 362-397) et `stream` (210-234, 302-327).
  - Deux traits portent le nom `EngineLoader` (`speech/service.rs:74`, `translation/service.rs:147`) ; `reply` utilise une fermeture (`reply/mod.rs:60`).
- **Fichiers** : voir ci-dessus
- **Correction** : factoriser seulement dans les fichiers découpés (A1, A2) et seulement si le bloc est strictement identique. Le serveur HTTP et le cache de moteurs de `reply` sont reportés (P3).
- **Risque de régression** : faible pour des copies strictes.

### A20 — Contrat implicite des identifiants de traduction
- **Gravité** : 🟢 · **Priorité** : P2
- **Explication** : les identifiants du chat commencent à `1 << 40` (`chat/mod.rs:35`) pour ne pas entrer en collision avec ceux des énoncés vocaux dans le service de traduction. Rien ne l'indique côté traduction.
- **Fichiers** : `src-tauri/src/chat/mod.rs`, `src-tauri/src/translation/service.rs`
- **Correction** : documenter le contrat dans `docs/architecture/data-flow.md` et à côté de `submit`. Le code ne change pas.
- **Risque de régression** : nul.

### A21 — Une allocation par bloc audio sur le thread de capture
- **Gravité** : 🟢 · **Priorité** : P3
- **Explication** : `SpeechSink::block` (`speech/service.rs:607-628`) construit un `Vec<f32>` mono pour chaque bloc de capture (environ 20 ms), puis l'envoie par `try_send` au thread VAD. Le silence alloue aussi (`vec![0.0; frames]`). C'est une allocation sur le thread de capture. Elle est modeste (quelques Ko toutes les 20 ms) et n'a causé aucun décrochage mesuré à ce jour.
- **Fichiers** : `src-tauri/src/speech/service.rs`
- **Correction** : aucune dans cette refactorisation. Un réservoir de tampons recyclés changerait le comportement mémoire et la politique d'abandon, ce qui exige une mesure sur Windows. La refactorisation garantit seulement qu'**aucune allocation n'est ajoutée** sur ce chemin.
- **Risque de régression** : —

### A16 — Défauts fonctionnels relevés (hors refactorisation)
Ces points changent le comportement. Conformément à la règle « pas de changement fonctionnel dans une PR de refactorisation », ils ont été **signalés, pas corrigés** dans la refactorisation. Chacun est ensuite corrigé dans sa propre PR `fix/*`, avec un test (colonne « Correction ») :

| # | Constat | Fichier | Correction |
|---|---|---|---|
| F1 | `recentLangs` prend `slice(-n)` sur un fil trié du plus récent au plus ancien (`store.ts:381,386`) : ce sont les **plus anciennes** lignes qui servent à deviner la langue de réponse. | `src/pages/ReplyCard.tsx:9-14` | PR #66 (`fix/*`, en attente de fusion) |
| F2 | `runCaptureTest` arrête la capture à la fin du test, même si une session a démarré entre-temps. | `src/state/audio.ts:150-174` | PR #67 (`fix/*`, en attente de fusion) |
| F3 | `ReplyWindow` déclenche des effets (`dictation.toggle`, `reply.hide`) dans un updater `setState`, qui peut s'exécuter deux fois en mode strict. | `src/reply/ReplyWindow.tsx:25-30` | PR #68 (`fix/*`, en attente de fusion) |
| F4 | `install.rs:307` émet un événement de progression par bloc de 256 Ko, sans limitation de débit. | `src-installer/src/install.rs` | PR #69 (`fix/*`, en attente de fusion) |
| F5 | Un `rename` qui échoue au milieu de la phase 2 laisse un mélange d'anciens et de nouveaux fichiers. | `src-installer/src/install.rs:334` | PR #70 (`fix/*`, en attente de fusion) |
| F6 | `download-models.ps1` ne couvre que ru→fr et en→fr. | `scripts/download-models.ps1` | PR #71 (`fix/*`, en attente de fusion) |
| F7 | `DiagPage` déduit l'état OK/ÉCHEC en comparant des couleurs hexadécimales. | `src/pages/DiagPage.tsx:83-93` | PR #72 (`fix/*`, en attente de fusion) |
| F8 | **À vérifier sur Windows.** Si l'activation de la capture par processus dépasse le délai, l'événement est fermé alors que l'activation asynchrone peut encore se terminer et appeler `SetEvent` sur un handle fermé. Le code de capture n'est pas modifié ici (« aucun changement intrusif de la capture »). | `src-tauri/src/audio/wasapi.rs:340-395` | PR #73 (`fix/*`, en attente de fusion) |
| F9 | `user_rules_file()` ignore `CROSSTALK_CONFIG_DIR`, contrairement à `settings::dir()`. | `src-tauri/src/chat/mod.rs:97` | PR #74 (`fix/*`, en attente de fusion) |
| F10 | Le serveur du mode streaming crée un thread et une file non bornée par client SSE, sans délai d'écriture. Un client bloqué fait grossir sa file. | `src-tauri/src/stream/mod.rs` | PR #75 (`fix/*`, en attente de fusion) |
| F11 | Le serveur GSI accepte les connexions une par une. Un client local lent le bloque. | `src-tauri/src/gsi/mod.rs:362-397` | PR #76 (`fix/*`, en attente de fusion) |
| F12 | Le journal écrit sur disque sous un verrou global, y compris depuis les threads VAD et ASR. | `src-tauri/src/logs.rs:88-100` | PR #77 (`fix/*`, en attente de fusion) |
| F13 | Une panique dans `Downloader::run` laisse `running = true` (aucun garde ne le remet à faux). En release, la panique termine le processus, donc l'effet est limité aux tests. | `src-tauri/src/downloads.rs:248,314` | PR #78 (`fix/*`, en attente de fusion) |
| F14 | Le journal écrit toujours « ru→fr », « en→fr »… quelle que soit la langue de l'utilisateur (`{src}→fr` en dur). Le test de caractérisation `summarizes_the_real_status_types` fige ce comportement en français, sans le corriger. | `src-tauri/src/logs.rs:340` | PR #79 (`fix/*`, en attente de fusion) |

## 3. Sécurité CS2 (inchangée)

L'audit confirme l'absence d'injection, de lecture mémoire de `cs2.exe`, de hook et d'interception réseau. Les sources de données restent `console.log`, la capture WASAPI par processus, Game State Integration et la capture de fenêtre (identification du locuteur, désactivée par défaut). `npm run check:anticheat` est exécuté avant et après la refactorisation. Aucune refactorisation ne touche `audio/wasapi.rs`, `game/` ni `speaker/screen.rs`.
