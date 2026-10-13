# Protocole de test : Whisper sur la carte graphique (Vulkan)

Sur Windows, avec une carte NVIDIA, AMD ou Intel et des pilotes à jour (ils installent `vulkan-1.dll`).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Audio & IA › Modèle local | Choix **CPU / GPU**, GPU par défaut. Sous les modèles : « Carte graphique : transcription bien plus rapide… ». Le moteur affiche `GPU · <nom de la carte> (Vulkan)`. |
| 2 | Diagnostic › journal | Ligne `asr: whisper-… : ready · GPU · <carte> (Vulkan)`. |
| 3 | Partie avec des voix, modèle **base** puis **small** | Latence voix (Accueil) nettement plus basse qu'en CPU ; avec small, transcription russe plus juste. |
| 4 | Choisir **CPU** | Le modèle se recharge : `CPU · AVX2`. La carte graphique n'est plus utilisée par Crosstalk. |
| 5 | FPS de CS2 avec GPU (overlay de Steam ou `cl_showfps 1`) | Baisse limitée (pics brefs pendant une transcription). Si gênant : choisir CPU. |
| 6 | PC sans pilote Vulkan (ou pilote trop ancien) | Crosstalk démarre normalement ; « Aucune carte graphique Vulkan utilisable : Whisper tourne sur le processeur ». |

Banc d'essai (paquet de test) : `crosstalk-asr-bench.exe --manifest corpus\manifest.tsv --model small --gpu` affiche `whisper small · GPU · <carte> (Vulkan)` et le temps par segment ; sans `--gpu`, la même mesure sur le CPU.
