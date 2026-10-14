# Benchmark du pipeline audio de Crosstalk face aux bruits de CS2

**Date :** 14 octobre 2026. **Branche :** `feat/audio-pipeline-benchmark`, partie de `develop` (build 127).
**Source :** `CS2_AUDIO_REA_INVESTIGATION_REPORT.md` (analyse REA/Ghidra et décodage des assets audio de CS2, build 2000930).

> **À lire en premier.** Tous les résultats chiffrés de ce rapport viennent de **scènes synthétiques** :
> - une vraie voix enregistrée ;
> - le vrai VAD Silero de l'application ;
> - des **bruits de combat générés**, spatialisés selon le modèle du rapport REA.
>
> **Aucun résultat n'a été mesuré en vraie partie**, et **le taux d'erreur de Whisper (WER) n'a pas pu être mesuré ici** (voir §6). Les gains présentés sont des gains d'**indicateurs de substitution** : rapport signal/bruit de la voix envoyée à Whisper, détection et couverture par le VAD. Ce ne sont pas des gains de transcription validés.

---

## 1. Résumé

| Question | Réponse |
|---|---|
| Le rapport REA est-il exploitable ? | Oui, sur trois points confirmés par les assets :<br>• voix VOIP mono et non spatialisée ;<br>• voix coupée sous 207,6 Hz à 48 dB/oct ;<br>• voix ajoutée au bus jeu à la toute fin du mixage.<br>Ce sont les hypothèses du Focus voix (A-01), et elles justifient une coupure à 200 Hz. |
| Le banc compare-t-il A, B et C sur les mêmes enregistrements ? | Oui : `crosstalk-asr-bench --compare-fronts`, avec les mêmes graines aléatoires pour toutes les variantes. |
| Whisper reconnaît-il mieux avec B ou C ? | **Inconnu à ce stade.** Le banc le mesure, mais aucun modèle Whisper n'était utilisable dans l'environnement de développement. Il reste une commande à lancer sur un PC Windows (§9). |
| Ce qui est démontré sur scènes synthétiques | **B (Focus voix)** : +5 à +7 dB de RSB de la voix par rapport à A.<br>**B + coupure 200 Hz** : +7 à +9 dB.<br>**C (RNNoise)** : +4 à +8 dB, et meilleur que B quand ses propres tirs sont au centre.<br>Dans toutes les variantes : aucune voix manquée, faux positifs quasi nuls, coût négligeable. |
| Modification du pipeline de production | **Aucune.** C'est conforme à la consigne : rien ne change sans validation par le WER. |

---

## 2. Découvertes du rapport REA exploitées

| Découverte (statut REA) | Ce qu'elle implique pour Crosstalk | Exploitation |
|---|---|---|
| Le sous-mix `VOIP` est **mono**, sans spatialisation ni HRTF dans la chaîne `csgo_voip` (**CONFIRMÉ**, §8.2, §10). Répartition centrée sur L/R (**PROBABLE**). | La voix arrive identique à gauche et à droite. Les bruitages passent par Steam Audio HRTF et sont spatialisés. | C'est la base du **Focus voix** (déjà livré, désactivé par défaut). Les scènes du banc placent la voix au centre. |
| Passe-haut VOIP à **207,59 Hz, 48 dB/oct** (**CONFIRMÉ**, §8.3). | Sous ~200 Hz, il n'y a **que** du son de jeu : grondement des explosions, basses des tirs. Notre passe-haut est à 120 Hz, ordre 2 : on garde 120 à 200 Hz de bruit pur. | Variante **« coupure 200 Hz »** du banc (Butterworth d'ordre 4). Les scènes appliquent à la voix le passe-haut exact de CS2 (Butterworth d'ordre 8 à 207,59 Hz). |
| Voix et jeu sont additionnés à `CMD_SUBMIX_OUTPUTx2`. La voix contourne la réverbération, l'EQ casque et `FinalMixGlue` (**CONFIRMÉ**). | La voix n'est pas réverbérée et ne reçoit aucun effet spatial. Seul le jeu en reçoit. | Le masque gauche/droite du Focus voix vise justement ce qui n'est pas identique sur les deux canaux. |
| Compression VOIP 40:1 parallèle, gain de sortie +24 dB, relâchement de 9 s (**CONFIRMÉ** pour les paramètres ; effet **PROBABLE**). | Les voix arrivent à un niveau très régulier. | Les scènes normalisent la voix (crête à −6 dBFS). Le niveau de combat varie de −6 à 0 dBFS de crête pour couvrir les cas difficiles. |
| Windows ne reçoit **qu'un flux mixé**. La catégorie `GameChat` est neutralisée par SDL3 (**CONFIRMÉ**). | Pas de flux voix séparé, pas d'atténuation des communications par Windows. La seule source reste le process loopback de `cs2.exe`, déjà utilisé. | Aucun développement « capture séparée » : il serait voué à l'échec. |
| Les répliques radio du jeu (VO) sont de la parole qui passe par le bus jeu (**CONFIRMÉ** pour le routage). | Risque de faux positifs que le VAD ne peut pas écarter. | Non mesurable ici, faute d'enregistrements de VO. Il faut un test en vraie partie (§8). |
| `VOIPVolume` et `MasterVolume` sont deux gains indépendants (topologie **CONFIRMÉE** ; correspondance avec les curseurs **NON VÉRIFIÉE**, test T2). | Volume du jeu à 0 et voix à 100 donneraient un flux voix seule. | Pas une solution de jeu, puisque le joueur n'entend plus le jeu. C'est en revanche **la bonne méthode pour enregistrer un corpus de vraies voix CS2**, avec des participants consentants (§8). |

Non exploité, car non pertinent ou hors périmètre :
- les démos (différées) ;
- la séparation de sources par réseau entraîné (pas de données) ;
- toute lecture du processus du jeu (interdite).

---

## 3. Existant vérifié (pour éviter les doublons)

Pipeline de production actuel, inchangé :

```
cs2.exe ─ process loopback WASAPI (float 48 kHz stéréo)
  └─ SpeechSink : moyenne L/R → mono   [ou Focus voix si activé : speech/focus.rs]
      └─ Conditioner : rééchantillonnage 16 kHz + passe-haut 120 Hz (ordre 2)
          └─ VAD Silero (pas de 6 trames) → Segmenter (garde anti-transitoires, 5 s max)
              └─ file Whisper (capacité 3, abandon après 6 s : Overflow / Stale)
                  └─ Whisper + filtre anti-hallucinations → traduction
```

| Élément déjà présent | Conséquence |
|---|---|
| Focus voix (PR #56) : masque fréquentiel gauche/droite, désactivé par défaut | Pas de nouveau développement « isolation par le centre ». Il est évalué comme variante B. |
| `asr_bench` : corpus, WER/CER, `--scenario combat` (bruit **mono**), `--noise-only`, `--vad-only`, `--live` | Le banc est **étendu** et non recréé. Le bruit mono existant ne permettait pas d'évaluer un traitement stéréo. |
| `fetch-asr-corpus.py` et `crosstalk-asr-bench.exe` dans le paquet portable Windows | La mesure du WER peut se faire sur le PC de l'utilisateur sans rien installer de plus. |
| Garde anti-transitoires, filtre anti-hallucinations, statistiques de rejet dans l'application | Les rejets `Transient`, `Short`, etc. sont comptés dans le banc comme dans l'application. |

---

## 4. Changements effectués

Seuls des fichiers de banc d'essai ont été modifiés. **Aucun changement dans le pipeline de production.**

| Fichier | Changement |
|---|---|
| `src-tauri/src/speech/bench.rs` | • `combat_scene` : scène stéréo 48 kHz. La voix est rééchantillonnée, passe par le passe-haut VOIP de CS2 et est placée au centre. Le combat vient de trois directions, avec des différences de niveau et de délai entre les oreilles, plus une source centrale optionnelle pour ses propres tirs. 3 s de combat seul avant et après la phrase.<br>• `Front` : plain, focus, band, focus+band.<br>• `apply_front`, avec compensation du délai du Focus.<br>• `score_scene` : voix manquée, couverture, faux positifs, texte.<br>• `voice_snr_db` : la référence reçoit la partie linéaire du traitement, pour ne compter que le bruit et les dégâts non linéaires.<br>• `cs2_voip_highpass`, `voice_band`, `resample_linear`. |
| `src-tauri/src/speech/dsp.rs` | `HighPass::with_q`, pour les cascades Butterworth. Comportement de `HighPass::new` inchangé. |
| `src-tauri/examples/asr_bench.rs` | Mode `--compare-fronts`, avec ou sans Whisper (`--vad-only`) et `--repeat N`. Variante C : RNNoise (`nnnoiseless` 0.5, BSD-3, poids intégrés), **dépendance de développement uniquement**, absente de l'application. |
| `src-tauri/examples/focus_bench.rs` | Supprimé : il fait doublon avec `--compare-fronts`. |
| `docs/testing/speech-recognition.md`, `docs/testing/voice-focus.md` | Commande de comparaison (étape 1.5) et renvoi vers ce rapport. |

Tests ajoutés (`speech::bench::tests`) :
- la scène place une voix mono au centre et un combat spatialisé dans les pauses ;
- le passe-haut VOIP de CS2 laisse moins de −45 dB à 100 Hz ;
- la coupure 200 Hz retire plus de 40 dB à 60 Hz et conserve 800 Hz ;
- le score compte voix manquées, couverture et faux positifs.

---

## 5. Méthodologie

- **Voix** : `jfk.wav` (anglais, 11 s, enregistrement public), seule voix réelle disponible dans l'environnement. Elle passe par le passe-haut VOIP de CS2, puis est placée au centre à −6 dBFS de crête.
- **Bruit** : générateur de combat du banc (tirs, rafales, explosions, fond), **synthétique**.
  - Trois sources spatialisées : gauche (gains 0,9 / 0,35, délai 0,6 ms), droite (0,3 / 0,85, 0,5 ms), presque devant (0,7 / 0,6, 0,1 ms).
  - Scène « + ses propres tirs » : une quatrième source exactement au centre, le pire cas pour le Focus voix.
- **Mêmes enregistrements pour toutes les variantes** : graines identiques, 12 tirages par configuration, 3 niveaux de combat (crête à −6, −3 et 0 dBFS).
- **Pipeline après le traitement** : exactement celui de l'application (`run_pipeline` : Conditioner, Silero, Segmenter, et Whisper quand il est disponible).
- **Métriques** :
  - **voix manquées** : aucun segment sur la phrase ;
  - **couverture** : part de la phrase incluse dans des segments. Les pauses naturelles de la phrase en sont exclues, d'où environ 87 % au maximum ;
  - **faux positifs** : segments pendant le combat seul ;
  - **RSB voix** : sur la phrase, par rapport à la voix propre ;
  - **coût** : millisecondes de calcul du traitement par seconde d'audio, et CPU de tout le pipeline.
- **Machine** : conteneur Linux x86-64 de développement (pas le PC de l'utilisateur). Les coûts absolus sont indicatifs.

---

## 6. Problèmes rencontrés

1. **Whisper indisponible, donc WER non mesuré.**
   - Les modèles officiels sont hébergés sur Hugging Face, bloqué par le proxy réseau de l'environnement (403).
   - Des copies des poids officiels ont été trouvées dans des paquets npm tiers, avec des tailles identiques aux fichiers officiels. Leur téléchargement a été **refusé par la politique de sécurité de l'environnement** (code tiers non vérifié), et je ne l'ai pas contourné.
   - Conséquence : **la mesure demandée, « Whisper reconnaît-il mieux ? », reste à exécuter**. Elle prend une commande sur Windows, où l'application installe les modèles (§9).
2. **Indicateur biaisé, corrigé.** Au premier passage, la coupure à 200 Hz semblait détruire la voix (RSB d'environ 1 dB). Deux raisons :
   - la voix de test contenait encore ses basses, alors que celle de CS2 est coupée à 207 Hz ;
   - le RSB comptait le déphasage du filtre comme du bruit.

   Après correction (voix passée par le passe-haut de CS2, référence filtrée de la même façon), la coupure apporte +1 dB seule et +2 dB après le Focus.
3. **DeepFilterNet non évalué.** Ses poids se téléchargent depuis GitHub (bloqué ici) et il exige un moteur ONNX lourd. RNNoise, disponible en Rust pur avec ses poids intégrés, a servi de variante C.
4. **Le VAD ne discrimine pas sur ce bruit.** Silero détecte 12 voix sur 12 sans faux positif dans **toutes** les variantes. Le bruit synthétique ne le trompe pas, ce qui ne prouve rien pour les vrais sons de CS2, en particulier la parole des répliques radio.

---

## 7. Résultats (scènes synthétiques, VAD réel, sans Whisper)

Colonnes : voix manquées (sur 12) · couverture · faux positifs · **RSB de la voix envoyée à Whisper** · coût du traitement (ms de calcul par seconde d'audio) · CPU de tout le pipeline sans Whisper.

### Combat à crête −3 dBFS (cas intermédiaire)

| Scène | Traitement | Manquées | Couverture | Faux positifs | RSB voix | Coût | CPU |
|---|---|---|---|---|---|---|---|
| combat autour | **A · actuel** | 0/12 | 86 % | 0 | **7,0 dB** | 0,13 ms/s | 3 % |
| combat autour | B · Focus voix | 0/12 | 87 % | 0 | 13,6 dB | 5,4 ms/s | 4 % |
| combat autour | coupure 200 Hz | 0/12 | 85 % | 0 | 8,2 dB | 0,5 ms/s | 4 % |
| combat autour | **B + coupure 200 Hz** | 0/12 | 87 % | 0 | **15,6 dB** | 5,5 ms/s | 4 % |
| combat autour | C · RNNoise | 0/12 | 85 % | 0 | 12,6 dB | 16,6 ms/s | 5 % |
| combat autour | B + RNNoise | 0/12 | 84 % | 0 | 12,8 dB | 21,2 ms/s | 6 % |
| + ses propres tirs | **A · actuel** | 0/12 | 87 % | 0 | **5,7 dB** | 0,13 ms/s | 4 % |
| + ses propres tirs | B · Focus voix | 0/12 | 88 % | 0 | 10,9 dB | 5,4 ms/s | 4 % |
| + ses propres tirs | coupure 200 Hz | 0/12 | 86 % | 0 | 6,8 dB | 0,5 ms/s | 4 % |
| + ses propres tirs | **B + coupure 200 Hz** | 0/12 | 87 % | 1 | **12,5 dB** | 5,6 ms/s | 4 % |
| + ses propres tirs | C · RNNoise | 0/12 | 86 % | 0 | 11,7 dB | 16,6 ms/s | 5 % |
| + ses propres tirs | B + RNNoise | 0/12 | 85 % | 0 | 12,0 dB | 21,9 ms/s | 6 % |

### RSB de la voix selon l'intensité du combat (toutes les variantes : 0 voix manquée sur 12)

| Scène | Combat (crête) | A | B | coupure 200 Hz | B + coupure | C RNNoise | B + RNNoise |
|---|---|---|---|---|---|---|---|
| combat autour | −6 dBFS | 10,0 | 16,1 | 11,2 | **18,0** | 13,7 | 14,1 |
| combat autour | −3 dBFS | 7,0 | 13,6 | 8,2 | **15,6** | 12,6 | 12,8 |
| combat autour | 0 dBFS | 4,0 | 10,9 | 5,2 | **13,2** | 11,2 | 11,8 |
| + ses propres tirs | −6 dBFS | 8,7 | 13,7 | 9,8 | **15,2** | 12,9 | 13,1 |
| + ses propres tirs | −3 dBFS | 5,7 | 10,9 | 6,8 | **12,5** | 11,7 | 12,0 |
| + ses propres tirs | 0 dBFS | 2,7 | 8,0 | 3,8 | 9,7 | **10,4** | **10,9** |

Faux positifs : 0 partout, sauf 1 segment pour « B + coupure » dans la scène « + ses propres tirs » à −3 et 0 dBFS. Rejets avant Whisper : 0 à 4 segments `Short` ou `Transient` par configuration, soit des bouts de tir écartés par les garde-fous existants.

Latence ajoutée par le traitement :
- Focus voix : 10,7 ms (une demi-trame de FFT) ;
- coupure 200 Hz : 0 ms (filtre IIR) ;
- RNNoise : 10 ms (une trame).

C'est négligeable devant la cible de 2 s.

---

## 8. Améliorations démontrées, et limites

**Démontré (sur scènes synthétiques uniquement) :**
1. **B (Focus voix) améliore toujours le RSB de la voix envoyée à Whisper** : +5 à +7 dB par rapport à A, sans voix manquée ni faux positif en plus, pour environ 0,5 % d'un cœur.
2. **La coupure à 200 Hz, tirée du rapport REA, ajoute environ +2 dB après le Focus voix**, pour un coût quasi nul (0,5 ms/s). C'est la meilleure variante dans 5 configurations sur 6.
3. **RNNoise (C) est le seul traitement qui réduit les sons centrés** (ses propres tirs). C'est la meilleure variante à 0 dBFS dans la scène « + ses propres tirs ». Il coûte environ trois fois plus que le Focus voix, soit environ 2 % d'un cœur, ce qui reste acceptable. Combiné au Focus, il n'ajoute presque rien.

**Non démontré, et ce qui décidera :**
- **le WER**. Les débruiteurs neuronaux produisent souvent des artefacts que Whisper tolère mal, même quand le RSB s'améliore. Un RSB meilleur ne garantit donc pas une meilleure transcription, et c'est particulièrement vrai pour C ;
- **les vrais sons de CS2** : HRTF réel, réverbération des cartes, répliques radio parlées, voix en plusieurs langues ;
- **l'hypothèse « voix au centre »** : les assets la rendent probable. Elle est mesurable avec le test T3 du rapport REA.

### À tester en vraie partie

| # | Test | Pourquoi |
|---|---|---|
| R1 | **WER A/B/C sur Windows** avec les modèles installés : la commande du §9, sur le corpus public puis sur des enregistrements CS2. | C'est la mesure demandée. Le banc est prêt. |
| R2 | Partie privée, Focus voix activé puis désactivé : nombre de lignes transcrites pendant les combats, et justesse. | Confirmer en conditions réelles. |
| R3 | Test **T3** du rapport REA (balayage sinusoïdal ou bruit rose envoyé par micro virtuel) : vérifier L − R ≈ −∞ dB pendant la voix, et la coupure à 207 Hz. | Valide la base du Focus voix et de la coupure. |
| R4 | Test **T2** du rapport REA (volume du jeu à 0, voix à 100) : si le flux ne contient plus que la voix, enregistrer **avec le consentement des participants** un petit corpus de vraies voix CS2, puis un corpus « jeu seul » (T1). | Remplace le bruit synthétique par du vrai, pour mesurer le WER sur de vrais mélanges (`--manifest` avec ces WAV). |
| R5 | 2 min de partie **sans** voix, avec les répliques radio du jeu (« Fire in the hole »…) : compter les faux sous-titres. | Seul cas où la parole du jeu peut tromper le VAD. Non mesurable ici. |
| R6 | `crosstalk-asr-bench --live 120 --model <modèle>` pendant une partie animée : compter les rejets `Overflow` et `Stale`. | Mesure les segments perdus quand Whisper n'arrive pas à suivre (file de 3 segments, abandon après 6 s). |

---

## 9. Commande à lancer sur Windows (mesure du WER)

Le paquet portable (`crosstalk-windows-x64-portable.zip`, publié avec chaque build) contient `crosstalk-asr-bench.exe` et `fetch-asr-corpus.py`. Les modèles sont ceux que Crosstalk a déjà installés.

```powershell
python fetch-asr-corpus.py --out corpus --per-lang 15
.\crosstalk-asr-bench.exe --manifest corpus\manifest.tsv --compare-fronts plain,focus,focus+band,rnnoise,focus+rnnoise --model base,small --repeat 2 --markdown fronts.md
```

Le tableau produit (`fronts.md`) donne, pour chaque modèle, scène et traitement :
- WER et CER, en russe et en anglais ;
- voix manquées, couverture, faux sous-titres ;
- rejets, temps Whisper p50/p95, coût du traitement, CPU.

Il est à coller au §7 de ce rapport.

---

## 10. Prochaines optimisations recommandées

Par ordre de priorité, et **chacune conditionnée par le WER** (R1/R2) :

1. **Si B l'emporte sur A au WER**, activer le Focus voix par défaut. Il est déjà en production derrière un interrupteur.
2. **Si B + coupure l'emporte sur B**, ajouter la coupure à 200 Hz (ordre 4) à la sortie du Focus voix. Cela fait environ 10 lignes, réutilise `voice_band` et coûte 0,5 ms/s. Ne **pas** changer le passe-haut global de 120 Hz sans mesure : il sert aussi au micro (dictée), où la voix n'est pas passée par CS2.
3. **Si C améliore le WER dans le cas « ses propres tirs »** sans le dégrader ailleurs, proposer RNNoise en option, en pur Rust avec ses poids intégrés (environ 2 % d'un cœur). DeepFilterNet ne serait à évaluer que si RNNoise montre un gain réel mais insuffisant, car il impose un moteur ONNX et des poids à télécharger.
4. **VAD** : aucun problème mesuré sur le bruit synthétique. Le risque réel est la parole des répliques radio (R5). Piste issue du rapport REA, au stade d'**hypothèse** : un classifieur de segment fondé sur la « signature VOIP », c'est-à-dire mono strict (L = R) et absence d'énergie sous 200 Hz, deux propriétés que les répliques du jeu n'ont pas.
5. **Segments perdus (`Overflow`/`Stale`)** : les mesurer d'abord (R6). S'ils sont fréquents :
   - choisir le modèle selon le RTF mesuré ;
   - utiliser la carte graphique (mode Auto déjà en place) ;
   - réduire le coût des transcriptions en direct. Elles ne partent déjà que si Whisper est libre, et à un rythme réglé sur sa vitesse (`request_partial`, `PARTIAL_MAX_RTF`). Le levier restant est le délai qu'une transcription en direct en cours impose à la phrase finale qui arrive juste après : la mesurer, et l'annuler si elle dépasse le budget.
6. **Corpus réel** (R4) : c'est la base de toutes les décisions suivantes, y compris d'un éventuel Whisper affiné pour CS2 (A-10).

---

## 11. Validation locale

| Vérification | Résultat |
|---|---|
| `cargo test --lib --features whisper` | 227 réussis, 0 échec |
| `cargo clippy --lib --tests --examples --features whisper -D warnings` | OK |
| `cargo fmt --check` | OK |
| `npm run check:anticheat` | OK : aucune API d'accès aux processus |
| `asr_bench --compare-fronts … --vad-only --repeat 12` (−6, −3 et 0 dBFS) | Tableaux du §7 |
| Compilation croisée Windows (`crosstalk-asr-bench.exe` inclus) | Voir la PR |

Rien n'a été exécuté sur GitHub Actions. La branche n'est pas fusionnée.
