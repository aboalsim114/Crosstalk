# Banc d'essai de la reconnaissance vocale

Scripts qui ont servi à choisir le moteur de reconnaissance vocale
(`docs/reports/asr-engine-replacement.md`). Tout tourne en local avec
sherpa-onnx (Python), sans clé ni compte. Rien d'enregistré ici : le corpus
est synthétique (voix Piper), plus les deux enregistrements de démonstration
fournis avec le modèle Parakeet (`test_wavs/en.wav`, `fr.wav`).

## Préparer

```bash
cd scripts/asr-bench
python3 -m venv venv && venv/bin/pip install sherpa-onnx==1.13.8 numpy soundfile psutil
```

Télécharger depuis les releases GitHub de k2-fsa/sherpa-onnx (`asr-models`
et `tts-models`) et extraire :

- dans `models/` : `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`,
  `sherpa-onnx-nemo-fast-conformer-transducer-be-de-en-es-fr-hr-it-pl-ru-uk-20k`,
  `sherpa-onnx-nemo-transducer-giga-am-v2-russian-2025-04-19`,
  `sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25`,
  `sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12`,
  `sherpa-onnx-whisper-base`, `sherpa-onnx-whisper-small` (référence
  seulement) et `silero_vad.onnx` ;
- dans `tts/` : les voix `vits-piper-*-medium` listées dans `VOICES`
  (`gen_corpus.py`).

`fastconformer-multi-int8` est une quantification int8 dynamique
(onnxruntime) de l'export fp32, faite localement.

`real.tsv` (facultatif) : `chemin<TAB>langue<TAB>référence`, une ligne par
enregistrement réel à ajouter au corpus.

## Lancer

```bash
venv/bin/python -I gen_corpus.py                        # corpus/ + manifest.tsv (graine 7)
venv/bin/python -I bench.py run parakeet-v3 --threads 4  # results/parakeet-v3-t4.json
venv/bin/python -I bench.py run parakeet-v3 --threads 4 --load 4   # 4 processus occupés à côté
venv/bin/python -I bench.py vad                          # Silero : parole trouvée, segments sur bruit
venv/bin/python -I bench.py report                       # results/report.md
```

Un modèle par processus, pour des mesures de mémoire propres. Les variantes
`whisper-*-oracle` reçoivent la vraie langue du clip (borne haute de Whisper).

La chaîne complète de l'application (VAD, découpage, filtre) se mesure avec
`crosstalk-asr-bench --manifest corpus/manifest.tsv --scenario clean,combat`
(`src-tauri/examples/asr_bench.rs`).
