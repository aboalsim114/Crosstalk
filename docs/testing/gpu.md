# Protocole de test : Whisper sur la carte graphique (Vulkan)

Sur Windows, avec une carte NVIDIA, AMD ou Intel et des pilotes à jour (ils installent `vulkan-1.dll`).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Audio & IA › Modèle local | Choix **AUTO / GPU / CPU**, AUTO par défaut. Au chargement, Crosstalk mesure la carte graphique et le processeur sur 3 s d'audio de test (le chargement prend donc quelques secondes de plus) : « Auto : carte graphique <nom>, N× plus rapide que le processeur » ou « Auto : processeur (carte graphique pas assez rapide…) ». |
| 2 | Diagnostic › journal | Lignes `asr: mode auto : GPU … ms, CPU … ms sur 3 s de test → GPU` et `asr: whisper-… : ready · GPU · <carte> (Vulkan) · auto : …`. |
| 3 | Partie avec des voix, modèle **base** puis **small** | Latence voix (Accueil) nettement plus basse qu'en CPU ; avec small, transcription russe plus juste. |
| 4 | Choisir **GPU** (forcé), puis **CPU** | GPU : la carte graphique est utilisée même si elle est plus lente. CPU :  le modèle se recharge en `CPU · AVX2`, la carte graphique n'est plus utilisée par Crosstalk. |
| 5 | FPS de CS2 avec GPU (overlay de Steam ou `cl_showfps 1`) | Baisse limitée (pics brefs pendant une transcription). Si gênant : choisir CPU. |
| 6 | PC sans pilote Vulkan (ou Vulkan < 1.2) | Crosstalk démarre normalement ; « Auto : processeur (aucune carte graphique Vulkan) ». |

Banc d'essai (paquet de test) : `crosstalk-asr-bench.exe --gpu-info` affiche la carte Vulkan utilisable (ou aucune) ; `crosstalk-asr-bench.exe --manifest corpus\manifest.tsv --model small --accel gpu` (ou `cpu`, `auto`) mesure le temps par segment.
