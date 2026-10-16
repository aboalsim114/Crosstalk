# Performances mesurées

Machine : Windows 11 Famille 26200, Intel Core i7-1165G7 (4 cœurs / 8 threads, sans GPU dédié), 15,8 Go. Toutes les mesures portent sur le processeur : la version MSVC n'a pas de Whisper GPU (CT-002).

> **Moteur remplacé ([D-14](../architecture/decisions.md))** : les mesures Whisper ci-dessous datent d'avant le passage à Parakeet TDT 0.6B v3 (sherpa-onnx, processeur seul). Elles n'ont pas été refaites sur cette machine avec le nouveau moteur. Voir [Parakeet : banc synthétique](#parakeet--banc-synthétique-d-14).

Sauf mention contraire, les mesures sont faites **machine au repos**. Plusieurs mesures prises pendant une analyse Ghidra ou une compilation ont été écartées comme non comparables. Elles restent signalées quand elles ont orienté une décision.

## Reconnaissance vocale — banc `crosstalk-asr-bench`

- **Corpus** : 8 phrases synthétiques (voix Windows Zira en anglais, Hortense en français, phrases inventées, 16 kHz). Il n'y a aucune voix russe : aucune voix russe n'est installée sur cette machine.
- **Commande** : `--model base,small --scenario clean,combat --lang en,fr`. 4 threads, fenêtre d'encodeur réduite (réglage par défaut de l'application).
- Le banc ne calcule de WER que pour l'anglais et le russe. Le français est transcrit mais pas noté.

| Modèle · scénario | Version | WER en | Whisper p50 / p95 | Latence estimée p50 / p95 | RTF | Rejets |
|---|---|---|---|---|---|---|
| base · propre | référence 97e6682 | 25,7 % | 494 / 944 ms | 1 102 / 1 552 ms | 0,27 | Repetition 1 |
| base · propre | après correctifs | **14,3 %** | 572 / 764 ms | 1 180 / 1 372 ms | 0,28 | aucun |
| base · combat | référence | 11,4 % | 530 / 1 055 ms | 1 138 / 1 663 ms | 0,24 | Repetition 1 |
| base · combat | après | 11,4 % | 670 / 1 605 ms | 1 278 / 2 213 ms | 0,32 | Repetition 1 (`fr8`, voir ci-dessous) |
| small · propre | référence | 14,3 % | 1 812 / 2 049 ms | 2 420 / 2 657 ms | 0,86 | aucun |
| small · propre | après | 14,3 % | 1 896 / 2 126 ms | 2 504 / 2 734 ms | 0,90 | aucun |
| small · combat | référence | 5,7 % | 2 095 / 2 858 ms | 2 703 / 3 466 ms | 0,94 | aucun |
| small · combat | après | 5,7 % | 2 064 / 2 370 ms | 2 672 / 2 978 ms | 0,88 | aucun |

Lecture :
- **Combat** : en base, `fr8` boucle sur « je retire, je retire… ». La seconde passe par échantillonnage (CT-001) le récupère une fois sur deux d'un essai à l'autre (elle est aléatoire par nature). Le p95 combat augmente d'autant.
- **Modèle small** : RTF de 0,86 à 0,94 sur ce processeur à 4 threads. C'est trop juste pendant une partie, ce qui confirme le choix de base par défaut.
- **Bruit seul** (`--noise-only 120`, tirs et explosions synthétiques) : 0 segment détecté et 0 fausse transcription, avant comme après.

## Parakeet : banc synthétique (D-14)

Mesures du [rapport de remplacement](../reports/asr-engine-replacement.md), **pas** sur la machine ci-dessus : processeur d'un conteneur Linux, 4 fils.

- **Corpus** : 517 extraits, voix de synthèse Piper (ru, en, fr), propres et sous bruit de combat synthétique ; 50 extraits de bruit seul.
- **WER** : Parakeet 46,6 % ; FastConformer multilingue 57,2 % ; whisper-base 60,7 %.
- **Bruit seul** : 1 fausse transcription sur 50 avec Parakeet, 50 sur 50 avec whisper-base.
- **Temps par extrait** : p50 ~480 ms. **Mémoire** : ~0,8 Go. **Chargement** : ~6 s.

À refaire sur l'i7-1165G7 de référence et avec CS2 lancé : ni la latence en jeu, ni le CPU en partie, ni les FPS n'ont été mesurés.

## Bout en bout dans l'application

- **Montage** : copie de test de l'application, source « Périphérique de sortie ». Les 10 fichiers du corpus sont joués sur la sortie audio, 4 s d'écart, puis on lit les compteurs de l'application (Audio & IA).
- **Mesure de latence** : celle de l'application, de la fin de la voix à la transcription. Pour une ligne traduite, l'affichage ajoute l'attente et le temps de traduction.

| Version | Transcrits | Rejetés | Latence voix p50 / p95 | CPU (% de la machine) |
|---|---|---|---|---|
| avant CT-008 (build du commit c02e61b) | 13 | 2 (répétition) | 2,4 s / 4,0 s | 10,9 % |
| après CT-007 + CT-008 (build du commit 74ace45) | 12 | **0** | **1,5 s / 2,3 s** | 9,4 % |

Les deux lignes ne sont pas strictement comparables :
- **« Avant »** : compteurs cumulés depuis le lancement, sur deux sessions (4 annonces pendant l'analyse Ghidra, puis 8 au repos). Les 2 rejets viennent de la session au repos (`en5` et `fr8` absents du fil).
- **« Après »** : application relancée, une seule session de 10 annonces, dont `en5` et `fr8` deux fois.

Dans une session de contrôle avant CT-008, avec diagnostics (`CROSSTALK_SPEECH_DIAGNOSTICS=1`), `en5` et `fr8` joués deux fois chacun : 3 annonces sur 4 rejetées `Repetition`. Après correction, 4 sur 4 sont transcrites.

Lignes traduites affichées (session machine au repos, avant CT-008) : EN→FR entre 1,80 et 2,0 s ; les annonces coupées en deux segments, entre 3,3 et 4,0 s.

**Essai écarté** : refaire le décodage d'une boucle sur la fenêtre complète de 30 s coûtait 5,2 s de Whisper (`full_ms` 5 162). L'annonce suivante devenait alors périmée (Stale).

## Traduction — `crosstalk-translate` (Opus-MT, beam search)

Mesures prises pendant l'analyse Ghidra (machine chargée) : elles sont à refaire au repos.

| Sens | Chargement du modèle | Une phrase (p50) |
|---|---|---|
| ru→fr | 0,7–0,9 s | 0,6–1,1 s |
| en→fr | 0,7–0,9 s | 0,6–1,4 s |
| fr→en / fr→ru / fr→uk (Répondre) | 1,3–2,7 s | 3,0–4,9 s (3 essais, machine très chargée) |
| ru→fr provisoire (`--greedy`) | — | 0,55 s |

Dans l'application (Langues › Répondre, machine au repos) : « Ils sont deux en bas avec un AWP, je tiens le smoke. » donne « They're two down there with a AWP, I'm holding the smoke. » en 3,6 s, chargement du modèle fr→en compris.

## Mémoire

| Mesure | Résultat |
|---|---|
| Application en session (Whisper base + ru→fr + en→fr chargés) | ensemble de travail ~ 900 Mo, mémoire privée réservée ~ 1,55 Go |
| Whisper base seul (banc) | pic privé 888 Mo, pic ensemble de travail 255 Mo |
| Whisper small seul (banc) | pic privé 1 311 Mo, pic ensemble de travail 632 Mo |
| Un modèle Opus-MT (outil `translate`) | pic privé 399 Mo, ensemble de travail 448 Mo |

L'écart entre mémoire réservée et ensemble de travail vient surtout des tampons de whisper.cpp, réservés mais peu touchés. Il n'y a aucun double chargement de modèle (vérifié dans le code : les moteurs de Répondre se chargent à la demande).

## Démarrage, arrêt, CPU au repos

| Mesure | Résultat |
|---|---|
| Lancement → interface (intro) affichée | 2,2 / 3,3 / 3,6 s (3 lancements ; temps de sondage CDP inclus) |
| Lancement → Whisper base prêt | 2,2 / 3,6 / 3,6 s |
| Arrêt d'urgence → capture arrêtée | 35 ms (statut `audio_status` sondé toutes les 20 ms) |
| Session en écoute, aucune voix (CS2 ouvert) | 1,3 % de la machine |

## Non mesuré

- **FPS de CS2 avec et sans Crosstalk** : aucune mesure dans une vraie partie. CS2 était ouvert, mais aucun outil de mesure d'images n'était disponible, et une partie demande un joueur.
- **VRAM, inférence GPU** : pas de build GPU (CT-002) ni de GPU dédié. Sans objet depuis D-14 : la reconnaissance tourne sur le processeur seul.
- **Parakeet sur cette machine** : ni banc, ni application, ni partie (voir plus haut).
- **Russe** : aucune voix russe sur la machine.
