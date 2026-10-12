# Validation du moteur vocal (Windows)

## Corrections et tests associés

| Correction | Régression vérifiée |
| --- | --- |
| Chaque bloc porte sa génération au début du callback de capture. Les blocs déjà dans le canal sont ignorés après purge. | `purged_capture_blocks_cannot_be_relabelled_as_new_audio` consomme une phrase complète antérieure à la purge : aucun segment, aucune inférence. |
| Purge et publication VAD/ASR sont sérialisées, avec ordre des verrous publication puis statistiques. Le calcul reste hors verrou publication. | `purge_during_vad_cannot_restore_old_speaking_activity` bloque le VAD pendant purge ; `purge_discards_the_result_being_computed` couvre le résultat ASR en vol. |
| L'âge est vérifié après attente du recognizer et après calcul, avant publication. La limite reste six secondes. | `result_expiring_during_inference_is_rejected` vérifie précisément la limite et la nanoseconde suivante. |
| Le seuil VAD est lu depuis la configuration, sans message de contrôle susceptible d'être perdu dans le canal audio plein. | `configure_keeps_the_model_when_unchanged`, `threshold_changes_apply_live` ; la configuration ne recharge pas le modèle. |
| Un changement de modèle purge les anciens segments ; un chargement dépassé recommence sur le modèle actuellement demandé. Le loader attend le recognizer sans tenir le verrou de configuration. | `reload_follows_configuration_changed_during_load` bloque le premier chargement, change le modèle puis vérifie le modèle installé et la suite des chargements. |
| Le service accepte un nombre explicite de threads. L'application conserve son défaut. Le CLI refuse zéro et les valeurs dépassant les CPU disponibles. | `explicit_live_threads_are_preserved` vérifie le nombre reçu dans `RecognizeOptions`, `explicit_threads_never_oversubscribe` vérifie les bornes. |
| `--live` exige un moteur prêt et la capture du processus CS2. Un modèle absent termine le benchmark avec un code non nul. | Vérifier les commandes négatives ci-dessous ; aucun fallback vers la sortie système. |
| Diagnostics opt-in : texte brut, décision du filtre, durée audio, attente, inférence, langue et timings langue/full. | `--verbose` sur corpus pour tous les segments ; variable d'environnement uniquement pour le service live. |

## Vérifications de développement

Utiliser le compilateur MSVC officiel du dépôt et ses paramètres existants. Ne pas changer `GGML_NATIVE=OFF`, l'encodeur ou les optimisations pour comparer deux exécutions.

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib speech:: -- --test-threads=1
cargo check --manifest-path src-tauri/Cargo.toml --example asr_bench
cargo build --release --manifest-path src-tauri/Cargo.toml --example asr_bench
```

Le build Whisper exige CMake, les outils C++ MSVC et libclang. `LIBCLANG_PATH` peut pointer vers le dossier contenant `libclang.dll` sans modifier les fichiers du dépôt.

## Corpus hors ligne

Un manifeste TSV contient `chemin WAV`, `ru` ou `en`, puis la transcription de référence. Les chemins sont relatifs au manifeste. Utiliser exactement le même corpus et la même machine avant/après. Inclure voix propres, phrases courtes CS2, bruit réel de combat, tirs seuls et silences. Un petit corpus général tel que FLEURS permet une vérification initiale, pas une validation des callouts en jeu.

```powershell
$bench = '.\src-tauri\target\release\examples\asr_bench.exe'
& $bench --manifest corpus/manifest.tsv --model tiny,base --scenario clean,combat --threads 2 --verbose
& $bench --manifest corpus/manifest.tsv --model tiny,base --scenario clean,combat --threads 4 --verbose
& $bench --manifest corpus/manifest.tsv --model tiny,base --scenario clean,combat --threads 2 --full-encoder --verbose
& $bench --noise-only 120 --model base --threads 2 --verbose
```

Le mode `combat` ajoute du bruit **synthétique** ; il ne simule pas la séparation de la voix du mix CS2. Pour le mix réel, fournir des WAV déjà capturés et choisir `--scenario clean`.

`--verbose` imprime les résultats bruts, y compris rejetés, et les durées de détection de langue et de `full`. La latence hors ligne est une estimation (post-roll + hop VAD + calcul), sans file temps réel. `queue_ms` live mesure attente du job depuis son insertion, y compris l'attente du verrou recognizer ; `inference_ms` comprend langue et full. Les timings sont absents pour les backends de test et les segments non envoyés au recognizer.

## Vérifications négatives et live

```powershell
& $bench --noise-only 1 --models-dir dossier-inexistant --model base
# Attendu : code 2, erreur de modèle/VAD, jamais un rapport de réussite vide.
& $bench --live 1 --model base --threads 0
# Attendu : code 2, erreur --threads.
& $bench --live 1 --model base --threads abc
# Attendu : code 2, erreur --threads.
& $bench --live 1 --model base
# Si CS2 est fermé et les modèles installés : code 2, CS2 absent.
```

Lancer CS2, puis une session de 300 secondes sur chacune des configurations comparées. Garder sortie audio, langue, modèle et scène comparables. Dans une session séparée avec l'application installée, utiliser Ctrl+Alt+F10 puis reprendre, changer de langue et de modèle, fermer CS2 ; vérifier qu'aucun ancien sous-titre réapparaît, que PAROLE revient à l'état inactif et que les réglages restent réactifs. Le CLI standalone n'enregistre pas ce raccourci global : son arrêt est contrôlé par la durée de --live.

```powershell
$env:CROSSTALK_SPEECH_DIAGNOSTICS = '1'
& $bench --live 300 --model base --threads 2
Remove-Item Env:CROSSTALK_SPEECH_DIAGNOSTICS
```

Les diagnostics du service sont désactivés par défaut et vont uniquement sur stderr. Ils contiennent du texte brut privé. Aucune persistance n'est ajoutée ; une redirection vers un fichier ou `--markdown` constitue un choix explicite du testeur. Aucun audio n'est écrit sauf `--export-mix`.

À l'arrêt, capture puis pipeline sont purgés. Le bilan porte sur les résultats terminés avant arrêt ; les derniers segments en attente ou encore en calcul sont abandonnés. Il n'y a pas d'attente prétendant vider la file.

## Limites

Ces corrections préviennent des races et rendent les mesures reproductibles ; elles ne prouvent aucun gain de latence ou de précision. Les seuils de filtre, la capacité de file (trois segments), le timeout de six secondes et le calcul de contexte encodeur restent identiques. Les résultats tardifs sont supprimés, pas accélérés. La capture live reste le mix audio du processus, sans séparation des tirs et des voix. La détection de langue est restreinte à russe/anglais ; sa probabilité est relative aux langues autorisées. Une vraie session CS2 et un corpus représentatif sont nécessaires avant de desserrer un filtre ou de modifier l'encodeur.
