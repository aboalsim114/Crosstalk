# Crosstalk — version de test (develop)

**[Télécharger l'installateur Crosstalk-setup.exe](https://github.com/aboalsim114/Crosstalk/raw/downloads/Crosstalk-setup.exe)** · outils de test : [crosstalk-windows-x64-portable.zip](https://github.com/aboalsim114/Crosstalk/raw/downloads/crosstalk-windows-x64-portable.zip)

| | |
|---|---|
| Build | 314 (mise à jour depuis l'application : `latest.json`) |
| Commit | [`19a7c17`](https://github.com/aboalsim114/Crosstalk/commit/19a7c174bf1eb123735e14492162309a24656ef2) |
| Construite le | 2026-10-17 19:38 UTC |
| SHA-256 de l'installateur | `597f230dc43a208056d9ef3d522cc704d9a3a383b1daa29ca3acef821cd77ad0` |
| SHA-256 des outils (zip) | `b28962eb1555119ca43b7a3c121bca018d79c3c79637615659b5d806b23fa3f2` |

Installateur Windows (x64), design « Crosstalk Installer » : installe Crosstalk (menu Démarrer, bureau) et télécharge le modèle de reconnaissance vocale pendant l'installation (Parakeet TDT 0.6B v3 et la détection de voix Silero : environ 671 Mo, une seule fois, puis tout fonctionne hors ligne). La reconnaissance tourne sur le processeur (sherpa-onnx, ONNX Runtime) ; la traduction passe par le LLM choisi au premier lancement. Compilé avec MinGW/gcc (`scripts/cross-build-windows-gnu.sh`). La version officielle reste celle compilée avec MSVC par `scripts\validate.ps1`. Non signée : SmartScreen affiche un avertissement (« Informations complémentaires » > « Exécuter quand même »).

Tout est local : aucune clé API, aucun abonnement, aucun envoi en ligne. Le zip contient les outils de test en ligne de commande (`LISEZMOI-TEST.txt`).
