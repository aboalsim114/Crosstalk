# Vue d'ensemble

Crosstalk est une application Windows (Tauri v2) qui sous-titre, en direct et **localement**, les voix et le chat d'une partie de Counter-Strike 2. Elle compte deux exécutables :

- `crosstalk.exe` : le cœur Rust (`src-tauri/`) et trois fenêtres WebView2 (principale, overlay, Répondre) ;
- `crosstalk-setup.exe` : l'installateur (`src-installer/`), une application Tauri distincte, emballée dans une enveloppe NSIS.

Documents liés :
- [modules.md](modules.md) : le rôle de chaque module ;
- [dependencies.md](dependencies.md) : les règles de dépendance et leur contrôle ;
- [data-flow.md](data-flow.md) : le chemin d'une voix jusqu'au sous-titre, files et purges ;
- [decisions.md](decisions.md) : les choix et leurs raisons ;
- [audit.md](audit.md) : l'état de départ ;
- [refactoring-report.md](refactoring-report.md) : ce qui a changé.

## Principes

1. **Monolithe modulaire** ([D-01](decisions.md)). Une crate pour l'application, des modules par fonctionnalité, des traits aux frontières. Pas de microservices, pas de plugins, pas de workspace sans gain mesuré.
2. **Rust détient l'état réel.** Les fenêtres React envoient des commandes et écoutent des événements ; elles ne décident ni des files, ni des modèles, ni des purges.
3. **Temps réel d'abord.** Files bornées, abandon du plus ancien, purge par génération, threads de calcul en priorité basse. La refactorisation n'ajoute aucune allocation au chemin audio. Celui-ci en fait déjà une par bloc : le tampon mono de `SpeechSink::block` ([audit A21](audit.md)).
4. **Tout reste sur le PC.** Aucun audio écrit sur disque par défaut, aucune donnée envoyée à un service en ligne. Seuls les modèles sont téléchargés depuis Hugging Face.
5. **Aucune interaction intrusive avec CS2.** Aucune injection dans CS2. Aucune lecture mémoire du jeu. Aucun hook interne. Aucun contournement anti-triche. Aucun changement intrusif de la capture. Sources : `console.log` (`-condebug`), capture audio WASAPI par processus, Game State Integration, capture de fenêtre (optionnelle). Vérifié par `npm run check:anticheat`.

## Schéma cible

```
                            ┌──────────────────── crosstalk.exe — cœur Rust (src-tauri/src) ───────────────────┐
                            │                                                                                   │
 cs2.exe (audio, WASAPI) ─► │ audio ──BlockSink──► speech ──SpeechEvents──┐                                     │
                            │ (capture)            (VAD, Parakeet)         │                                    │
 console.log ─────────────► │ chat ──ChatEvents──────────────────────────┤                                     │
                            │                                              ▼                                    │
 Game State Integration ──► │ gsi                                       wiring  (composition : la seule colle   │
                            │                                              │      entre services)               │
 fenêtre CS2 (optionnel) ─► │ speaker ◄────────────────────────────────────┤                                    │
                            │                                              ▼                                    │
                            │                                         translation ──TranslationEvents──┐        │
                            │                                         (Opus-MT, cache, pivot EN)       │        │
                            │                                                                          │        │
                            │ reply · dictation · history · stream · recap · updater · logs · settings │        │
                            │ ─────────────────────────────────────────────────────────────────────────┤        │
                            │ partagés : paths · cpu · version · downloads · catalogues de modèles     │        │
                            └──────────────────────────────────────────────────────┬───────────────────┘
                                                                                   │ commandes ▲ ▼ événements
                                             ┌─────────────────────────────────────┼────────────────────────┐
                                             ▼                                     ▼                        ▼
                                  fenêtre principale                       overlay (lecture seule)   Répondre (pop-up)
                                  src/app + src/features                   src/windows/overlay       src/windows/reply
```

## Organisation du dépôt (cible)

```
src-tauri/src/
  lib.rs              création des services et de la fenêtre (composition)
  wiring.rs           colle entre services : capture → purge, transcription → locuteur → traduction, chat → traduction
  paths.rs cpu.rs version.rs          utilitaires feuilles (aucune dépendance interne)
  audio/              capture WASAPI, niveaux, plan de capture
  speech/             VAD, segmentation, reconnaissance (sherpa-onnx), filtre
    service/          mod.rs (API) · queue.rs · stats.rs · front.rs (VAD) · asr.rs
  translation/        catalogue, glossaire, SentencePiece, Marian
    engine.rs         contrats Engine / EngineLoader, moteur pivot Chain
    opus.rs           chargement des modèles Opus-MT
    service/          mod.rs (API) · config.rs · cache.rs · worker.rs
  chat/ gsi/ speaker/ reply/ stream/ …   un module par fonctionnalité
  downloads.rs        téléchargements reprenables (partagé avec l'installateur)

src-installer/src/
  main.rs             commandes Tauri, mode silencieux, fichiers partagés (#[path])
  install.rs          déroulé de l'installation (phases)
  system.rs           Windows : registre, raccourcis, processus

src/
  app/                point d'entrée de la fenêtre principale : App, coquille (barre de titre, menu), store de session, ponts natifs
  features/<nom>/     une fonctionnalité : page ou cartes, état (Zustand), règles
  shared/
    ipc/              appels Tauri typés, un fichier par domaine
    i18n/             langues de l'interface
    ui/               atomes visuels
  windows/
    overlay/          fenêtre overlay
    reply/            fenêtre Répondre
  installer/          interface de l'installateur (bundle séparé, vite.installer.config.ts)
```

Fonctionnalités du frontend : `audio`, `speech`, `translation`, `languages`, `chat`, `session` (tableau de bord et fil), `overlay` (réglages), `reply`, `history`, `recap` (GSI et récapitulatif), `stream`, `speaker`, `update`, `setup` (téléchargement des modèles), `diagnostics`, `about`.
