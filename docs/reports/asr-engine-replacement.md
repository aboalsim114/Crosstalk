# Remplacement du moteur de reconnaissance vocale

Branche `feat/asr-engine-replacement`, depuis `develop`. Décision :
[D-14](../architecture/decisions.md). Whisper est retiré entièrement, sans
moteur de secours.

**À lire d'abord : ce qui n'a pas été vérifié.**
- Aucun test avec CS2 lancé. La reconnaissance en partie, la latence réelle,
  l'impact sur les FPS et les longues sessions restent à mesurer
  (`docs/testing/speech-recognition.md`).
- Les mesures viennent d'un processeur de conteneur (Xeon 2,8 GHz, 4 cœurs),
  pas du PC de référence (i7-1165G7, 16 Go).
- Le corpus est synthétique : voix Piper, bruit de combat généré, et deux
  vrais enregistrements de démonstration.
- Le paquet Windows est compilé, mais `crosstalk.exe` n'a pas été lancé sur
  Windows (voir G).

## A. Audit

### État avant le changement

- **Moteur** : whisper.cpp via whisper-rs.
- **Modèles** : `ggml-tiny`, `base`, `small`, `medium`, `large-v3-turbo`.
  `base` était le modèle par défaut, l'installateur posait `base` et `small`.
- **Détection de voix** : Silero, version ggml.
- **Carte graphique** : Vulkan en option (build MinGW seulement).
- **Secours** : bascule automatique vers `base` quand le modèle choisi était
  trop lent.
- **Indications au modèle** : prompt initial (glossaire, lieux de la carte
  via GSI).
- **Filtres** : log-probabilité et « no speech ».

### Régression #103 : diagnostic en 6 étapes

Sources : journaux du 15 et du 16 octobre envoyés par l'utilisateur. Seuls
les compteurs et les messages ont été lus ; aucun contenu n'est repris ici.

| Étape | Constat | Verdict |
|---|---|---|
| 1. Audio capturé ? | Niveau autour de −35 dBFS pendant les parties | OK |
| 2. VAD ? | Des segments sont détectés et transmis | OK |
| 3. File d'attente ? | Beaucoup de segments écartés « trop tard » ou « file pleine » | **Perte** |
| 4. Reconnaissance ? | Whisper tournait entre 1,2 et 29 fois le temps réel, en priorité basse ; les segments restants étaient souvent rejetés (confiance faible, boucles) | **Cause principale** |
| 5. Transmission au LLM ? | « traduction N ok » dans les journaux | OK |
| 6. Affichage ? | Non vérifiable depuis les journaux | Non vérifié |

**Conclusion** : la parole arrivait jusqu'au moteur. Whisper, trop lent
pendant une partie, saturait la file d'attente, et ses filtres rejetaient le
reste. La priorité normale (D-13, build 281) a réduit le problème sans le
régler, d'où le changement de moteur.

## B. Alternatives étudiées

Critères : gratuit, local, hors ligne, sans clé, ru + en + fr, licence du
code d'exécution **et** des poids compatible avec la redistribution.

| Candidat | Runtime | Licence du runtime | Licence des poids | ru / en / fr | Résultat |
|---|---|---|---|---|---|
| **Parakeet TDT 0.6B v3** (NVIDIA), int8 | sherpa-onnx + ONNX Runtime | Apache-2.0 / MIT | CC-BY-4.0 (attribution) | oui / oui / oui (25 langues européennes, sans le turc) | **Retenu** |
| FastConformer multilingue (NeMo, 10 langues) | sherpa-onnx | Apache-2.0 | CC-BY-4.0 | oui | Moins précis (C) |
| GigaAM v2 (Sber) | sherpa-onnx | Apache-2.0 | MIT | russe seulement | Écarté : une seule langue |
| Qwen3-ASR 0.6B | sherpa-onnx | Apache-2.0 | Apache-2.0 | oui | Écarté : écrit en chinois sur des annonces russes courtes, 2 à 3 s par clip |
| Omnilingual ASR 300M CTC (Meta) | sherpa-onnx | Apache-2.0 | Apache-2.0 | oui | Écarté : translittère le russe en alphabet latin, peu précis |
| SenseVoice small | sherpa-onnx | Apache-2.0 | licence du modèle FunASR | pas de russe | Écarté |
| Canary 180M Flash | sherpa-onnx | Apache-2.0 | CC-BY-4.0 | pas de russe | Écarté |
| Nemotron streaming | — | — | — | — | Absent du miroir sherpa-onnx, non testé |
| Vosk | Vosk / Kaldi | Apache-2.0 | Apache-2.0 | oui | Non testé : téléchargement bloqué depuis l'environnement de test |
| Whisper base / small | — | — | MIT | oui | **Exclu par décision** ; mesuré seulement comme référence |

**Sources** :
- Fiche Hugging Face `nvidia/parakeet-tdt-0.6b-v3` : licence CC-BY-4.0, WER
  FLEURS ru 5,51 %, en 4,85 %, fr 5,15 %.
- Export int8 `csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`.
- sherpa-onnx v1.13.8 (Apache-2.0, k2-fsa) ; ONNX Runtime (MIT, Microsoft) ;
  Silero VAD (MIT).
- CC-BY-4.0 autorise la redistribution et l'usage commercial, avec
  attribution. Celle-ci figure dans À propos (`LICENSES`), dans Audio & IA et
  dans l'installateur.

## C. Mesures

### Protocole

Scripts : `scripts/asr-bench/` (reproductibles, graine 7).

- **Corpus : 517 clips.**
  - Annonces tactiques en russe et en anglais : 4 voix Piper par langue,
    débit normal et rapide.
  - Dictée en français : 3 voix.
  - Conditions : `clean`, `combat` (tirs, explosions et pas synthétiques),
    `weak` (−24 dB avec bruit de pièce), chevauchement de deux voix.
  - 50 clips sans voix (combat seul, quasi-silence).
  - 2 enregistrements humains réels (en, fr) et leur version bruitée.
- **WER / CER** : texte normalisé (minuscules, sans ponctuation).
- **Annonce juste** : tous les mots clés présents (nombre, lieu).
- **Manquée** : texte vide alors que la voix existe.
- **Faux sur bruit** : un texte quelconque sur un clip sans voix.
- **Matériel** : Xeon 2,8 GHz, 4 cœurs, sans carte graphique.
- **`--load 4`** : 4 processus occupés à côté, pour imiter CS2.
- **Mémoire** : RSS du processus, un modèle par processus.

### Résultats (4 fils, sans charge sauf mention)

| Modèle | WER | CER | Annonces justes | Manquées | Faux sur bruit | p50 / p95 par clip | RAM chargé / pic | Chargement |
|---|---|---|---|---|---|---|---|---|
| **Parakeet TDT 0.6B v3 int8** | **46,6 %** | 35,1 % | **181/448** | 7/464 | **1/50** | 480 / 690 ms | 776 / 876 Mo | 6,1 s |
| Parakeet, 2 fils | 46,6 % | 35,1 % | 181/448 | 7/464 | 1/50 | 296 / 410 ms | 775 / 872 Mo | 3,4 s |
| Parakeet, 4 fils + charge 4 | 46,6 % | 35,1 % | 181/448 | 7/464 | 1/50 | 961 / 2170 ms | 776 / 877 Mo | 3,3 s |
| FastConformer multi fp32 | 57,2 % | 37,0 % | 115/448 | 6/464 | 0/50 | 164 / 217 ms | 547 / 627 Mo | 1,9 s |
| FastConformer multi int8 | 63,2 % | 43,1 % | 95/448 | 43/464 | 0/50 | 142 / 195 ms | 186 / 282 Mo | 1,5 s |
| GigaAM v2 (russe seul) | 78,1 % (ru : 45,1 %) | 66,8 % | 64/448 | 32/464 | 0/50 | 333 / 450 ms | 289 / 408 Mo | 1,6 s |
| *Référence* whisper-base | 60,7 % | 45,3 % | 131/448 | 3/464 | 50/50 | 278 / 412 ms | 201 / 407 Mo | 1,2 s |
| *Référence* whisper-small | 47,2 % | 34,0 % | 166/448 | 4/464 | 8/50 | 1086 / 2199 ms | 460 / 888 Mo | 2,6 s |
| *Référence* whisper-small, langue donnée | 43,8 % | 26,5 % | 169/448 | 0/464 | 50/50 | 1061 / 3029 ms | 1258 / 2415 Mo | 8,3 s |

Les temps de chargement varient d'une exécution à l'autre : 6,1 s et 3,3 s
pour le même Parakeet, à cause de la charge du conteneur.

### Par langue (Parakeet, 4 fils)

| Groupe | WER | Annonces justes |
|---|---|---|
| en, toutes | 31,6 % | 109/200 |
| en, `clean` | 17,5 % | 55/80 |
| en, `combat` | 38,2 % | 33/80 |
| fr, dictée `clean` | 19,6 % | 18/36 |
| fr, dictée `combat` | 60,9 % | 1/12 |
| ru, toutes | 67,9 % | 53/200 |
| ru, `clean` | 40,7 % | 33/80 |
| ru, `combat` | 79,9 % | 13/80 |
| ru, `weak` | 94,1 % | 7/40 |

Les enregistrements réels en anglais et en français sont transcrits mot pour
mot, avec ou sans bruit.

**Limite importante : le russe bruité ou faible.** Sur ces clips, Parakeet
écrit souvent en alphabet latin (« Vojen a B », « Ekarm. »). Il fait alors
moins bien que GigaAM (ru `combat` 49,5 %) et à peu près comme whisper-small
(73,0 %). Le corpus russe est entièrement synthétique, avec des voix Piper
au timbre peu naturel : la fiche du modèle donne 5,51 % de WER sur FLEURS
russe, des voix réelles. Ce point doit être vérifié en partie avant toute
conclusion (voir H).

### Silero VAD (ONNX)

- Parole détectée dans 455 des 468 clips qui en contiennent.
- Au moins un segment sur 4 des 50 clips sans voix : le filtre et le moteur
  ne produisent alors du texte que sur 1 clip (Parakeet).

### Chaîne complète de l'application

Mesure avec `crosstalk-asr-bench` (VAD, découpage, filtre, Parakeet),
40 annonces russes propres :
- WER 44,8 %, CER 33,9 % ;
- langue correcte pour 28 des 36 transcriptions gardées, 4 segments rejetés
  « tir / explosion » ;
- reconnaissance p50 253 ms / p95 353 ms, latence estimée p50 861 ms /
  p95 961 ms ;
- RTF 0,18.

Ce test a révélé un défaut, corrigé : l'API C de sherpa-onnx lit
`min_speech_duration = 0` comme « valeur par défaut » (0,25 s). La VAD
démarrait donc environ 8 trames trop tard et coupait le premier mot des
annonces courtes. La valeur est maintenant 0,001.

### Non mesuré

- Impact sur les FPS et utilisation du processeur avec CS2.
- Sessions longues (fuite mémoire, dérive de latence).
- Voix réelles de joueurs.

## D. Décision

**Parakeet TDT 0.6B v3 (int8) + sherpa-onnx 1.13.8 + Silero VAD (ONNX).**

Pourquoi :
- **Précision** : meilleur WER et le plus d'annonces justes parmi les
  candidats autorisés.
- **Fausses phrases** : 1 sur 50 clips sans voix, contre 50 sur 50 pour
  whisper-base.
- **Vitesse** : environ 2 fois plus rapide que whisper-small pour une
  précision équivalente.
- **Licences** : toutes compatibles avec la redistribution.
- **Fonctionnement** : entièrement local, sans clé.

Ce que coûte ce choix :
- **Taille** : 671 Mo à télécharger au lieu de 148 Mo pour `base`.
- **Mémoire** : environ 0,8 Go.
- **Russe** : faiblesse sur le russe bruité synthétique (C).
- **Turc** : non pris en charge (Audio & IA le signale).
- **Carte graphique** : plus d'option (ONNX Runtime CPU).
- **Indices de carte** : plus de prompt, donc plus de lieux suggérés au
  moteur via GSI.

Alternative si le russe réel déçoit : ajouter GigaAM v2 (MIT) pour le russe.
Il passe par le même runtime sherpa-onnx, il faudrait seulement choisir le
modèle selon la langue. Ce n'est pas fait ici.

## E. Architecture

```
capture → Conditioner → Silero VAD (sherpa-onnx, fenêtres de 512) → segmenter
        → file → SherpaRecognizer (Parakeet, ONNX Runtime CPU, ½ des cœurs, 1 à 4 fils)
        → langue lue dans le texte (chat::lang::detect, parmi les langues choisies)
        → filtre (vide, hallucinations, répétitions, débit, alphabet) → transcript
```

**Rust** (`src-tauri/src/speech/`) :
- `engine.rs` : trait `Recognizer` et `SherpaRecognizer` (préchauffage
  d'une seconde au chargement), fonction `identify`.
- `vad.rs` : `SileroVad` via `VoiceActivityDetector`, sortie binaire.
- `models.rs` : catalogue avec fichiers, tailles, SHA-256 et licence.
- `legacy.rs` : anciens fichiers Whisper.
- Supprimés : `gpu_guard.rs` et `service/fallback.rs`.

**Contrats Tauri** :
- Inchangés : `speech_status`, `speech_stats`, `speech_models`,
  `speech_configure`, `speech_rescan` et les événements `speech://*`.
- Ajoutés : `speech_legacy_whisper` et `speech_legacy_cleanup`.
- `SpeechStatus` : `model_label` remplace `fallback_from`.
- `Transcript` : sans `avg_logprob` ni `no_speech_prob`.
- `SpeechConfig` : sans `accel` ni `glossary`. Les anciens champs sont
  ignorés à la lecture.

Dictée, Répondre, voix Piper, radar, récap, historique et diagnostic passent
par les mêmes contrats et n'ont pas changé. Les tests Rust et UI qui les
couvrent passent.

**Build** :
- crates officielles `sherpa-onnx` et `sherpa-onnx-sys` ;
- copie locale de `sherpa-onnx-sys` (`src-tauri/vendor/`) : seule la
  dépendance de build `zip` passe de 2 à 9, toutes les versions 2.x étant
  retirées de crates.io ;
- bibliothèques précompilées officielles (`scripts/fetch-sherpa-onnx.sh`) ;
- sous Windows, trois DLL à côté de l'exécutable : `sherpa-onnx-c-api.dll`,
  `onnxruntime.dll`, `onnxruntime_providers_shared.dll`
  (`tauri.windows.conf.json`, script MinGW, `validate.ps1`) ;
- plus besoin de LLVM ni de CMake.

## F. Migration

- **Réglages** : un ancien `model` (`base`, `small`…) ou `accel` est ignoré,
  et Parakeet s'applique.
- **Téléchargement** : au lancement après la mise à jour, l'application
  télécharge le modèle manquant (bannière de préparation existante). Il est
  repris s'il est interrompu, puis vérifié par SHA-256.
- **Anciens fichiers** : les fichiers `ggml-*.bin` (modèles Whisper, ancienne
  VAD, `.part`) ne sont **jamais supprimés automatiquement**. Audio & IA
  affiche leur nombre et leur taille, avec un bouton « Libérer l'espace ». La
  suppression est journalisée.
- **Installateur** : télécharge Silero et Parakeet (5 fichiers, SHA-256,
  reprise), aucun poids Whisper.
- **`scripts/download-models.ps1`** : réécrit pour Parakeet (SHA-256, reprise
  avec `curl.exe -C -`). Un test Rust vérifie qu'il reste aligné sur le
  catalogue.
- **Retrait de Whisper** :
  - dépendances `whisper-rs` et `sha1_smol` ;
  - option `gpu`, `--gpu-probe`, shim Vulkan, contournement ggml du build
    MinGW ;
  - réglages CPU/GPU, bascule automatique, prompt et glossaire de
    reconnaissance.
  - Un test vérifie que `Cargo.toml` ne mentionne plus Whisper.
- **Interface Audio & IA** : nom du modèle, licence, état (absent,
  chargement, prêt avec backend, fils et temps de chargement, erreur),
  langues écoutées, avertissement turc, latence p50/p95, RTF et CPU.
- **Nouveautés** : une entrée pour les joueurs dans `release-notes.json`.

## G. Vérifications

| Contrôle | Résultat |
|---|---|
| `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (Linux) | OK |
| `cargo test --all-targets` (application) | OK, 288 tests |
| Installateur : fmt, clippy, `cargo test` | OK, 19 tests |
| `npx tsc --noEmit`, Vitest | OK, 247 tests |
| `npm run check:anticheat` | OK : aucune lecture mémoire, injection ou hook de `cs2.exe` |
| `npm run check:architecture` | OK (contrat IPC régénéré : +2 commandes) |
| `npm run check:release-notes` | OK |
| Build Windows MinGW (`scripts/cross-build-windows-gnu.sh`) | voir la PR |
| Lancement de `crosstalk.exe` sous Windows | **Non vérifié** |
| Build MSVC (`scripts/validate.ps1`) | **Non lancé** (pas de machine Windows) |
| Reconnaissance avec CS2 lancé, FPS, sessions longues | **Non vérifié** |

## H. Instructions

**Développement Linux** :
```bash
source <(scripts/fetch-sherpa-onnx.sh linux)   # dans chaque shell, avant cargo
cd src-tauri && cargo test --all-targets
```

**Windows** :
- Version officielle : `scripts\validate.ps1` (télécharge lui-même les
  bibliothèques).
- Paquet de test : `scripts/cross-build-windows-gnu.sh`.

**À tester en partie (PC de référence, CS2 lancé)**, en suivant
`scripts/cross/LISEZMOI-TEST.txt` et `docs/testing/speech-recognition.md` :
1. `crosstalk-asr-bench --noise-only 60` : attendu 0 transcription.
2. `crosstalk-asr-bench --live 300` pendant une partie avec des joueurs
   russophones : noter justes, partielles, fausses et manquées, la latence
   p50/p95 et le CPU. Ce test est le plus important vu la limite russe (C).
3. FPS sans et avec Crosstalk au même endroit, puis `--threads 2` et
   `--threads 4`.
4. Une session d'une heure : mémoire stable, latence stable.
5. Dans l'application : la carte « Reconnaissance vocale » doit être verte,
   puis le Script, la traduction, Répondre et la dictée doivent marcher.

## I. Git

- Branche `feat/asr-engine-replacement` créée depuis `develop`.
- PR vers `develop`, **sans fusion automatique**. `main` n'est pas touché.
- Pas de GitHub Actions : validation locale.
- Aucune voix privée, aucun pseudo ni aucune conversation dans le dépôt :
  corpus synthétique non versionné (`scripts/asr-bench/.gitignore`), et
  journaux utilisateurs seulement résumés ci-dessus.
