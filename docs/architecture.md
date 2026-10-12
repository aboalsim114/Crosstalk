# Architecture — Crosstalk

Complète le [cahier des charges](cahier-des-charges.md) (§6). Ce document décrit l'architecture **cible**. La section 7 indique ce qui existe déjà dans le dépôt et le chemin pour y arriver.

## 1. Principes

1. **Rust gère l'état de session.** Le cœur Rust est la seule source de vérité : session, pipeline, réglages effectifs. React (fenêtre principale) et l'overlay sont des **vues**. Ils envoient des commandes et reçoivent des événements. Le traitement continue quand la fenêtre principale est masquée dans la zone de notification.
2. **Modules indépendants** : capture, nettoyage, reconnaissance, traduction, identification du locuteur. Chacun peut tomber en panne sans entraîner les autres ni l'application.
3. **Files bornées partout.** En cas de saturation, on abandonne les données les plus anciennes plutôt que d'accumuler du retard (cible : sous-titres utiles, pas exhaustifs).
4. **Horloge commune** : toutes les données horodatées utilisent l'horloge `QueryPerformanceCounter` (QPC) de Windows.
5. **Traduction d'abord, pseudo ensuite.** L'identification du locuteur n'est jamais sur le chemin critique de la traduction.
6. **Aucune interaction intrusive avec le jeu** (cahier des charges §3) : `console.log`, WASAPI, Windows Graphics Capture, Game State Integration, fenêtre séparée. Ni lecture mémoire, ni injection, ni hook.

## 2. Vue d'ensemble

```
                    ┌──────────────────────────── cœur Rust (src-tauri) ────────────────────────────┐
                    │                                                                               │
 cs2.exe audio ──►  │ [capture] ─► [nettoyage + VAD + segmentation] ─► [asr] ─┐                     │
 (WASAPI)           │   thread       thread DSP                     thread    │                     │
                    │   temps réel                                  inférence ├─► [translation] ─┐  │
 console.log ──►    │ [chat-ingest] ──────────────────────────────────────────┘    tâches tokio   │  │
 (-condebug)        │                                                                             ▼  │
                    │                                                       [session] (état, ids, │  │
 fenêtre CS2 ──►    │ [speaker] : capture HUD ─► indicateur ─► OCR ─► corrélation ─► machine à états)│
 (WGC, exp.)        │   thread dédié, désactivé par défaut              speaker.resolved          │  │
                    │                                                                             │  │
 Game State Int. ►  │ [gsi] serveur HTTP local (carte, phase de round) ──► glossaire par carte    │  │
                    └──────────────────────────────────────────────────────────────┬──────────────┘  
                                                                                   │ événements Tauri typés
                                                         ┌─────────────────────────┴──────────────────┐
                                                         ▼                                            ▼
                                              fenêtre principale (React)                    fenêtre overlay (React)
                                              commandes : start/stop/pause,                 lecture seule
                                              réglages, glossaire, diagnostics
```

## 3. Modules (crate `crosstalk_lib`)

Le découpage se fait d'abord en **modules** de la crate existante (`src-tauri/src/`). On passera à un workspace multi-crates seulement si les temps de compilation ou les tests isolés le justifient.

| Module | Responsabilité | Exécution |
| --- | --- | --- |
| `session` | Machine à états, identifiants d'énoncés, diffusion des événements, arrêt d'urgence | tâche tokio |
| `cs2` | Bibliothèques Steam, PID de `cs2.exe`, `-condebug`, chemin de `console.log` | tâche tokio (sondage 2 s) |
| `chat` | Suivi de `console.log`, parseur versionné, pseudos connus | tâche tokio |
| `audio::capture` | WASAPI process loopback (repli : périphérique), format imposé 48 kHz float | **thread dédié**, priorité MMCSS « Pro Audio » |
| `audio::dsp` | Passe-bande, rééchantillonnage 16 kHz, (exp.) mid/side, DeepFilterNet | thread DSP |
| `audio::vad` | VAD Silero (ONNX), validation des segments, pré-roll et post-roll, segments ≤ 8 s | thread DSP |
| `asr` | Trait `Transcriber`, whisper.cpp, filtre anti-hallucinations | **thread dédié**, priorité basse |
| `translation` | Opus-MT local (candle, Rust pur) : tokenizer SentencePiece, Marian, glossaire protégé, cache LRU, délai et repli | thread `translate`, priorité basse |
| `speaker` (exp.) | Windows Graphics Capture, détection de l'indicateur, OCR, corrélation, confiance | thread dédié, ~10 Hz, désactivé par défaut |
| `gsi` | Serveur HTTP local Game State Integration : carte, phase de round | tâche tokio |
| `overlay`, `tray`, `shortcuts`, `actions` | Intégration Windows (existants) | thread principal Tauri |
| `storage` | Réglages, glossaire, historique SQLite, modèles | tâche tokio |

Les traits `AudioCapture`, `Transcriber`, `Translator` et `ChatSource` existent déjà dans `pipeline/`. Ils permettent de tester chaque module avec des implémentations factices et d'alimenter le banc d'essai hors ligne avec des fichiers WAV.

### Installateur (`src-installer/`)

Application Tauri séparée, à l'interface tirée de la maquette « Crosstalk Installer » (`src/installer/`, page `installer.html`, bundle `dist-installer/`). Elle embarque les fichiers de l'application, compressés en deflate par `build.rs` à partir de `CROSSTALK_PAYLOAD_DIR`, et compile tels quels `downloads.rs` et les catalogues de modèles de l'application. Le code Tauri des téléchargements est séparé dans `download_commands.rs`.

| Phase (UI) | Travail réel |
| --- | --- |
| Préparation 0–12 % | Espace libre (application et modèles), Crosstalk fermé |
| Extraction 12–62 % | Fichiers copiés dans un dossier temporaire, puis téléchargement des modèles manquants, avec une progression pondérée par la taille |
| Configuration 62–88 % | Fichiers mis en place, `HKCU\Software\Crosstalk` (`InstallDir`, `Version`) |
| Finalisation 88–100 % | Raccourcis du menu Démarrer et du bureau |

Annuler pendant les deux premières phases supprime le dossier temporaire : une version déjà installée reste intacte et les modèles partiels reprendront. `Crosstalk-setup.exe` est une enveloppe NSIS sans fenêtre (`scripts/cross/setup-wrapper.nsi`). Elle installe WebView2 s'il manque, lance l'installateur puis, si celui-ci renvoie 0, écrit `uninstall.exe` et l'entrée « Applications ». `/S` installe sans interface (`--silent`).

## 4. Files et concurrence

| De → vers | Type | Capacité | Politique en cas de saturation |
| --- | --- | --- | --- |
| capture → dsp | anneau SPSC (sans allocation, sans verrou) | ~2 s d'audio | écrase le plus ancien, compteur `audio_overruns` |
| vad → asr | canal borné | 3 segments | abandonne le plus ancien (F-27), événement de diagnostic |
| asr → translation | canal tokio borné | 16 | abandonne le plus ancien |
| chat → translation | canal tokio borné | 64 | abandonne le plus ancien |
| speaker → session | canal tokio borné | 32 | abandonne le plus ancien |
| session → UI | événements Tauri | — | l'overlay ne garde que les N derniers blocs |

- Le thread de capture ne fait **aucune** allocation ni opération bloquante.
- Whisper n'est **jamais** exécuté sur un worker tokio : il est bloquant et long.
- L'arrêt d'urgence ferme toutes les sources et vide toutes les files en moins de 200 ms.

## 5. Horloge commune et latence

- **Audio** : `IAudioCaptureClient::GetBuffer` fournit la position QPC de chaque paquet.
- **Image** : `Direct3D11CaptureFrame::SystemRelativeTime` est sur la même base QPC.
- Chaque segment vocal porte `speech_start_qpc` et `speech_end_qpc`. Chaque observation de l'indicateur vocal porte un intervalle QPC.
- **Latence voix mesurée** = horodatage de l'affichage final (événement `utterance.final` émis) − `speech_end_qpc`. C'est la définition du cahier des charges : fin de parole → affichage final.
- Les percentiles p50 et p95 sont calculés par le module `session` et affichés dans l'écran Accueil (carte « Latence voix »).

## 6. Contrat d'événements (Rust → interface)

Événements typés (`serde`, `camelCase`), versionnés par un champ `v`. Ils reprennent les notions déjà présentes dans l'interface (`FeedEntry` : statuts provisoire, final, original, erreur ; latence) et dans `pipeline::Utterance`.

| Événement | Contenu principal | Usage |
| --- | --- | --- |
| `session.state` | `state` (IDLE … DEGRADED), raison, `startedAt` | badge, timecode, tray |
| `utterance.provisional` | `id`, `source` (voice\|chat), `lang`, `original`, `text` | sous-titre provisoire |
| `utterance.final` | `id`, `source`, `channel?`, `lang`, `original`, `translated \| null`, `status` (final\|original\|erreur), `latencyMs`, `sender?` | sous-titre final ; `sender` présent pour le chat |
| `speaker.resolved` | `id` de l'énoncé, `speaker` : `{ name \| null, confidence, method: "hud_ocr" \| "chat" \| "discord" }` | ajoute le pseudo, ou « Joueur inconnu » si `name` est nul |
| `pipeline.stage` | étape active (capture, vad, asr, traduction, overlay) | barre de progression de l'Accueil |
| `audio.level` | niveau RMS, `speaking` (≤ 15 Hz) | anneau d'onde 3D, indicateur VAD |
| `diagnostic` | composant, niveau, message expurgé | écran Diagnostic |

Commandes (interface → Rust) : `session_start`, `session_stop`, `session_pause`, `emergency_stop`, `settings_get` / `settings_set`, `glossary_*`, `model_download`, `audio_test`, `diagnostics_run`, plus `overlay_configure` (existante).

Exemple :

```json
{ "v": 1, "type": "utterance.final", "id": "u-0193", "source": "voice", "lang": "ru",
  "original": "Двое на точке Б!", "translated": "Deux sur le point B !", "status": "final",
  "latencyMs": 1780 }
{ "v": 1, "type": "speaker.resolved", "id": "u-0193",
  "speaker": { "name": null, "confidence": 0.41, "method": "hud_ocr" } }
```

Dans cet exemple, la confiance est insuffisante : l'interface affiche « Joueur inconnu ».

## 7. État actuel et migration

| Élément | Aujourd'hui | Cible |
| --- | --- | --- |
| État de session, flux, simulation | Store Zustand dans la fenêtre principale React (`src/state/store.ts`) | Module Rust `session`. Le store React devient un miroir alimenté par les événements |
| Overlay | Reçoit son état de la fenêtre principale (`src/native.ts`) | Reçoit les événements directement de Rust |
| Raccourcis, tray, overlay, autostart | Rust ✅ | Inchangé ; les actions appellent directement `session` |
| Pipeline réel | Traits seulement (`src-tauri/src/pipeline/`) | Implémentations lot par lot (L0 → L4) |
| Fenêtre masquée | Le WebView peut ralentir ses minuteries en arrière-plan | Sans effet sur le pipeline (Rust). Option Tauri `backgroundThrottling` à évaluer pour l'overlay |

**Migration en trois étapes :**
1. Introduire `session` en Rust, en reproduisant la machine à états actuelle avec ses tests.
2. Faire émettre par Rust les événements simulés : le simulateur passe côté Rust, derrière une option de développement.
3. Remplacer la simulation par le pipeline réel, une source à la fois.

À chaque étape, les tests React existants restent verts.

## 8. Identification du locuteur

Elle est volontairement isolée (cahier des charges §4.8) :
- elle consomme des observations « indicateur actif de t1 à t2 » et des segments vocaux horodatés ;
- elle produit, au plus tard quelques centaines de millisecondes après la traduction, un `speaker.resolved` ;
- un pseudo n'est retenu que s'il figure dans la liste des pseudos connus de la session, si un seul indicateur chevauche le segment, et si le score dépasse le seuil réglable. Sinon : `name: null`, donc « Joueur inconnu ».

Aucune image n'est conservée. Les pseudos sont expurgés des journaux.
