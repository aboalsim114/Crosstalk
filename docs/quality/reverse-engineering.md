# Rétro-ingénierie des binaires Crosstalk

Mission de stabilisation, branche `fix/rea-full-stabilization`. Ce document distingue :
- **ce que REA a observé** (identifiants d'Evidence) ;
- **ce que d'autres outils ont observé** (`dumpbin`, protocole CDP, Win32) ;
- **les déductions et ce qui reste inconnu**.

## 1. Outils et état

| Outil | Version | État |
|---|---|---|
| REA (`rea-agents`) | 6.4.0 | CLI fonctionnelle. Le serveur MCP est enregistré dans `~/.claude.json` (`rea doctor` : sain), mais sa connexion a expiré au démarrage de la session (`CONNECT_TIMEOUT` après 30 s). Toutes les analyses ont donc été faites par la CLI. |
| Ghidra | 12.1.4 (JDK 21) | Moteur natif de REA, détecté sain par `rea doctor`. |
| Hopper, IDA | — | Absents (non nécessaires). |
| `dumpbin` | MSVC 14.29 (Build Tools 2019) | Imports et en-têtes PE. |
| WebView2 CDP | `--remote-debugging-port=9333` sur la copie de test | Inspection et pilotage de l'interface du vrai binaire. |

**Blocage documenté** : `rea analyze` sur `crosstalk.exe` (9,6 Mo, LTO, release) échoue avec `provider_timeout` sur `list_documents`. L'auto-analyse de Ghidra n'a pas abouti en 330 s, ni en 2 400 s avec `REA_GHIDRA_STARTUP_TIMEOUT_MS`. Le binaire principal n'a donc pas été désassemblé. L'analyse native a porté sur `crosstalk-audio-probe.exe` (255 Ko), qui contient le même code de capture WASAPI (`src-tauri/src/audio/wasapi.rs`). En CLI, chaque requête réimporte le binaire dans Ghidra (~7 min) : une session MCP connectée éviterait ce coût.

## 2. Binaires analysés

Build de référence : `dist-local/97e6682/` (MSVC, `scripts/validate.ps1 -Release`, commit `97e6682`).

| Fichier | SHA-256 | Analyse |
|---|---|---|
| `crosstalk.exe` | `930fe480…a3a` | REA `inspect-artifact` (ev_fa6bfa37…), `inspect-pe-resources` (ev_a87d7e08…), `dumpbin` |
| `Crosstalk_0.3.0_x64-setup.exe` | `634513ba…948` | REA `inspect-artifact` (ev_e9f8192f…) |
| `crosstalk-audio-probe.exe` | `adef8ca4…eae` | REA `analyze` via Ghidra (ev_02c0b28f…), `search` |
| Copie de test `crosstalk-qa.exe` (identifiant `app.crosstalk.qa`) | — | REA `inspect-web-page` sur ses 3 fenêtres WebView2 |

## 3. Observations

### 3.1 `crosstalk.exe` — en-têtes, ressources, dépendances

- **REA `inspect-pe-resources` (ev_a87d7e08…)** :
  - 9 ressources, couverture complète ;
  - un groupe d'icônes cohérent (16, 24, 32, 48, 64 px et 256 px PNG, tailles déclarées conformes) ;
  - une `VERSIONINFO` et un manifeste de 334 octets (Common-Controls 6.0).
- **Version** : 0.3.0, `CompanyName` = « crosstalk », aucun copyright.
- **Signature** : `Get-AuthenticodeSignature` = **NotSigned**. SmartScreen avertira l'utilisateur ; une signature de code demande un certificat (action humaine).
- **`dumpbin /headers`** :
  - sous-système GUI ;
  - ASLR haute entropie, Dynamic base, NX ;
  - `/GS` sur les objets C/C++.
- **`dumpbin /dependents`** : 33 DLL, dont **`MSVCP140.dll`** (bibliothèque C++ de whisper.cpp : flux, regex, threads). Cette DLL vient du redistribuable Visual C++, que Windows n'installe pas de lui-même et que l'installateur NSIS de Tauri ne fournit pas. Voir CT-007 dans [bugs.md](bugs.md).
- **Aucun import d'API d'accès à un autre processus**, ce qui est cohérent avec `npm run check:anticheat` : ni `OpenProcess`, ni `ReadProcessMemory`, ni `WriteProcessMemory`, ni `CreateRemoteThread`, ni `SetWindowsHookEx`.

### 3.2 `crosstalk-audio-probe.exe` — capture WASAPI

- **REA/Ghidra (ev_02c0b28f…)** : 822 procédures, 473 chaînes définies, 7 segments.
- **REA `search --kind procedures`** :
  - import `MMDEVAPI.DLL::ActivateAudioInterfaceAsync` (ev_54fdbf13…) : activation asynchrone de la capture par processus ;
  - `KERNEL32::CloseHandle` (ev_da54714e…) ;
  - `KERNEL32::WaitForSingleObject` et `WaitForSingleObjectEx` (ev_bbaef71a…).
- **Recherches sans résultat** :
  - `search` sur le message « activation de la capture » : 0 (ev_dbe792a6…). Les chaînes Rust ne sont pas terminées par un zéro, et Ghidra ne les définit pas comme chaînes.
  - `search` sur la procédure `open_process` : 0 (ev_6164e75a…). Le binaire release n'a pas de symboles et la fonction est probablement intégrée par LTO.
- **Inconnu** : le chemin « délai dépassé » de l'activation n'a pas été décompilé. Cela demandait `xrefs` sur l'import puis `function` sur l'appelant, soit au moins 15 minutes de réimport par requête en CLI. Sa correction (#73, F8) est vérifiée dans le source et compilée pour Windows, pas dans le binaire.

### 3.3 Interface (WebView2) — REA `inspect-web-page`

Copie de test lancée avec un identifiant et des dossiers isolés. L'installation personnelle n'a pas été touchée.

| Fenêtre | Evidence | Console |
|---|---|---|
| principale `http://tauri.localhost/` | ev_a24fee67… | 1 avertissement : `overlay_configure failed · overlay window missing` |
| overlay | ev_ce207b6d… | rien |
| réponse | ev_49633df0… | rien |

L'avertissement a été suivi jusqu'à sa cause : la fenêtre principale appelle `overlay_configure` avant que le cœur ait créé la fenêtre overlay. L'effet visible a été reproduit : le coin enregistré était ignoré au démarrage (CT-004, corrigé).

Une visite automatisée des 7 pages, avec captures et écoute de la console (`Runtime`, `Log`), n'a relevé aucune exception JavaScript ni aucune erreur de console. Elle a révélé deux défauts :
- la version « V0.2 » affichée en dur sur l'intro (CT-003) ;
- une liste de licences inexacte (CT-006).

## 4. Recoupement avec le source

| Observation binaire / exécution | Source | Conclusion |
|---|---|---|
| `MSVCP140.dll` importée | `whisper-rs-sys` compile whisper.cpp avec CMake ; la CRT est dynamique par défaut | Dépendance au redistribuable VC++ (CT-007) |
| Aucune API d'accès à un processus | `check-anticheat.mjs` ; `wasapi.rs` passe par `ActivateAudioInterfaceAsync` avec `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` | Architecture externe confirmée sur le binaire |
| `ActivateAudioInterfaceAsync` + `WaitForSingleObject` + `CloseHandle` | `wasapi.rs::open_process` | Imports cohérents avec le code corrigé en #73 ; chemin d'erreur non décompilé |
