# Audit initial — mission de stabilisation

Date : 14 octobre 2026. L'audit d'architecture du code est dans [`docs/architecture/audit.md`](../architecture/audit.md) (constats A1 à A21, défauts F1 à F14, corrigés par #66 à #80). Celui-ci porte sur **l'état réel du produit livré**, observé à l'exécution.

## État de départ

- **Dépôt** :
  - `main` @ `97e6682` : v0.3.0, plus les correctifs Windows de `validate.ps1` (#83) ;
  - `develop` @ `7edd9a7` : en retard sur `main` des commits de release et de #83 ;
  - branche de travail créée depuis `develop`, avec `main` fusionné pour disposer de ces correctifs. Son contenu est identique à `97e6682`.
- **Copie de travail** : propre ; aucune modification locale à préserver.
- **PR ouvertes** : #57 (en attente de décision) et #60 (brouillon, TranslateGemma, absent de `develop`). TranslateGemma n'est donc pas testé ici : il n'est pas dans la version examinée.
- **Outils** :
  - Rust 1.99 MSVC, Node 24.19, VS Build Tools 2019 (CMake 3.20, dumpbin) ;
  - libclang (installation locale d'une session précédente) ;
  - REA 6.4.0 avec Ghidra 12.1.4 et JDK 21 ;
  - **pas** de SDK Vulkan, **pas** de VM ni de Windows Sandbox (Windows Famille).
- **Modèles installés** (`%LOCALAPPDATA%\Crosstalk\models`) : Whisper base, small et large-v3-turbo, VAD Silero, Opus-MT en↔fr, ru↔fr, uk↔fr et tr→fr.
- **Environnement vivant** : Crosstalk 0.2.0 (build MinGW) de l'utilisateur en cours d'exécution, et CS2 ouvert. Ni l'un ni l'autre n'a été modifié ou arrêté.

## Référence (baseline)

`dist-local/97e6682/validation.md`, exécuté sous Windows avant la mission : 14 étapes OK.
- npm ci, anti-triche, architecture, typecheck, Vitest, build UI ;
- rustfmt, clippy, tests Rust, installateur ;
- NSIS + MSI MSVC, outils.

Les binaires de ce dossier servent de référence « avant » aux analyses REA et aux bancs.

## Risques identifiés au départ

| Risque | Observation | Suite |
|---|---|---|
| Dépendances natives non livrées | `MSVCP140.dll` importée par le binaire officiel | CT-007 (corrigé) |
| Promesse GPU non tenue par la version officielle | build MSVC sans `gpu` | CT-002 (ouvert) |
| Qualité de transcription en conditions réelles | boucles de Whisper, annonces perdues | CT-001, CT-008 (corrigés) |
| Exécutables non signés | `NotSigned` | Action humaine : certificat de signature de code |
| Licences | liste incomplète ; 2 modèles en CC-BY-4.0 (attribution) | CT-006 (corrigé) ; revue juridique à faire |
| Couverture de test réelle | aucune partie de CS2, aucun PC vierge, aucune voix russe | voir test-matrix.md |
