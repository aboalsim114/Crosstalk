# Protocole de test : carte graphique (sans objet)

Depuis [D-14](../architecture/decisions.md), la reconnaissance vocale (Parakeet TDT 0.6B v3 via sherpa-onnx / ONNX Runtime) tourne **uniquement sur le processeur**, avec la moitié des processeurs logiques (1 à 4 fils). Il n'y a plus de choix AUTO / GPU / CPU, plus de Vulkan ni de test de la carte graphique, et `crosstalk-asr-bench` n'a plus d'option `--gpu-info` ou `--accel`.

Ce protocole ne s'applique donc plus. Un ancien réglage `accel` est ignoré au chargement. Pour la reconnaissance, voir [speech-recognition.md](speech-recognition.md).
