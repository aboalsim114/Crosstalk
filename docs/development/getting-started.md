# Démarrer

## Prérequis

| Outil | Version | Pour |
|---|---|---|
| Windows 10/11 x64 | — | lancer l'application, construire la version officielle (MSVC) |
| Node.js | 22 | interface (Vite, Vitest) |
| Rust stable, toolchain MSVC | — | cœur (`src-tauri`) et installateur (`src-installer`) |
| Visual Studio Build Tools (C++) | — | édition de liens MSVC |
| Bibliothèques précompilées sherpa-onnx (API C + ONNX Runtime) | 1.13.8 | reconnaissance vocale ; téléchargées par `scripts/fetch-sherpa-onnx.sh` ou `validate.ps1` |

Sous Linux, l'interface et les tests (`cargo test`, Vitest) fonctionnent. Avant `cargo`, chargez les bibliothèques Linux de sherpa-onnx : `source <(scripts/fetch-sherpa-onnx.sh linux)` (elles vont dans `src-tauri/target/sherpa-onnx/linux/` et le script exporte `SHERPA_ONNX_LIB_DIR`). La compilation croisée vers Windows passe par `scripts/cross-build-windows-gnu.sh` (MinGW, voir l'en-tête du script). Le binaire produit n'est **pas** la version officielle.

## Premiers pas

```powershell
npm install
npm run app:dev            # application Tauri, rechargement à chaud
npm run dev                # interface seule dans un navigateur (sans cœur Rust : données de démonstration)
npm run dev:installer      # interface de l'installateur seule
```

Modèles : l'application télécharge elle-même ce qui manque (Langues, Audio & IA). Pour pointer vers un dossier existant :

```powershell
$env:CROSSTALK_MODELS_DIR = 'D:\models'
```

## Où est quoi

- Vue d'ensemble : [docs/architecture/overview.md](../architecture/overview.md).
- Rôle de chaque module : [docs/architecture/modules.md](../architecture/modules.md).
- Ajouter une fonctionnalité :

  | Côté | Emplacement |
  |---|---|
  | Rust | un module dans `src-tauri/src/<nom>/` ; ses commandes dans `mod.rs` ; s'il réagit à un autre service, la colle va dans `wiring.rs` |
  | Interface | `src/features/<nom>/` ; ses appels Tauri dans `src/shared/ipc/<nom>.ts` |
  | Contrat | les nouvelles commandes et les nouveaux événements s'ajoutent à `docs/architecture/ipc-contract.txt` avec `node scripts/check-architecture.mjs --write` |

## Avant d'ouvrir une Pull Request

```powershell
powershell -ExecutionPolicy Bypass -File scripts\validate.ps1
```

Voir [testing.md](testing.md) et [CONTRIBUTING.md](../../CONTRIBUTING.md). Aucune GitHub Action : tout se valide en local.
