# Dépendances

## Règles

### Cœur Rust
1. **Aucun cycle entre les modules de premier niveau** de `src-tauri/src`. Un module peut en utiliser un autre, à condition que l'inverse ne soit pas vrai, ni directement ni par un détour.
2. **Les services ne s'appellent pas entre eux en réaction à un événement.** Cette colle est dans `wiring.rs` ([D-03](decisions.md)). Exception assumée : `gsi`, qui lit l'état des autres services pour son propre statut et pousse le glossaire de la carte.
3. **Utilitaires feuilles** : `paths`, `cpu`, `version` n'importent aucun module de la crate.
4. **Fichiers partagés avec l'installateur** (`downloads.rs`, `paths.rs`, `speech/models.rs`, `translation/models.rs`) : ils n'importent que d'autres fichiers partagés ([D-08](decisions.md)).
5. **Domaine et application sans Tauri** : `speech/service/`, `translation/service/`, `audio/service.rs`, les catalogues et les règles pures n'importent pas `tauri`. Les adaptateurs `Tauri*Events` et les commandes sont dans `<module>/mod.rs` ou dans `wiring.rs`.

### Interface
1. **Aucun cycle d'import** à l'exécution (imports `type` exclus : TypeScript les efface).
2. **La couche IPC (`src/shared/ipc`, anciennement `src/backend`) n'importe rien de l'interface**, pas même un type.
3. **Les fenêtres secondaires** (`windows/overlay`, `windows/reply`) n'importent pas le store de la fenêtre principale. *Objectif* : l'overlay l'importe encore indirectement par `OverlayPage` ([audit A10](audit.md)).
4. **Une fonctionnalité** n'importe d'une autre que ses règles pures ou son store, jamais ses composants internes.

## Contrôle

```bash
npm run check:architecture
```

`scripts/check-architecture.mjs`, sans dépendance externe, vérifie :
- la règle Rust 1 (algorithme de Tarjan sur les `crate::<module>` hors tests) ;
- la règle Rust 4 ;
- les règles d'interface 1 et 2 ;
- le contrat IPC ([D-10](decisions.md)).

`scripts/validate.ps1` le lance avec les autres contrôles locaux. Les règles Rust 2, 3 et 5 se vérifient à la relecture : `grep -n "crate::" src-tauri/src/paths.rs` doit être vide.

## Graphe des modules Rust

### Avant ([audit §1.2](audit.md))
Un seul bloc de 7 modules liés en cycle : `downloads ↔ logs ↔ reply ↔ speaker ↔ speech ↔ translation ↔ updater`.

### Après
Aucun cycle. Les arêtes principales (`A → B` : A utilise B) :

```
wiring ──► speech, translation, speaker, chat, audio
gsi ──► chat, history, reply, speech, translation
speech ──► audio, logs, settings, version, paths, cpu
translation ──► paths, cpu
reply ──► translation, paths
speaker ──► logs, overlay, reply
chat ──► logs
logs ──► audio, history, version, paths
updater ──► downloads, version
downloads ──► speech::models, translation::models, paths
audio ──► game
```

## Dépendances externes

| Crate | Pourquoi |
|---|---|
| `tauri` 2 + `single-instance`, `autostart`, `global-shortcut` | fenêtres, IPC, plateau, raccourcis |
| `windows` / `windows-core` | WASAPI, fenêtres, OCR, registre, priorité des threads |
| `whisper-rs` | reconnaissance vocale (whisper.cpp), VAD Silero |
| `candle-core`, `candle-nn`, `candle-transformers` | Marian Opus-MT en Rust pur |
| `ureq`, `sha1_smol`, `sha2` | téléchargements et vérification |
| `serde`, `serde_json`, `thiserror`, `regex` | données et erreurs |

`src-installer` y ajoute `flate2` (extraction), `winreg` et `mslnk` (registre, raccourcis), et le plugin `dialog`. Le frontend dépend de React 19, Zustand 5, `motion`, `three` et `@tauri-apps/api`.
