# Modules

## Cœur Rust (`src-tauri/src`)

| Module | Rôle | Threads | Événements émis |
|---|---|---|---|
| `lib.rs` | Création des services, des fenêtres, du plateau ; liste des commandes | principal | — |
| `wiring` | Colle entre services ([D-03](decisions.md)) : une capture qui s'arrête purge parole et traduction ; une transcription est nommée (`speaker`), émise, puis traduite ; un message du chat est traduit | ceux des services appelants | `speech://transcript`, `speech://activity`, `chat://message` |
| `paths` | Dossier des modèles (`CROSSTALK_MODELS_DIR`, sinon `%LOCALAPPDATA%\Crosstalk\models`) | — | — |
| `cpu` | Temps CPU du processus, priorité basse des threads de calcul, `CpuMeter` | — | — |
| `version` | Numéro de build (`CROSSTALK_BUILD`) | — | — |
| `audio` | Capture WASAPI de `cs2.exe` (par processus, repli périphérique), niveaux, micro de la dictée | `audio-capture` (un par démarrage) | `audio://status`, `audio://level` |
| `game` | Détection de `cs2.exe` (noms et PID uniquement) | `cs2-watcher` | `cs2://status` |
| `speech` | Conditionnement, VAD Silero, segmentation, reconnaissance Parakeet (sherpa-onnx), filtre anti-hallucinations, statistiques, anciens fichiers Whisper (`legacy.rs`) | `speech-vad`, `speech-asr` (priorité normale), `speech-load` | `speech://status`, `speech://stats`, `speech://retract` (+ ceux de `wiring`) |
| `translation` | Catalogue Opus-MT, routage (direct ou pivot anglais), SentencePiece, Marian (candle), glossaire, lexique d'annonces, cache | `translate` (priorité basse) | `translation://status`, `translation://result` |
| `chat` | Détection Steam / CS2, `-condebug`, suivi de `console.log`, analyse, langue du message | `chat-tail` | `chat://status` (+ `chat://message` via `wiring`) |
| `gsi` | Serveur local Game State Integration : carte, phase de round | `gsi` | `gsi://status` |
| `speaker` | « Qui parle ? » : indicateur vocal de CS2, OCR (désactivé par défaut) | `speaker` | `speaker://status` |
| `reply` | « Répondre » : traduction de la langue de l'utilisateur vers celle des coéquipiers, fenêtre pop-up | à la demande | `reply://open` |
| `dictation` | Dictée de la réponse (micro → reconnaissance vocale) | `dictation` | `dictation://state` |
| `downloads`, `download_commands` | Téléchargements reprenables, vérifiés par taille et empreinte | `downloads` | `models://download` |
| `updater` | Mise à jour intégrée : flux, téléchargement vérifié, installation silencieuse | `update` | `update://status` |
| `history` | Historique local, texte seulement, désactivé par défaut | — | — |
| `stream` | Mode streamer : page locale pour OBS | `stream` + un par client | — |
| `recap` | Enregistrement de la carte de fin de match | — | — |
| `logs` | Journal de diagnostic (sans nom d'utilisateur), rapport | — | — |
| `settings` | Réglages sur disque (`%APPDATA%\Crosstalk`) | — | — |
| `overlay`, `tray`, `shortcuts`, `tap_keys` (boutons latéraux Mouse 4 / Mouse 5, `GetAsyncKeyState`), `actions`, `clipboard`, `commands` | Intégration Windows | principal | `crosstalk://action` |

## Installateur (`src-installer/src`)

| Fichier | Rôle |
|---|---|
| `main.rs` | Commandes Tauri, mode silencieux (`--silent`), inclusion des fichiers partagés par `#[path]` |
| `install.rs` | Déroulé de l'installation : espace libre, extraction, modèles, mise en place, registre, raccourcis. Choix des modèles de traduction selon la langue de Windows |
| `system.rs` | Implémentation Windows du trait `Platform` : registre, raccourcis, processus en cours, langue de l'utilisateur |

Fichiers de l'application compilés dans l'installateur : `downloads.rs`, `paths.rs`, `speech/models.rs` et `translation/models.rs`. Ils ne doivent rien importer d'autre ; c'est vérifié par `npm run check:architecture`.

## Interface (`src`)

| Dossier | Rôle |
|---|---|
| `app/` | Fenêtre principale : `main.tsx`, `App.tsx`, coquille (barre de titre, menu, en-tête, intro, scène 3D), store de session (`useApp`), persistance, ponts natifs (`native.ts`) |
| `features/<nom>/` | Une fonctionnalité : composants, store Zustand en miroir de l'état Rust, règles pures |
| `shared/ipc/` | Appels `invoke` et écoutes `listen` typés, un fichier par domaine ; `index.ts` réexporte tout |
| `shared/i18n/` | Langues de l'interface (9), `useT`, noms des langues |
| `shared/ui/` | Atomes visuels (`fnt`, `Toggle`, `LevelMeter`…) |
| `windows/overlay/`, `windows/reply/` | Points d'entrée des fenêtres secondaires |
| `installer/` | Interface de l'installateur, bundle séparé |

Règles d'état :
- un store Zustand par source de vérité : l'état d'un service Rust a **un** miroir (`useSpeech`, `useTranslation`, `useChat`…) ;
- `useApp` porte la session, le fil et les réglages de l'utilisateur ;
- les composants lisent les stores et appellent `shared/ipc`. Les ponts de `app/native.ts` synchronisent les réglages vers Rust.

## Contrats des moteurs d'IA

Ces traits existent et sont testés avec des implémentations factices ([D-12](decisions.md)). Remplacer un moteur, c'est fournir une implémentation, sans toucher au service.

| Rôle | Trait | Fichier | Implémentation réelle | Faux (tests) |
|---|---|---|---|---|
| Détection de voix | `VoiceDetector` | `speech/vad.rs` | `SileroVad` (Silero v5 ONNX, via sherpa-onnx), repli `EnergyDetector` | `GatedDetector`, `EnergyDetector` |
| Reconnaissance | `Recognizer` | `speech/engine.rs` | `SherpaRecognizer` (Parakeet TDT 0.6B v3 via `sherpa-onnx`, processeur seul) | `EchoRecognizer`, `Script`, `CheckThreads` |
| Chargement ASR | `speech::service::EngineLoader` | `speech/service/mod.rs` | `SherpaLoader` (`speech/mod.rs`) | `FakeLoader`, `GatedLoader` |
| Amélioration du signal | `Conditioner` (struct) | `speech/dsp.rs` | passe-bande, rééchantillonnage 16 kHz | — (pur, déterministe) |
| Traduction | `Engine` | `translation/engine.rs` | `Marian` Opus-MT (`translation/marian.rs`) ; `Chain` pour le pivot anglais | `Upper`, `Dash`, `Echo` (reply) |
| Chargement traduction | `translation::EngineLoader` | `translation/engine.rs` | `OpusLoader` (`translation/opus.rs`) | `Loader` (tests) |
| Catalogue de modèles | données | `speech/models.rs`, `translation/models.rs` | Hugging Face, tailles et SHA-256 figés | — |
| Téléchargement | `Fetcher`, `DownloadEvents` | `downloads.rs` | `HttpFetcher` (ureq) | `Fake`, `Serve` (updater) |
| Capture audio | `CaptureBackend`, `CaptureStream`, `BlockSink` | `audio/backend.rs`, `audio/service.rs` | `WasapiBackend` (`audio/wasapi.rs`) ; `SpeechSink` | `FakeBackend`, `Count` |
| Diagnostic | `SpeechStats`, `TranslationStatus`, `logs::summarize` | — | journal et rapport | — |

Les fichiers de modèles déjà installés (`%LOCALAPPDATA%\Crosstalk\models\…`) gardent leurs noms et leurs emplacements.
