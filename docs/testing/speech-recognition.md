# Protocole de test — reconnaissance vocale (feat/speech-recognition)

À exécuter sur un PC **Windows 10/11 x64** avec Counter-Strike 2. Moteur : **Parakeet TDT 0.6B v3** (int8, ONNX) exécuté par **sherpa-onnx** / ONNX Runtime sur le processeur, VAD **Silero** (ONNX) ([D-14](../architecture/decisions.md)). `scripts\validate.ps1` (en local) vérifie la logique (tests unitaires) et la compilation Windows avec les bibliothèques précompilées de sherpa-onnx. Les mesures (corpus public, CS2, latence, CPU) sont manuelles et décrites ci-dessous.

> **Pas encore testé en jeu** : la reconnaissance avec CS2 lancé n'a pas été essayée avec ce moteur (ni latence réelle, ni CPU, ni FPS). Les seuls chiffres disponibles viennent d'un banc synthétique ([rapport](../reports/asr-engine-replacement.md)).

> Rien n'est enregistré ni envoyé : l'audio reste en mémoire, la transcription est locale (processeur seul, sans carte graphique). N'ajoutez pas vos enregistrements de partie au dépôt : les voix et les pseudos sont des données personnelles.

## 0. Préparation

1. Sur le commit à tester, construisez le paquet : `powershell -ExecutionPolicy Bypass -File scripts\validate.ps1`. Le dossier `dist-local\<commit>\` contient l'installateur, `crosstalk-audio-probe.exe`, `crosstalk-asr-bench.exe`, `download-models.ps1` et `fetch-asr-corpus.py`.
2. Installez Crosstalk (`*-setup.exe`).
3. Installez les modèles (VAD `silero_vad.onnx` + dossier `parakeet-tdt-0.6b-v3-int8`, ~671 Mo) dans `%LOCALAPPDATA%\Crosstalk\models` (l'installateur le fait déjà ; sinon) :
   ```powershell
   powershell -ExecutionPolicy Bypass -File .\download-models.ps1
   ```
   Le script vérifie la taille exacte et le SHA-256 de chaque fichier, et reprend un téléchargement interrompu. Les DLL `sherpa-onnx-c-api.dll`, `onnxruntime.dll` et `onnxruntime_providers_shared.dll` doivent être à côté de `crosstalk.exe` et de `crosstalk-asr-bench.exe`.
4. Notez votre processeur (Gestionnaire des tâches → Performances) et votre version de Windows (`winver`).

> **Sans PC Windows de build** : `scripts/cross-build-windows-gnu.sh` produit sous Linux un paquet portable de test compilé avec MinGW/gcc (`dist-test/crosstalk-<commit>-windows-x64-gcc-test.zip`, notice `LISEZMOI-TEST.txt` incluse). La version officielle reste le build MSVC de `validate.ps1`. Pour comparer les deux compilateurs, lancez la même commande `crosstalk-asr-bench` avec les deux exécutables, sur le même WAV, CS2 fermé.

## 1. Banc d'essai hors ligne (sans CS2)

Dans un terminal ouvert dans `dist-local\<commit>\` :

| # | Commande | Résultat attendu |
| --- | --- | --- |
| 1.1 | `.\crosstalk-asr-bench.exe --vad-only --noise-only 120` | « 0 segments envoyés à la reconnaissance » ou très peu : la VAD et les garde-fous écartent les tirs et explosions **synthétiques**. |
| 1.2 | `.\crosstalk-asr-bench.exe --noise-only 120` | « **0 fausses transcriptions affichées** ». Sinon, notez les phrases inventées. |
| 1.3 | Corpus de référence : `python fetch-asr-corpus.py --out corpus --per-lang 15` (inclus dans le paquet ; Python 3 requis), puis `.\crosstalk-asr-bench.exe --manifest corpus\manifest.tsv --markdown bench.md` | Un tableau avec WER/CER russe et anglais, latence et CPU, propre (`clean`) et sous bruit de combat synthétique (`combat`). Collez `bench.md` dans la PR. |
| 1.4 | **Vos enregistrements CS2** (voir ci-dessous) : `.\crosstalk-asr-bench.exe --manifest mes-wav.tsv --scenario clean --verbose` | Pour chaque fichier : les phrases transcrites et les segments rejetés (`Transient` = tir ou explosion, `Hallucination`, …). Vérifiez à l'oreille si le texte correspond à ce qui est dit. |

**Format de `mes-wav.tsv`** (un fichier par ligne, séparateur tabulation ; la référence est facultative) :

```
partie1.wav	ru	Двое идут на B
partie2.wav	en
```

Les WAV enregistrés avec `crosstalk-audio-probe.exe --wav` conviennent directement. Avec une référence, le banc calcule le WER ; sans référence, il affiche seulement la transcription.

## 2. Flux réel (CS2 lancé)

| # | Action | Résultat attendu |
| --- | --- | --- |
| 2.1 | `.\crosstalk-asr-bench.exe --live 120`, en partie avec du chat vocal (ou `voice_loopback 1`, cf. protocole audio §3) | Une ligne `[RU] « … »` ou `[EN] « … »` par phrase, avec sa latence. À la fin : segments, rejets, latence p50/p95 et **CPU moyen**. Collez ce bilan. |
| 2.2 | Lancez Crosstalk. Écran **Audio & IA** | Carte « Modèle local » : état **Parakeet TDT 0.6B v3** (vert), « CPU · ONNX Runtime », temps de chargement (quelques secondes). Sans modèle : « Modèle absent », puis **Rechercher les modèles** une fois les fichiers installés. Si d'anciens fichiers Whisper (`ggml-*.bin`) sont présents, leur taille s'affiche avec **Libérer l'espace** ; rien n'est supprimé sans ce clic. |
| 2.3 | **Accueil → Démarrer**, partie en cours | En haut à droite, « ● PAROLE DÉTECTÉE » quand quelqu'un parle. Dans le pipeline : VAD, puis Transcription s'allument ; **Traduction reste éteinte (« à venir »)**. |
| 2.4 | Une phrase est dite | Elle apparaît dans le Script avec **VOIX · RU · TRANSCRIPTION** et le statut **TRANSCRIT**, dans sa langue d'origine, avec la note « traduction en français à venir ». Le sous-titre indique « VOIX · RUSSE · TRANSCRIPTION ». **Aucun texte français n'est inventé.** |
| 2.4 b | Une phrase de **plus de 2 s** | Pendant qu'elle est dite, une ligne **EN DIRECT** apparaît (texte en gris, en italique, terminé par « … ») et se complète environ chaque seconde. Moins d'une demi-seconde après la fin de la phrase, la même ligne devient **TRANSCRIT** (texte définitif), puis passe en français. Notez le délai entre le début de la phrase et la première ligne EN DIRECT. Si un bruit a été pris pour une voix, la ligne EN DIRECT disparaît. |
| 2.5 | Overlay en jeu | Le bloc indique « TRANSCRIPTION (NON TRADUITE) » ; en format Minimal, la ligne commence par `[RU]`. |
| 2.6 | Fusillade et grenades **sans** parole pendant 2 min | Aucune ligne TRANSCRIT n'apparaît, ou très peu. Le compteur sous le seuil VAD (« … rejetés (tir / explosion …) ») augmente. Notez toute fausse transcription. |
| 2.7 | Seuil VAD : 30 %, puis 80 % | À 30 %, plus de segments, donc plus de rejets ; à 80 %, des débuts de phrase peuvent manquer. Le réglage s'applique sans redémarrer. |
| 2.8 | Langues : décochez **Anglais** (Langues & glossaire), parlez anglais | Le modèle écrit ce qu'il entend ; la langue est lue dans le texte, parmi les langues autorisées. La ligne est rejetée (`alphabet incohérent`) ou attribuée à une autre langue autorisée à alphabet latin : c'est attendu. Le turc n'est pas reconnu par ce modèle. |
| 2.9 | **Ctrl+Alt+F10** pendant qu'une phrase est en cours | Capture arrêtée ; aucune transcription n'arrive après l'arrêt (la file est purgée). |
| 2.10 | Gestionnaire des tâches pendant une partie | Notez le CPU de `crosstalk.exe` et les FPS de CS2 avec et sans Crosstalk. |
| 2.11 | Écran **Diagnostic** | La ligne « Reconnaissance vocale » (« Parakeet TDT 0.6B v3 · chargé en … · CPU ») montre l'état réel du moteur. |

## 2 bis. Bilan de la chaîne dans le journal (issue #103)

Pendant une capture, Crosstalk écrit une ligne `pipeline` dans le journal toutes les 30 s (Diagnostic › Journal, ou `%LOCALAPPDATA%\Crosstalk\logs\crosstalk-<date>.log`). Cette ligne ne contient que des compteurs et des niveaux, jamais le texte ni le son :

```
30 s de capture : son moyen -34 dBFS, crête -8 dBFS · VAD : 6 segment(s), écartés avant la reconnaissance : 2 (tir/explosion 2) · Reconnaissance : 3 gardée(s), rejetées : 1 (hallucination 1), calcul 0,6× la durée de voix · traduction : 3 ok, 0 laissée(s) telle(s) quelle(s), 0 erreur(s)
```

Quand une étape ne produit plus rien, un avertissement `WARN pipeline:` la nomme, une seule fois par capture. La ligne **Voix → texte** de l'écran Diagnostic affiche le même message.

| Avertissement | Étape en cause |
| --- | --- |
| `aucun bloc audio reçu de CS2` | Capture : le flux de `cs2.exe` est interrompu. |
| `capture silencieuse` | Capture : CS2 n'envoie que du silence (mauvaise sortie, son coupé). |
| `du son arrive de CS2 … aucune parole` | VAD : du son arrive depuis 2 min sans aucun segment (seuil trop haut, ou voix absentes du son capturé). |
| `… tous écartés avant la reconnaissance` | Découpage : tirs, segments trop courts, file pleine. |
| `… envoyés à la reconnaissance, aucune transcription gardée` | Reconnaissance : chaque phrase est rejetée (raisons indiquées), ou le modèle ne rend rien. |
| `bloc(s) audio perdus` | La reconnaissance ne suit pas le flux (processeur trop chargé). |
| `traduction impossible : …` / `aucune ligne traduite` | Traduction : erreur du LLM. |

`calcul N× la durée de voix` : le temps de la reconnaissance rapporté à la durée des phrases transcrites. Au-dessus de 1×, elle prend du retard sur les voix.

Raisons de rejet possibles : `short`, `transient`, `stale` (« trop tard »), `overflow` (« file pleine »), `empty`, `hallucination`, `repetition`, `wrongScript` (« alphabet incohérent »).

Il n'y a **plus de bascule automatique** sur un modèle plus léger : Parakeet est le seul modèle. Si les segments expirent « trop tard » ou « file pleine » avec CS2 lancé, notez-le avec la ligne `pipeline` : c'est le signe que le processeur ne suit pas.

Joignez ces lignes au rapport (Diagnostic › Exporter).

## 3. Résultats à remonter

```
Windows : … (build …)      CPU : …      (reconnaissance sur processeur seul)
1.1 : … segments / 1.2 : … fausses transcriptions
1.3 : (coller bench.md)
1.4 : … fichiers, … phrases correctes / …, faux positifs : …
2.1 : segments …, transcrits …, latence p50 … / p95 …, CPU … %
2.2 – 2.11 : ✅ / ❌ + remarques
FPS CS2 avec / sans Crosstalk : … / …
```
