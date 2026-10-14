# Registre des bugs — mission de stabilisation

Branche `fix/rea-full-stabilization` (depuis `develop` + `main` 97e6682).
Environnement : Windows 11 Famille 26200, Intel Core i7-1165G7 (4 c / 8 t), 15,8 Go, rustc 1.99.0 MSVC, Node 24.19.0.
Sévérités : P0 inutilisable · P1 fonction principale défaillante · P2 précision, latence ou comportement · P3 mineur.

| ID | Sév. | Résumé | Statut |
|---|---|---|---|
| CT-001 | P2 | Une boucle de Whisper sur un segment court fait rejeter toute l'annonce | Corrigé, vérifié au banc (f201140) |
| CT-002 | P2 | La version officielle MSVC (`validate.ps1`) est compilée sans la fonctionnalité `gpu` : pas de Whisper sur carte graphique, contrairement au README | Voir fiche |

## CT-001 — Une boucle de Whisper fait perdre l'annonce

- **Environnement** : `crosstalk-asr-bench.exe` du build de référence 97e6682 (MSVC, release), whisper base, CPU AVX2, 4 threads.
- **Reproduction** : corpus synthétique (voix SAPI Windows, phrases inventées), `--manifest manifest.tsv --model base --scenario clean --lang en,fr --verbose`.
- **Attendu** : « They are rotating mid, three of them. » transcrit.
- **Constaté** : le segmenteur coupe à la virgule. Le premier segment (1,66 s) est décodé en boucle, « they are rotating mid, » ×20, en 1 669 ms au lieu de ~700 ms. Le filtre le rejette (`Repetition`) : seule la moitié « 3 of them » s'affiche. Même chose en combat sur `fr8` (« je retire » ×30).
- **Cause** : décodage glouton sans repli de température (`temperature_inc = 0`, choisi pour la latence) et plafond fixe de 96 jetons, quelle que soit la durée du segment.
- **Correction** : `speech/engine.rs`. Une seconde passe à température 0,2, uniquement quand la première boucle (`filter::is_looping`) ; elle n'est gardée que si elle ne boucle plus. Le plafond de jetons suit la durée du segment (16 + 12/s, entre 24 et 96).
- **Tests** : `engine::tests::the_token_budget_follows_the_segment_length`, et `filter::tests::rejects_loops` (cas `is_looping`).
- **Validation** (même banc, même corpus, même machine) :

  | Mesure | Avant | Après |
  |---|---|---|
  | base · propre : WER en | 25,7 % | 14,3 % |
  | base · propre : rejets | Repetition 1 | aucun |
  | base · combat : rejets | Repetition 1 | aucun |
  | bruit seul 120 s : fausses transcriptions | 0 | 0 |

  Pour `fr8` en combat, la seconde passe rend « je tu presque mort, je retire, je retire » : un texte dégradé, mais affiché, là où rien ne l'était.
- **Statut** : corrigé, vérifié au banc. Non vérifié en partie réelle.

## CT-002 — Pas de Whisper sur carte graphique dans la version MSVC

- **Constaté** : `scripts/validate.ps1` construit l'application avec `npx tauri build --target x86_64-pc-windows-msvc`, sans `--features gpu`. Seule la version de test MinGW (`scripts/cross-build-windows-gnu.sh`) active `gpu`. Le README annonce pourtant « Whisper tourne sur la carte graphique (Vulkan) ». L'installateur officiel MSVC fait donc tout tourner sur le processeur.
- **Cause** : la version MinGW charge `vulkan-1.dll` à la demande (`scripts/cross/vulkan_shim.c`), ce qui lui permet de démarrer sur un PC sans pilote Vulkan. Ce mécanisme n'existe pas en MSVC. Activer `gpu` tel quel lierait `vulkan-1.dll` statiquement : l'application ne démarrerait plus sur un PC sans Vulkan.
- **Ce qui est fait** : le rapport `validation.md` indique désormais « Whisper GPU : absent de ce build MSVC », pour que personne ne prenne ce paquet pour une version avec GPU.
- **Ce qui reste à faire** : un build MSVC avec `gpu` et `/DELAYLOAD:vulkan-1.dll` (plus `delayimp.lib`), puis un test sur un PC avec et sans pilote Vulkan. Impossible ici : le SDK Vulkan (`glslc`) n'est pas installé sur cette machine.
- **Statut** : ouvert, documenté.
