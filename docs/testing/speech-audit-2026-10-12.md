# Audit et validation vocale du 12 octobre 2026

## Périmètre et provenance

Après clarification du mainteneur, cette livraison concerne uniquement la reconnaissance vocale et ses diagnostics. Aucune modification de workflow, de compilateur officiel, de traduction ou d'identification des joueurs. La PR #23 a été fusionnée le 12 octobre à 09:24:58 UTC ; le document de reprise qui la présentait comme ouverte était dépassé. Base : `develop f15c513c02c49b27575aec8888c1d948fadc4139`. Code corrigé compilé : `9de8bf504f5c386863a1f128797ae7bd54c4ebe0` (les commits ultérieurs de cette livraison documentent les résultats uniquement).

Machine : Windows 11 Famille build 26200, Intel Core i7-1165G7 (4 cœurs / 8 threads), RAM 16 965 230 592 octets, Intel Iris Xe (non utilisé pour Whisper). Rust 1.99.0 `b940084d7`, Node 24.19.0. Le Node local diffère du Node 22 de CI ; tests et build sont exécutés, sans changement de dépendances. MSVC 19.29.30157 / Visual Studio 2019 Build Tools présents sur ce PC, et non le VS 2022 suggéré dans le document. Bindgen utilise libclang 18.1.1 local. CMake compile Whisper en `Release`, AVX2/FMA/F16C/BMI2, `GGML_NATIVE=OFF`. Aucun changement d'optimisation.

## Erreurs confirmées et corrections

| Gravité | Constat | Correction / preuve |
| --- | --- | --- |
| P1 | L'interface démarrait sur `small`, le moteur et l'installation par défaut sur `base`. Un démarrage pouvait sélectionner un modèle absent. | `c6c8c3f` reprend le correctif ciblé de Claude `3ababe5` non intégré à develop. Le nouveau test UI échoue avant le changement (`small` au lieu de `base`), puis réussit. |
| P1 | `--threads` affiché mais ignoré en mode live. | `f0924a9`, `f8d6286` : transmission explicite au service ; test du nombre réellement reçu par Recognizer. Paramètres invalides refusés. |
| P1 | Les blocs audio déjà dans le canal pouvaient être traités sous la nouvelle génération après arrêt. Activité VAD et publication pouvaient également courir avec purge. | `f0924a9` : génération capturée avant downmix, contrôle de génération, publication sérialisée avec purge. Tests VAD bloqué et backlog complet antérieur à purge. |
| P1 | Changement de modèle pendant chargement : requête ignorée, ancien modèle susceptible d'être utilisé/étiqueté comme le nouveau. | `f0924a9` : chargement jusqu'au modèle courant, vérification du modèle effectif, purge lors du changement. Test avec loader bloqué. |
| P1 | Fraîcheur vérifiée uniquement avant Whisper ; un résultat pouvait expirer pendant le calcul. | `f0924a9` : contrôle après attente et calcul. Limite de six secondes inchangée. Cela supprime les résultats tardifs, sans accélérer leur calcul. |
| P2 | Le message de seuil VAD pouvait être perdu lorsque le canal audio était plein. | `f0924a9` : configuration relue directement, suppression du message de contrôle fragile. |
| P2 | Benchmark pouvait ignorer un modèle absent et terminer avec succès ; live autorisait le fallback global. | `f8d6286` : code d'erreur et capture process CS2 stricte. |
| P2 | Durées de détection de langue, inférence et texte brut rejeté non observables. | `afd7ec9`, `f8d6286` : timings et `--verbose` hors ligne ; service opt-in `CROSSTALK_SPEECH_DIAGNOSTICS=1`, stderr, aucune persistance ajoutée. |

Revue indépendante en lecture seule : aucune régression bloquante relevée après correction de l'ordre des verrous. L'inférence reste hors du verrou de publication ; ordre publication puis statistiques uniforme. Les vrais callbacks WASAPI/CS2 restent à valider en partie.

## Tests réellement exécutés

```powershell
npm ci
npm run typecheck
npm test
npm run build
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked --example asr_bench --example audio_probe
```

Résultats sur le code corrigé : 69 tests React (7 fichiers), 104 tests Rust, typecheck/build/fmt/clippy réussis. Les tests jsdom produisent des messages `HTMLCanvasElement.getContext` non implémenté ; ils ne valident pas le rendu 3D, qui nécessite l'application native. La base Rust non corrigée passait 98 tests sur ce PC.

Les commandes npm sont exécutées à la racine du worktree corrigé, les commandes cargo dans `src-tauri`. Pour cette machine, `LIBCLANG_PATH=C:/workspace/Crosstalk-codex-validation/tooling/clang/native` et le CMake des VS 2019 Build Tools sont ajoutés au PATH du processus. Le cache isolé utilisé est `CARGO_TARGET_DIR=C:/workspace/Crosstalk-codex-audit/src-tauri/target` ; le dépôt initial de Claude n'est pas modifié.

Packaging réellement exécuté : `npx tauri build --config C:/workspace/Crosstalk-codex-validation/package-config.json`, avec uniquement `{"build":{"beforeBuildCommand":""}}` pour réutiliser le frontend déjà construit dans ce worktree. La cible hôte est Windows MSVC x64. Exécutable et installateurs NSIS + MSI fr-FR/en-US générés ; installation NSIS silencieuse réussie (code 0). Installation MSI non exécutée.

Test natif du binaire corrigé : écran Audio & IA, `Whisper base` prêt, 4 threads ; modèle `small` effectivement chargé en 2,1 s puis retour à `base` en 633 ms. Le démarrage sans CS2 affiche l'erreur attendue de capture, aucun flux système global ; Ctrl+Alt+F10 ramène la session à `ARRÊTÉ`. Test effectué hors partie : le raccourci global est validé, l'absence de résultats tardifs d'une vraie capture reste à tester.

## Comparaison GCC / MSVC sur la même machine

Source des deux exécutables de référence : archive du même commit `f15c513`. Rust 1.99.0 dans les deux cas, modèles officiels identiques, 2 threads, encodeur réduit, glossaire et VAD identiques. GCC 13.2 MinGW POSIX est compilé sous Ubuntu/WSL puis l'exécutable Windows est lancé **nativement sur ce PC**, sans Wine. Le header de compatibilité des déclarations EcoQoS de la branche Claude est utilisé pour MinGW ; la génération bindgen cible les headers Windows, aucune structure ABI n'est forcée ni vérification désactivée.

Corpus : 4 extraits FLEURS dev russes et 4 anglais, 91 secondes au total, téléchargés avec `fetch-asr-corpus.py --per-lang 4`. Le scénario combat ajoute des tirs/explosions **synthétiques**. Ces phrases lues ne sont pas des annonces CS2. Les exécutions sont séquentielles, sans compilation concurrente. La fenêtre de test était encore ouverte : une première commande de fermeture avait comparé des chemins utilisant des séparateurs différents et n'avait pas trouvé le processus. Cette fenêtre a ensuite été effectivement fermée.

**Comparaison non concluante** : CS2 a été lancé à 12:54:25 heure locale, pendant le benchmark GCC (12:54:12–12:55:11). La passe MSVC s'était terminée à 12:54:12. Les charges système diffèrent donc malgré le même PC et le même code. Les nombres ci-dessous sont conservés pour traçabilité, pas pour classer les compilateurs. Pas de répétition statistique ; charge résiduelle et chauffe du portable ne sont pas exclues.

Commande pour chaque exécutable :

```powershell
& $bench --models-dir C:/workspace/Crosstalk-codex-validation/models `
  --manifest C:/workspace/Crosstalk-codex-validation/corpus/manifest.tsv `
  --scenario clean,combat --model base --threads 2 --markdown rapport.md --verbose
```

| Compilateur (source f15c513) | Scénario | Whisper p50 / p95 mesuré | RTF moyen | CPU / durée audio | WER ru / CER ru | WER en / CER en | Rejets |
| --- | --- | --- | --- | --- | --- | --- | --- |
| MSVC 19.29 | propre | 930 / 3612 ms | 0,39 | 44 % d'un cœur | 40,2 / 20,3 % | 34,2 / 25,0 % | LowConfidence 1, Repetition 2, WrongScript 1 |
| GCC 13.2 | propre | 1107 / 4124 ms | 0,49 | 53 % d'un cœur | 40,2 / 20,3 % | 34,2 / 25,0 % | identiques |
| MSVC 19.29 | combat synthétique | 1175 / 2194 ms | 0,36 | 48 % d'un cœur | 63,4 / 30,6 % | 15,8 / 6,9 % | LowConfidence 3, Repetition 1 |
| GCC 13.2 | combat synthétique | 1415 / 2712 ms | 0,46 | 54 % d'un cœur | 63,4 / 30,6 % | 15,8 / 6,9 % | identiques |

Ces mesures ne permettent ni de confirmer ni d'infirmer l'hypothèse d'un MSVC quatre fois plus lent, à cause du lancement du jeu pendant la seconde passe. Recommandation : conserver MSVC jusqu'à une comparaison répétée avec CS2 fermé et charge contrôlée. Aucun essai clang-cl ni changement de priorité, modèle par défaut du moteur, quantification ou kernels n'est appliqué. La différence avec les anciens chiffres CI (p50 8,3 s) ne doit pas être appelée gain du correctif : CPU, versions d'outillage et conditions différents.

Le CPU rapporté ici est le temps CPU du processus divisé par la **durée de l'audio**, 100 % correspondant à un cœur occupé pendant toute cette durée. Ce n'est pas le CPU moyen pendant une partie, ni le coût de rendu WebView2. La latence estimée du banc ajoute 608 ms de post-roll/pas VAD à l'inférence et exclut la file d'attente ; elle n'est pas une latence live mesurée.

## Code corrigé : corpus sous charge et capture réelle

Exécutable MSVC `9de8bf5`, mêmes huit WAV et modèles officiels, `--model tiny,base --threads 4 --scenario clean,combat --verbose`. CS2 était alors actif et fortement consommateur de CPU ; la fenêtre de test était encore ouverte pendant l'essentiel de ce passage. Ces données illustrent la sensibilité à la concurrence, et ne sont pas un avant/après du correctif.

| Modèle | Scénario FLEURS | Whisper p50 / p95 mesuré | Latence **estimée**, hors file | RTF moyen | CPU / durée audio | WER ru / CER ru | WER en / CER en |
| --- | --- | --- | --- | --- | --- | --- | --- |
| tiny | propre | 1562 / 18907 ms | 2170 / 19515 ms | 1,19 | 130 % d'un cœur | 58,5 / 28,7 % | 51,3 / 39,2 % |
| tiny | combat synthétique | 2335 / 4674 ms | 2943 / 5282 ms | 0,77 | 110 % d'un cœur | 84,1 / 66,7 % | 76,3 / 65,7 % |
| base | propre | 2485 / 11018 ms | 3093 / 11626 ms | 1,16 | 162 % d'un cœur | 40,2 / 20,3 % | 34,2 / 25,0 % |
| base | combat synthétique | 2858 / 5888 ms | 3466 / 6496 ms | 0,92 | 147 % d'un cœur | 63,4 / 30,6 % | 15,8 / 6,9 % |

La précision et les rejets sont identiques aux passes de référence pour chaque modèle/scénario : aucun filtre n'est desserré. Pour base : propre `LowConfidence 1, Repetition 2, WrongScript 1`, combat `LowConfidence 3, Repetition 1`. Pour tiny : propre `LowConfidence 1, Repetition 5`, combat `Empty 1, LowConfidence 2, Repetition 9`. Ce corpus ne justifie pas de remplacer base par tiny pour le russe. Les durées montrent qu'une garantie de latence en jeu reste à établir.

Test hors ligne de 120 secondes de bruit de combat **synthétique**, modèle base, 4 threads, seuil 0,55 : **0 segment, 0 inférence, 0 fausse transcription**.

Test live de 300 secondes (13:01:19–13:06:19 heure locale), application de test fermée, modèle base, 2 threads, capture WASAPI process de `cs2.exe`, arrêt normal avec purge :

```powershell
.\crosstalk-asr-bench.exe --models-dir C:/workspace/Crosstalk-codex-validation/models --live 300 --model base --threads 2
```

Le mainteneur a confirmé **aucun chat vocal actuellement**. Résultat réel : **5 segments, 0 transcription affichée**, rejets **Short 4, Stale 1**, **0 bloc perdu**, CPU moyen **10 % d'un cœur / 1,2 % de la machine**. p50/p95 non disponibles, car aucune transcription acceptée. Aucune mesure de précision vocale ou de FPS n'en découle. Le log local est conservé hors dépôt ; aucun enregistrement audio ou texte de partie n'est committé. La présence de combats continus n'a pas été attestée, il s'agit donc d'une session CS2 sans chat vocal déclaré, pas d'un corpus contrôlé de combats.

Vérifications CLI sur l'exécutable MSVC corrigé : `--threads 0`, `--threads abc`, modèle absent et `--live 1` avec CS2 fermé terminent chacun par code 2. Le service live affiche `Loading`, `Ready`, puis l'erreur CS2 absent. Aucune vraie capture vocale n'est effectuée dans ce test négatif.

## Interprétation des rejets et limites

Les 17 Overflow et 8 Stale du test du mainteneur prouvent une charge dépassant le consommateur, sans identifier seuls la cause de ce coût. Short signifie moins de 256 ms classées vocales ; LowConfidence signifie log-probabilité moyenne inférieure à -1.0. Sans l'audio et le texte brut concernés, impossible de conclure que ces filtres rejettent une véritable annonce. Aucun seuil, délai, capacité (3 segments) ou encodeur n'a été modifié.

La source exacte de whisper.cpp montre deux encodages en mode bilingue : détection de langue puis `full`. La première détection utilise le contexte initial complet de 30 s ; les suivantes héritent du contexte précédent. Une langue unique explicite évite cette détection, mais ne convient pas automatiquement à une équipe bilingue. Les diagnostics permettent d'en mesurer le coût ; aucune réécriture du décodeur ou cache de langue n'est introduit.

CS2 n'était pas lancé pendant le test négatif initial du CLI et du raccourci UI ; il a ensuite été lancé par le mainteneur. Celui-ci a confirmé l'absence de chat vocal pendant la capture live suivante. Latence de vrais callouts, transcriptions correctes/manquées et FPS avec/sans Crosstalk restent donc non mesurés. Le protocole reproductible est dans [speech-validation.md](../speech-validation.md) et [speech-recognition.md](speech-recognition.md). L'issue #9 reste en revue.

La CI GitHub actuelle reste bloquée avant démarrage par « recent account payments have failed or your spending limit needs to be increased », confirmé sur le run `38229742237`. Le mainteneur doit consulter les paramètres du compte GitHub → Billing & plans, vérifier le moyen de paiement et le budget Actions. Aucun paiement ni réglage du compte effectué.
