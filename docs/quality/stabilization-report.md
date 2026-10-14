# Rapport de stabilisation — Crosstalk 0.3.0

Branche `fix/rea-full-stabilization` (depuis `develop`, avec `main` @ 97e6682 fusionné). Machine : Windows 11 Famille 26200, Intel i7-1165G7, 15,8 Go, sans GPU dédié. Aucune GitHub Action.

Documents détaillés :
- [audit.md](audit.md) : état de départ et risques ;
- [reverse-engineering.md](reverse-engineering.md) : REA, Ghidra, dumpbin, CDP ;
- [bugs.md](bugs.md) : fiches CT-001 à CT-009 ;
- [test-matrix.md](test-matrix.md) : statut réel de chaque fonctionnalité ;
- [performance.md](performance.md) : mesures avant/après.

## A. Rétro-ingénierie

- **Utilisation de REA 6.4.0** : en CLI uniquement. Le serveur MCP, pourtant enregistré et jugé sain par `rea doctor`, n'a pas pu se connecter (délai de 30 s dépassé), d'où le passage par la CLI.
- **Opérations REA réellement exécutées** :
  - `inspect-artifact` sur `crosstalk.exe` et sur l'installateur ;
  - `inspect-pe-resources` ;
  - `analyze`, puis `search` (chaînes et procédures) via Ghidra 12.1.4 sur `crosstalk-audio-probe.exe` ;
  - `inspect-web-page` sur les 3 fenêtres WebView2 de l'application lancée.
- **Blocage** : l'analyse Ghidra de `crosstalk.exe` (9,6 Mo, LTO) n'aboutit pas en 40 min (`provider_timeout`). Le binaire principal n'a donc pas été décompilé.
- **Anomalies trouvées par les binaires et l'exécution, causes confirmées** :
  - `MSVCP140.dll` importée → CT-007 ;
  - avertissement console « overlay window missing » → CT-004 ;
  - version et licences inexactes affichées → CT-003, CT-006 ;
  - aucune API d'accès à un autre processus dans les imports, ce qui confirme l'architecture externe.

## B. Débogage

| | Nombre |
|---|---|
| Bugs détectés | 9 |
| Corrigés et vérifiés | 7 (CT-001, CT-003 à CT-008) |
| Ouverts, documentés | 2 (CT-002 : pas de GPU dans la version MSVC ; CT-009 : port du mode streamer non refermé) |
| Gravité | 1 P1 (CT-007), 3 P2, 5 P3 ; aucun P0 observé |

## C. Validation

- **`scripts/validate.ps1`** sur `b6f985b` : 12/12 étapes OK, NSIS + MSI MSVC compris (`dist-local/b6f985b/`).
- **Tests automatisés** :
  - Rust : 254 tests (application) et 29 (installateur) ;
  - Vitest : 173 tests ;
  - fmt et clippy `-D warnings` propres ;
  - anti-triche et règles d'architecture OK.
- **Exécutables réellement lancés** :
  - copie de test de l'application (build release MSVC, identifiant isolé), pilotée dans la vraie interface ;
  - `crosstalk-asr-bench` et `crosstalk-translate` (avant / après).
- **Vérifié en exécution** :
  - lancement et premier lancement ;
  - les 7 pages (0 erreur de console) ;
  - capture cs2.exe et capture de la sortie ;
  - Whisper et traduction de bout en bout ;
  - Répondre, mode streamer (SSE, filtrage du Host), arrêt d'urgence ;
  - overlay (coin enregistré), mises à jour, diagnostic.
- **Non testé, avec la raison** :
  - **partie réelle de CS2, « Qui parle ? », récapitulatif de fin de match** : il faut des joueurs ;
  - **voix russe** : aucune voix de synthèse russe installée ;
  - **GPU** : absent de la version MSVC, et pas de GPU dédié ;
  - **dictée** : micro et voix humaine ;
  - **installation et désinstallation réelles, PC vierge sans redistribuable** : aucune VM ni Windows Sandbox (Windows Famille), et pas question de toucher à l'installation personnelle ;
  - **zone de notification, démarrage automatique** : écriture dans le registre de l'utilisateur.

## D. Performances (au repos, processeur)

- **Dans l'application**, même scénario de 10 annonces :
  - annonces rejetées : 2 avant, **0 après** ;
  - latence voix p50 / p95 : **2,4 / 4,0 s avant, 1,5 / 2,3 s après** ;
  - CPU pendant la parole : environ 10 % de la machine ; 1,3 % en écoute sans voix.
- **Banc Whisper base** (propre) : WER en 25,7 % → **14,3 %** ; rejets « Repetition » 1 → 0. Bruit seul (120 s) : 0 fausse transcription avant comme après.
- **Mémoire** : environ 0,9 Go d'ensemble de travail en session (Whisper base et 2 modèles Opus-MT).
- **Temps** :
  - démarrage : 2,2 à 3,6 s jusqu'à l'interface et Whisper prêt ;
  - arrêt d'urgence : 35 ms.
- **Non mesuré** : FPS de CS2 et VRAM.

## E. Livraison

- **Commits** (branche `fix/rea-full-stabilization`) :

  | Commit | Changement |
  |---|---|
  | f201140 | CT-001 : boucle de Whisper |
  | b52e37f | CT-002 : rendu visible dans le rapport de validation |
  | 60991ef | CT-003 : version affichée |
  | e14f59d | CT-004 : coin de l'overlay |
  | c02e61b | CT-005 : faux « modèles absents » |
  | d4ec04a | CT-006 : licences |
  | 7f1ddbb | CT-008 : phrase répétée |
  | 74ace45 | CT-007 : CRT statique |
  | b6f985b et suivants | documentation |

- **Builds** :
  - `dist-local/b6f985b/` : `crosstalk.exe`, NSIS, MSI, outils, `SHA256SUMS.txt` ;
  - copie de test identique au code de `74ace45` : `src-tauri/target/x86_64-pc-windows-msvc/release/crosstalk.exe`.
- **Interventions humaines nécessaires** :
  1. Tester une partie réelle de CS2 avec cette version : latence, FPS, « Qui parle ? », chat.
  2. Installer sur un PC ou une VM vierge, pour confirmer CT-007 sans redistribuable VC++, puis mise à jour et désinstallation.
  3. Faire un build GPU MSVC (CT-002) : SDK Vulkan à installer, chargement différé de `vulkan-1.dll`.
  4. Signer le code : certificat requis.
  5. Revue juridique des licences, dont l'attribution CC-BY-4.0 de 2 modèles Opus-MT.
  6. Reconnecter le serveur MCP REA, pour permettre une analyse native plus poussée de `crosstalk.exe`.

Crosstalk n'est **pas** déclaré « sans bug ». Les fonctions testables sur cette machine fonctionnent, et les défauts trouvés sont corrigés ou documentés. La validation en partie réelle reste à faire.
