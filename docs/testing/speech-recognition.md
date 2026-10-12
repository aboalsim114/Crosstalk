# Protocole de test — reconnaissance vocale (feat/speech-recognition)

À exécuter sur un PC **Windows 10/11 x64** avec Counter-Strike 2. La CI vérifie la logique (tests unitaires), la compilation Windows de whisper.cpp et mesure les modèles sur un corpus public (tâche `asr-bench`). En revanche, **elle ne peut ni tester CS2 ni mesurer votre machine** : ces étapes sont manuelles.

> Rien n'est enregistré ni envoyé : l'audio reste en mémoire, la transcription est locale (CPU). N'ajoutez pas vos enregistrements de partie au dépôt : les voix et les pseudos sont des données personnelles.

## 0. Préparation

1. Téléchargez l'artefact `crosstalk-windows-x64` de la PR (onglet **Checks** → **CI**). Il contient l'installateur, `crosstalk-audio-probe.exe`, `crosstalk-asr-bench.exe` et `download-models.ps1`.
2. Installez Crosstalk (`*-setup.exe`).
3. Installez les modèles (VAD Silero + Whisper `base`, ~150 Mo) dans `%LOCALAPPDATA%\Crosstalk\models` :
   ```powershell
   powershell -ExecutionPolicy Bypass -File .\download-models.ps1 -Models base,small
   ```
   Le script vérifie la taille exacte et le SHA-1 publié par whisper.cpp. Il affiche aussi le SHA-256 : copiez la ligne `ggml-silero-v5.1.2.bin` dans le compte rendu.
4. Notez votre processeur (Gestionnaire des tâches → Performances) et votre version de Windows (`winver`).

## 1. Banc d'essai hors ligne (sans CS2)

Dans un terminal ouvert dans le dossier de l'artefact :

| # | Commande | Résultat attendu |
| --- | --- | --- |
| 1.1 | `.\crosstalk-asr-bench.exe --vad-only --noise-only 120` | « 0 segments envoyés à Whisper » ou très peu : la VAD et les garde-fous écartent les tirs et explosions **synthétiques**. |
| 1.2 | `.\crosstalk-asr-bench.exe --noise-only 120 --model base` | « **0 fausses transcriptions affichées** ». Sinon, notez les phrases inventées. |
| 1.3 | Corpus de référence : `python fetch-asr-corpus.py --out corpus --per-lang 15` (inclus dans l’artefact ; Python 3 requis), puis `.\crosstalk-asr-bench.exe --manifest corpus\manifest.tsv --model base,small --markdown bench.md` | Un tableau avec WER/CER russe et anglais, latence et CPU, propre (`clean`) et sous bruit de combat synthétique (`combat`). Collez `bench.md` dans la PR. |
| 1.4 | **Vos enregistrements CS2** (voir ci-dessous) : `.\crosstalk-asr-bench.exe --manifest mes-wav.tsv --scenario clean --model base --verbose` | Pour chaque fichier : les phrases transcrites et les segments rejetés (`Transient` = tir ou explosion, `Hallucination`, …). Vérifiez à l'oreille si le texte correspond à ce qui est dit. |

**Format de `mes-wav.tsv`** (un fichier par ligne, séparateur tabulation ; la référence est facultative) :

```
partie1.wav	ru	Двое идут на B
partie2.wav	en
```

Les WAV enregistrés avec `crosstalk-audio-probe.exe --wav` conviennent directement. Avec une référence, le banc calcule le WER ; sans référence, il affiche seulement la transcription.

## 2. Flux réel (CS2 lancé)

| # | Action | Résultat attendu |
| --- | --- | --- |
| 2.1 | `.\crosstalk-asr-bench.exe --live 120 --model base`, en partie avec du chat vocal (ou `voice_loopback 1`, cf. protocole audio §3) | Une ligne `[RU] « … »` ou `[EN] « … »` par phrase, avec sa latence. À la fin : segments, rejets, latence p50/p95 et **CPU moyen**. Collez ce bilan. |
| 2.2 | Lancez Crosstalk. Écran **Audio & IA** | Carte « Modèle local » : état **Whisper base** (vert), « CPU · AVX2 · N threads · chargé en … ». Sans modèle : « Modèle absent », la case reste sélectionnable une fois le fichier installé puis **Rechercher les modèles**. |
| 2.3 | **Accueil → Démarrer**, partie en cours | En haut à droite, « ● PAROLE DÉTECTÉE » quand quelqu'un parle. Dans le pipeline : VAD, puis Transcription s'allument ; **Traduction reste éteinte (« à venir »)**. |
| 2.4 | Une phrase est dite | Elle apparaît dans le Script avec **VOIX · RU · TRANSCRIPTION** et le statut **TRANSCRIT**, dans sa langue d'origine, avec la note « traduction en français à venir ». Le sous-titre indique « VOIX · RUSSE · TRANSCRIPTION ». **Aucun texte français n'est inventé.** |
| 2.5 | Overlay en jeu | Le bloc indique « TRANSCRIPTION (NON TRADUITE) » ; en format Minimal, la ligne commence par `[RU]`. |
| 2.6 | Fusillade et grenades **sans** parole pendant 2 min | Aucune ligne TRANSCRIT n'apparaît, ou très peu. Le compteur sous le seuil VAD (« … rejetés (tir / explosion …) ») augmente. Notez toute fausse transcription. |
| 2.7 | Seuil VAD : 30 %, puis 80 % | À 30 %, plus de segments, donc plus de rejets ; à 80 %, des débuts de phrase peuvent manquer. Le réglage s'applique sans redémarrer. |
| 2.8 | Langues : décochez **Anglais** (Langues & glossaire), parlez anglais | Le texte est forcé en russe ou rejeté (`alphabet incohérent`) : c'est attendu. |
| 2.9 | **Ctrl+Alt+F10** pendant qu'une phrase est en cours | Capture arrêtée ; aucune transcription n'arrive après l'arrêt (la file est purgée). |
| 2.10 | Gestionnaire des tâches pendant une partie | Notez le CPU de `crosstalk.exe` et les FPS de CS2 avec et sans Crosstalk. |
| 2.11 | Écran **Diagnostic** | La ligne « Modèle whisper-… » montre l'état réel du moteur. |

## 3. Résultats à remonter

```
Windows : … (build …)      CPU : …      GPU : (non utilisé)
SHA-256 ggml-silero-v5.1.2.bin : …
1.1 : … segments / 1.2 : … fausses transcriptions
1.3 : (coller bench.md)
1.4 : … fichiers, … phrases correctes / …, faux positifs : …
2.1 : segments …, transcrits …, latence p50 … / p95 …, CPU … %
2.2 – 2.11 : ✅ / ❌ + remarques
FPS CS2 avec / sans Crosstalk : … / …
```
