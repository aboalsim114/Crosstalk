# Registre des bugs — mission de stabilisation

Branche `fix/rea-full-stabilization` (depuis `develop` + `main` 97e6682).
Environnement : Windows 11 Famille 26200, Intel Core i7-1165G7 (4 c / 8 t), 15,8 Go, rustc 1.99.0 MSVC, Node 24.19.0.
Sévérités : P0 inutilisable · P1 fonction principale défaillante · P2 précision, latence ou comportement · P3 mineur.

| ID | Sév. | Résumé | Statut |
|---|---|---|---|
| CT-001 | P2 | Une boucle de Whisper sur un segment court fait rejeter toute l'annonce | Corrigé, vérifié au banc (f201140) |
| CT-002 | P2 | La version officielle MSVC (`validate.ps1`) est compilée sans la fonctionnalité `gpu` : pas de Whisper sur carte graphique, contrairement au README | Ouvert, rendu visible dans le rapport |
| CT-003 | P3 | L'intro affiche « V0.2 » en dur dans la version 0.3.0 | Corrigé, vérifié par test |
| CT-004 | P3 | Le coin d'overlay enregistré est ignoré au démarrage (« overlay window missing ») | Corrigé, vérifié dans l'application |
| CT-005 | P3 | « modèles absents » et un WARN au journal à chaque démarrage, alors que les modèles sont installés | Corrigé, vérifié dans l'application |
| CT-006 | P3 | Licences tierces inexactes dans À propos (rubato cité ; candle, modèles et CC-BY absents) | Corrigé, vérifié par test |
| CT-007 | P1 | `crosstalk.exe` (MSVC) dépend de `MSVCP140.dll`, que ni Windows ni l'installateur ne fournissent | Corrigé (74ace45) ; dépendance supprimée et vérifiée, non essayé sur un PC vierge |
| CT-008 | P2 | Dans l'application, Whisper répète une phrase entière (« Save your weapons this round. » ×4) : l'annonce est rejetée | Corrigé (7f1ddbb), vérifié dans l'application |
| CT-009 | P3 | Mode streamer : désactivé, le serveur local reste à l'écoute sur 127.0.0.1:31983 | Ouvert |

## CT-001 — Une boucle de Whisper fait perdre l'annonce

- **Environnement** : `crosstalk-asr-bench.exe` du build de référence 97e6682 (MSVC, release), whisper base, CPU AVX2, 4 threads.
- **Reproduction** : corpus synthétique (voix SAPI Windows, phrases inventées), `--manifest manifest.tsv --model base --scenario clean --lang en,fr --verbose`.
- **Attendu** : « They are rotating mid, three of them. » transcrit.
- **Constaté** : le segmenteur coupe à la virgule. Le premier segment (1,66 s) est décodé en boucle, « they are rotating mid, » ×20, en 1 669 ms au lieu de ~700 ms. Le filtre le rejette (`Repetition`) : seule la moitié « 3 of them » s'affiche. Même chose en combat sur `fr8` (« je retire » ×30).
- **Cause** : décodage glouton sans repli de température (`temperature_inc = 0`, choisi pour la latence) et plafond fixe de 96 jetons, quelle que soit la durée du segment.
- **Correction** : `speech/engine.rs`. Une seconde passe à température 0,2, uniquement quand la première boucle (`filter::is_looping`) ; elle n'est gardée que si elle ne boucle plus. Le plafond de jetons suit la durée du segment (16 + 12/s, entre 24 et 96).
- **Tests** : `engine::tests::the_token_budget_follows_the_segment_length`, et `filter::tests::rejects_loops` (cas `is_looping`).
- **Validation** (même banc, même corpus, même machine) :

  | Mesure | Avant | Après |
  |---|---|---|
  | base · propre : WER en | 25,7 % | 14,3 % |
  | base · propre : rejets | Repetition 1 | aucun |
  | base · combat : rejets | Repetition 1 | aucun |
  | bruit seul 120 s : fausses transcriptions | 0 | 0 |

  Pour `fr8` en combat, la seconde passe rend « je tu presque mort, je retire, je retire » : un texte dégradé, mais affiché, là où rien ne l'était.
- **Statut** : corrigé, vérifié au banc. Non vérifié en partie réelle.

## CT-002 — Pas de Whisper sur carte graphique dans la version MSVC

- **Constaté** : `scripts/validate.ps1` construit l'application avec `npx tauri build --target x86_64-pc-windows-msvc`, sans `--features gpu`. Seule la version de test MinGW (`scripts/cross-build-windows-gnu.sh`) active `gpu`. Le README annonce pourtant « Whisper tourne sur la carte graphique (Vulkan) ». L'installateur officiel MSVC fait donc tout tourner sur le processeur.
- **Cause** : la version MinGW charge `vulkan-1.dll` à la demande (`scripts/cross/vulkan_shim.c`), ce qui lui permet de démarrer sur un PC sans pilote Vulkan. Ce mécanisme n'existe pas en MSVC. Activer `gpu` tel quel lierait `vulkan-1.dll` statiquement : l'application ne démarrerait plus sur un PC sans Vulkan.
- **Ce qui est fait** : le rapport `validation.md` indique désormais « Whisper GPU : absent de ce build MSVC », pour que personne ne prenne ce paquet pour une version avec GPU.
- **Ce qui reste à faire** : un build MSVC avec `gpu` et `/DELAYLOAD:vulkan-1.dll` (plus `delayimp.lib`), puis un test sur un PC avec et sans pilote Vulkan. Impossible ici : le SDK Vulkan (`glslc`) n'est pas installé sur cette machine.
- **Statut** : ouvert, documenté.

## CT-003 — Version « V0.2 » en dur sur l'intro

- **Constaté** : capture CDP de l'application 0.3.0 lancée : « V0.2 — MMXXVI ».
- **Cause** : texte écrit en dur dans `src/app/shell/Intro.tsx`.
- **Correction** : la version vient de `package.json`, injectée par Vite (`__APP_VERSION__`), de même que la valeur de repli de `getAppInfo` hors Tauri.
- **Test** : `App.test.tsx` vérifie « V0.3 — MMXXVI » et `introVersion('0.3.0')`.
- **Statut** : corrigé (60991ef).

## CT-004 — Coin d'overlay enregistré ignoré au démarrage

- **Reproduction** : copie de test, Overlay › « Bas gauche » (fenêtre placée en 24,448, réglage enregistré `"corner":"bl"`), puis redémarrage.
- **Constaté** : l'overlay revient en 1456,24 (haut droite). La console de la fenêtre principale affiche `overlay_configure failed · overlay window missing` (REA `inspect-web-page`, ev_a24fee67…).
- **Cause** : la fenêtre principale envoie les réglages d'overlay avant que le cœur ait créé la fenêtre overlay (dans `setup`, après le démarrage des services). L'échec n'était jamais renvoyé, et le coin n'est renvoyé que s'il change.
- **Correction** : `overlayFlagsSync` (`src/app/native.ts`). Un envoi en échec est retenté au changement suivant, et le signal « prête » de l'overlay renvoie tous les réglages, coin compris.
- **Validation** : même scénario après correction, overlay en 24,448 au redémarrage. Test : `native.test.tsx`.
- **Statut** : corrigé (e14f59d), vérifié dans l'application.

## CT-005 — « modèles absents » au démarrage

- **Constaté** : le journal de l'application lancée affiche à chaque démarrage `translation: en→fr : missing` (WARN), puis `loading`, puis `ready` ; l'interface affiche « modèles absents » pendant ce temps.
- **Cause** : une paire pas encore examinée par le thread de traduction démarrait à l'état `Missing`.
- **Correction** : état initial `Loading` (`translation/service/mod.rs`) ; le thread décide ensuite `Missing` ou `Ready`.
- **Validation** : journal du démarrage suivant, `loading` puis `ready` et aucun WARN. Test : `models_not_yet_examined_are_loading_not_missing`.
- **Statut** : corrigé (c02e61b).

## CT-006 — Licences tierces inexactes

- **Constaté** : À propos cite rubato (absent de `Cargo.lock`) et omet candle, ureq, React, les modèles Whisper, Silero VAD et Opus-MT.
- **Vérification** : licences des 37 dépôts Opus-MT du catalogue lues sur l'API Hugging Face : 35 en Apache-2.0, `opus-mt-ru-en` et `opus-mt-en-de` en CC-BY-4.0 (attribution obligatoire). whisper.cpp et whisper-vad sont en MIT.
- **Correction** : `src/app/state/data.ts`. Test : `licenses.test.ts`, qui confronte la liste à `src-tauri/Cargo.toml`.
- **Statut** : corrigé (d4ec04a). La revue juridique complète (texte des licences livré avec l'application) reste à faire.

## CT-007 — Dépendance à MSVCP140.dll (redistribuable Visual C++)

- **Constaté** : `dumpbin /dependents` sur le `crosstalk.exe` officiel (MSVC, 97e6682) liste `MSVCP140.dll`. Les imports sont des fonctions de la bibliothèque standard C++ (flux, `std::regex`, `_Thrd_join`) utilisées par whisper.cpp.
- **Risque** : sur un Windows sans le redistribuable Visual C++ 2015-2022, l'application ne démarre pas (« MSVCP140.dll introuvable »). L'installateur NSIS/MSI de Tauri ne l'installe pas. Cette machine l'a (14.51.36247), donc le démarrage y fonctionne : le défaut n'a **pas** été reproduit sur un PC vierge, faute de machine ou de VM disponible (Windows Famille, pas de Windows Sandbox).
- **Correction** : CRT statique pour la cible MSVC (`src-tauri/.cargo/config.toml`, `+crt-static`) ; CMake compile alors whisper.cpp en `/MT`.
- **Validation** : `dumpbin /dependents` sur le nouveau `crosstalk.exe` ne liste plus `MSVCP140.dll` ni `VCRUNTIME140.dll` (seulement l'UCRT `api-ms-win-crt-*`, fournie par Windows 10/11). Ce build a été lancé et a fait une session complète (capture, Whisper, traduction), et les 254 tests Rust passent.
- **Non vérifié** : un démarrage sur un Windows vierge sans redistribuable.
- **Statut** : corrigé (74ace45).

## CT-008 — Whisper répète une phrase entière : l'annonce est rejetée

- **Reproduction** : copie de test, source « Périphérique de sortie », session démarrée, fichiers du corpus synthétique joués sur la sortie audio. `CROSSTALK_SPEECH_DIAGNOSTICS=1`, stderr redirigé.
- **Constaté** : sur 10 annonces jouées, 2 rejetées `Repetition`. Le texte brut était `Save your weapons this round. Save your weapons this round. Save your…` et `Je suis presque mort, je recule.` ×4. Whisper avait bien entendu la phrase, puis l'avait répétée jusqu'au plafond de jetons. La seconde passe de CT-001 bouclait elle aussi.
- **Cause** : boucle du décodage glouton après la fin réelle de la phrase, avec la fenêtre d'encodeur réduite.
- **Correction** :
  1. `filter::collapse_repeats` : une phrase d'au moins 3 mots différents, répétée bout à bout (dernière copie éventuellement coupée), est ramenée à une seule occurrence avant les autres contrôles. Un « go go go go » reste une boucle rejetée, et une hallucination répétée (« Thanks for watching! » ×3) reste rejetée une fois ramenée à un exemplaire.
  2. La seconde passe (CT-001) n'a lieu que si cette réduction ne suffit pas.
- **Essai écarté (mesuré)** : une seconde passe sur la fenêtre complète de 30 s récupérait « Je suis presque mort, je recule. », mais coûtait +4,3 s sur le processeur (full_ms 5 162). L'annonce suivante devenait alors périmée (Stale).
- **Tests** : `keeps_a_sentence_whisper_repeated_after_hearing_it_once`, `rejects_loops`.

- **Validation dans l'application** (même scénario de 10 annonces, build 74ace45) : 12 transcrits, 0 rejeté ; latence p50 1,5 s, p95 2,3 s. Avant : 2 rejets « Repetition », p50 2,4 s, p95 4,0 s.
- **Statut** : corrigé (7f1ddbb).

## CT-009 — Le serveur du mode streamer reste ouvert après désactivation

- **Reproduction** : `stream_configure` avec `enabled: true` (page 200, SSE OK), puis `enabled: false`.
- **Constaté** : `listening` reste `true`. Le port 127.0.0.1:31983 continue d'accepter les connexions. Aucune ligne n'est plus envoyée.
- **Cause** : `StreamService::listen` lance une boucle `accept` bloquante, sans aucun moyen de l'arrêter.
- **Impact** : faible. Le serveur est local (127.0.0.1), vérifie l'en-tête Host et ne sert plus aucun sous-titre une fois désactivé. Le port reste occupé jusqu'à la fermeture de l'application.
- **Statut** : ouvert. Corriger demande une boucle d'acceptation qu'on peut interrompre (écoute non bloquante, ou fermeture du socket), à tester avec OBS.