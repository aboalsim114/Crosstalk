# Flux de données

## Une voix, de CS2 au sous-titre

```
cs2.exe ──WASAPI (par processus)──► audio-capture
                                       │ AudioBlock (≈ 20 ms, 48 kHz)
                                       ▼
                         SpeechSink::block            mono (ou « Focus voix » en stéréo)
                                       │ try_send, canal borné de 256 blocs
                                       │ plein → bloc abandonné, compteur dropped_blocks
                                       ▼
                         speech-vad (front_loop)       conditionnement 16 kHz, VAD Silero, segmentation
                                       │ JobQueue : 3 segments finaux (plein → le plus ancien est retiré : Overflow)
                                       │            + 1 emplacement de ligne provisoire (remplacé par la plus récente)
                                       ▼
                         speech-asr (priorité normale) Parakeet (sherpa-onnx), langue lue dans le texte, filtre anti-hallucinations
                                       │ SpeechEvents::transcript
                                       ▼
                         wiring                        1. nom du locuteur (speaker, si activé)
                                                       2. émet speech://transcript
                                                       3. soumet à la traduction : provisoire → submit_partial, finale → submit
                                       ▼
                         translate (priorité basse)    file de 8 (pleine → le plus ancien sans réponse attendue échoue « file pleine »)
                                                       1 ligne provisoire au plus ; les finales passent d'abord
                                                       lexique d'annonces → cache (1 000, par langue cible) → Opus-MT (direct ou via l'anglais)
                                       │ TranslationEvents::result
                                       ▼
                         translation://result ──► fenêtre principale (fil, sous-titre) · overlay · mode streamer
```

## Le chat

```
console.log (-condebug) ──► chat-tail (toutes les 250 ms) ──► analyse, langue du message
                                                                   │ ChatEvents::message
                                                                   ▼
                                                     wiring : émet chat://message, nom connu pour « Qui parle ? »,
                                                              soumet à la traduction si la langue fait partie du catalogue
```

Les identifiants du chat commencent à `1 << 40` (`chat/mod.rs`) : la traduction reçoit les deux flux et répond par identifiant. Les identifiants vocaux ne l'atteignent jamais en pratique.

## Purges et arrêts

| Déclencheur | Effet |
|---|---|
| La capture quitte l'état `Capturing` (arrêt, arrêt d'urgence, CS2 fermé) | `wiring::CaptureEvents` appelle `speech.purge()` puis `translation.purge()` |
| `speech.purge()` | incrémente la génération : blocs, segments et transcriptions en cours sont ignorés à la publication (verrou `publication`) |
| `translation.purge()` | incrémente la génération ; garde seulement les demandes manuelles (test, Répondre) |
| Changement de langue cible | la traduction vide sa file, son cache et libère les moteurs des langues abandonnées |

## Commandes et événements

La liste exacte des commandes Tauri et des événements est figée dans [`ipc-contract.txt`](ipc-contract.txt) ([D-10](decisions.md)).

## Données persistées

Aucune n'est modifiée par la refactorisation.

| Donnée | Emplacement | Format |
|---|---|---|
| Réglages | `%APPDATA%\Crosstalk\config.json` (`CROSSTALK_CONFIG_DIR`) | JSON versionné, migré par `src/app/persist.ts` |
| Glossaire | `%APPDATA%\Crosstalk\glossary.json` | JSON |
| Règles du chat | `%APPDATA%\Crosstalk\chat-rules.json` | JSON |
| Historique | `%LOCALAPPDATA%\Crosstalk\history.jsonl` (`CROSSTALK_HISTORY_DIR`) | texte seulement, désactivé par défaut |
| Journal | `%LOCALAPPDATA%\Crosstalk\logs\` (`CROSSTALK_LOG_DIR`) | texte, 7 jours |
| Modèles | `%LOCALAPPDATA%\Crosstalk\models\` (`CROSSTALK_MODELS_DIR`) | fichiers d'origine (Hugging Face, GitHub pour `silero_vad.onnx`) ; les anciens `ggml-*.bin` de Whisper restent jusqu'à « Libérer l'espace » |
| Installation | `HKCU\Software\Crosstalk` (`InstallDir`, `Version`) | registre |

Aucun audio n'est écrit sur disque.
