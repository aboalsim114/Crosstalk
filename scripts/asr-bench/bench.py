"""Crosstalk ASR benchmark: one candidate per process (clean memory numbers).

    python bench.py run <model> [--threads 4] [--load N]   -> results/<model>-t<threads>[-load<N>].json
    python bench.py vad                                     -> results/vad.json
    python bench.py report                                  -> results/report.md

<model>: parakeet-v3 | fastconformer-multi[-int8] | gigaam-v2 | qwen3-asr | omnilingual-300m
         | whisper-base[-oracle] | whisper-small[-oracle] (reference only, not shipped)
--load N: N busy processes at normal priority during decoding (stand-in for CS2).
"""
import json
import multiprocessing as mp
import os
import re
import sys
import threading
import time
import unicodedata

import numpy as np
import psutil
import sherpa_onnx
import soundfile as sf

ROOT = os.path.dirname(os.path.abspath(__file__))
M = os.path.join(ROOT, "models")
RES = os.path.join(ROOT, "results")


def find(d, pat):
    import glob

    hits = sorted(glob.glob(os.path.join(M, d, pat)))
    if not hits:
        raise SystemExit(f"missing {d}/{pat}")
    return hits[0]


def make(model, threads, lang=""):
    if model == "parakeet-v3":
        d = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"
        return sherpa_onnx.OfflineRecognizer.from_transducer(
            encoder=find(d, "encoder*.onnx"), decoder=find(d, "decoder*.onnx"), joiner=find(d, "joiner*.onnx"),
            tokens=find(d, "tokens.txt"), num_threads=threads, model_type="nemo_transducer")
    if model == "fastconformer-multi":
        d = "sherpa-onnx-nemo-fast-conformer-transducer-be-de-en-es-fr-hr-it-pl-ru-uk-20k"
        return sherpa_onnx.OfflineRecognizer.from_transducer(
            encoder=find(d, "encoder*.onnx"), decoder=find(d, "decoder*.onnx"), joiner=find(d, "joiner*.onnx"),
            tokens=find(d, "tokens.txt"), num_threads=threads, model_type="nemo_transducer")
    if model == "fastconformer-multi-int8":
        d = "fastconformer-multi-int8"
        return sherpa_onnx.OfflineRecognizer.from_transducer(
            encoder=find(d, "encoder*.onnx"), decoder=find(d, "decoder*.onnx"), joiner=find(d, "joiner*.onnx"),
            tokens=find(d, "tokens.txt"), num_threads=threads, model_type="nemo_transducer")
    if model == "qwen3-asr":
        d = "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25"
        return sherpa_onnx.OfflineRecognizer.from_qwen3_asr(
            conv_frontend=find(d, "conv_frontend.onnx"), encoder=find(d, "encoder*.onnx"), decoder=find(d, "decoder*.onnx"),
            tokenizer=os.path.join(M, d, "tokenizer"), num_threads=threads)
    if model == "omnilingual-300m":
        d = "sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-int8-2025-11-12"
        return sherpa_onnx.OfflineRecognizer.from_omnilingual_asr_ctc(
            model=find(d, "model*.onnx"), tokens=find(d, "tokens.txt"), num_threads=threads)
    if model == "gigaam-v2":
        d = "sherpa-onnx-nemo-transducer-giga-am-v2-russian-2025-04-19"
        return sherpa_onnx.OfflineRecognizer.from_transducer(
            encoder=find(d, "encoder*.onnx"), decoder=find(d, "decoder*.onnx"), joiner=find(d, "joiner*.onnx"),
            tokens=find(d, "tokens.txt"), num_threads=threads, model_type="nemo_transducer")
    if model.startswith("whisper-"):
        size = model.split("-")[1]
        d = f"sherpa-onnx-whisper-{size}"
        return sherpa_onnx.OfflineRecognizer.from_whisper(
            encoder=find(d, f"{size}-encoder.int8.onnx"), decoder=find(d, f"{size}-decoder.int8.onnx"),
            tokens=find(d, f"{size}-tokens.txt"), num_threads=threads, language=lang, task="transcribe")
    raise SystemExit("unknown model " + model)


def manifest():
    rows = []
    with open(os.path.join(ROOT, "corpus", "manifest.tsv"), encoding="utf-8") as f:
        head = f.readline().rstrip("\n").split("\t")
        for line in f:
            rows.append(dict(zip(head, line.rstrip("\n").split("\t"))))
    return rows


# ---------- text normalisation for scoring ----------
DIGITS = {
    "ru": {"1": "один", "2": "двое", "3": "трое", "4": "четверо", "5": "пятеро"},
    "en": {"1": "one", "2": "two", "3": "three", "4": "four", "5": "five"},
    "fr": {"1": "un", "2": "deux", "3": "trois", "4": "quatre", "5": "cinq"},
}
LETTERS = {
    "ru": {"бэ": "б", "би": "б", "b": "б", "a": "а", "эй": "а", "ей": "а"},
    "en": {"bee": "b", "be": "b", "ay": "a", "eh": "a"},
    "fr": {"bé": "b", "be": "b", "bee": "b"},
}


def norm(text, lang):
    t = unicodedata.normalize("NFC", text.lower()).replace("ё", "е").replace("’", "'")
    t = re.sub(r"[^\w\s']", " ", t)
    t = re.sub(r"(\d)([^\W\d_])", r"\1 \2", t)  # "2b" -> "2 b"
    words = []
    for w in t.split():
        w = DIGITS.get(lang, {}).get(w, w)
        w = LETTERS.get(lang, {}).get(w, w)
        words.append(w)
    return " ".join(words)


def edit(a, b):
    d = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        prev, d[0] = d[0], i
        for j, y in enumerate(b, 1):
            cur = min(d[j] + 1, d[j - 1] + 1, prev + (x != y))
            prev, d[j] = d[j], cur
    return d[len(b)]


def key_ok(hyp, keys, lang):
    words = hyp.split()
    for k in keys.split("|"):
        k = norm(k, lang)
        stem = k[:4] if lang == "ru" and len(k) > 4 else k
        if not any(w == k or (lang == "ru" and len(k) > 4 and w.startswith(stem)) for w in words):
            return False
    return True


def script_ok(text, lang):
    cyr = len(re.findall(r"[а-яё]", text.lower()))
    lat = len(re.findall(r"[a-zàâçéèêëîïôûùüÿœ]", text.lower()))
    if cyr + lat == 0:
        return False
    return cyr > lat if lang == "ru" else lat > cyr


def busy():
    while True:
        pass


class Peak:
    def __init__(self):
        self.p = psutil.Process()
        self.peak = self.p.memory_info().rss
        self.stop = False
        threading.Thread(target=self.run, daemon=True).start()

    def run(self):
        while not self.stop:
            self.peak = max(self.peak, self.p.memory_info().rss)
            time.sleep(0.02)


def run(model, threads, load):
    rows = manifest()
    proc = psutil.Process()
    rss0 = proc.memory_info().rss
    peak = Peak()
    t0 = time.perf_counter()
    oracle = model.endswith("-oracle")
    base = model[: -len("-oracle")] if oracle else model
    if oracle:
        recs = {l: make(base, threads, l) for l in ("ru", "en", "fr")}
        rec = recs["en"]
    else:
        rec = make(base, threads)
    load_s = time.perf_counter() - t0
    rss_loaded = proc.memory_info().rss
    # Warm-up (first call allocates).
    s = rec.create_stream()
    s.accept_waveform(16000, np.zeros(16000, np.float32))
    rec.decode_stream(s)
    workers = [mp.Process(target=busy, daemon=True) for _ in range(load)]
    for w in workers:
        w.start()
    out = []
    cpu0 = proc.cpu_times()
    wall0 = time.perf_counter()
    for r in rows:
        x, sr = sf.read(os.path.join(ROOT, r["path"]), dtype="float32")
        rr = recs.get(r["lang"], rec) if oracle else rec
        s = rr.create_stream()
        s.accept_waveform(sr, x)
        t = time.perf_counter()
        rr.decode_stream(s)
        ms = (time.perf_counter() - t) * 1000
        out.append({"id": r["id"], "text": s.result.text.strip(), "ms": ms, "audio_ms": len(x) * 1000 / sr})
    wall = time.perf_counter() - wall0
    cpu1 = proc.cpu_times()
    for w in workers:
        w.terminate()
    peak.stop = True
    res = {
        "model": model, "threads": threads, "load": load, "load_s": load_s,
        "rss_base_mb": rss0 / 2**20, "rss_loaded_mb": rss_loaded / 2**20, "rss_peak_mb": peak.peak / 2**20,
        "cpu_percent_of_machine": 100 * ((cpu1.user - cpu0.user) + (cpu1.system - cpu0.system)) / wall / psutil.cpu_count(),
        "clips": out,
    }
    os.makedirs(RES, exist_ok=True)
    name = f"{model}-t{threads}" + (f"-load{load}" if load else "")
    json.dump(res, open(os.path.join(RES, name + ".json"), "w"), ensure_ascii=False, indent=1)
    print(name, f"load {load_s:.1f}s rss {rss_loaded/2**20:.0f}MB peak {peak.peak/2**20:.0f}MB wall {wall:.1f}s")


def vad():
    """Silero VAD (sherpa-onnx), the same for every candidate: recall on speech, false alarms on noise."""
    cfg = sherpa_onnx.VadModelConfig()
    cfg.silero_vad.model = os.path.join(M, "silero_vad.onnx")
    cfg.silero_vad.threshold = 0.5
    cfg.silero_vad.min_silence_duration = 0.25
    cfg.silero_vad.min_speech_duration = 0.15
    cfg.sample_rate = 16000
    out = []
    for r in manifest():
        x, sr = sf.read(os.path.join(ROOT, r["path"]), dtype="float32")
        v = sherpa_onnx.VoiceActivityDetector(cfg, buffer_size_in_seconds=30)
        segs = 0
        for i in range(0, len(x) - 511, 512):
            v.accept_waveform(x[i : i + 512])
            while not v.empty():
                segs += 1
                v.pop()
        v.flush()
        while not v.empty():
            segs += 1
            v.pop()
        out.append({"id": r["id"], "category": r["category"], "condition": r["condition"], "segments": segs})
    os.makedirs(RES, exist_ok=True)
    json.dump(out, open(os.path.join(RES, "vad.json"), "w"), indent=1)
    sp = [o for o in out if o["category"] != "noise"]
    no = [o for o in out if o["category"] == "noise"]
    print("speech detected", sum(o["segments"] > 0 for o in sp), "/", len(sp), "; noise clips with a segment", sum(o["segments"] > 0 for o in no), "/", len(no))


def pct(v, q):
    return float(np.percentile(v, q)) if v else float("nan")


def score(res, rows):
    by = {r["id"]: r for r in rows}
    g = {}
    for c in res["clips"]:
        r = by[c["id"]]
        lang = r["lang"]
        hyp_raw = c["text"]
        if r["category"] == "noise":
            k = g.setdefault("noise", {"n": 0, "false": 0, "ms": []})
            k["n"] += 1
            k["false"] += bool(re.search(r"\w", hyp_raw))
            k["ms"].append(c["ms"])
            continue
        if not r["reference"]:
            g.setdefault("noref", []).append((c["id"], hyp_raw))
            continue
        hyp, ref = norm(hyp_raw, lang), norm(r["reference"], lang)
        group = f"{lang}/{r['category']}/{r['condition'].replace('-fast','')}"
        for key in (group, f"{lang}/ALL", "ALL"):
            k = g.setdefault(key, {"n": 0, "we": 0, "wn": 0, "ce": 0, "cn": 0, "miss": 0, "keys": 0, "keyn": 0, "script": 0, "ms": [], "rtf": []})
            k["n"] += 1
            k["we"] += edit(ref.split(), hyp.split())
            k["wn"] += max(1, len(ref.split()))
            k["ce"] += edit(list(ref.replace(" ", "")), list(hyp.replace(" ", "")))
            k["cn"] += max(1, len(ref.replace(" ", "")))
            k["miss"] += hyp.strip() == ""
            if r["keys"] and r["category"] != "overlap":
                k["keyn"] += 1
                k["keys"] += key_ok(hyp, r["keys"], lang)
            k["script"] += script_ok(hyp_raw, lang)
            k["ms"].append(c["ms"])
            k["rtf"].append(c["ms"] / c["audio_ms"])
    return g


def report():
    rows = manifest()
    files = sorted(f for f in os.listdir(RES) if f.endswith(".json") and f != "vad.json")
    lines = ["# Résultats bruts\n"]
    summary = ["| Modèle | Fils | Charge | WER | CER | Annonces justes | Manquées | Langue (écriture) | Faux sur bruit | p50 ms | p95 ms | RTF moyen | RAM chargé | RAM pic | Chargement |",
               "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    detail = []
    for f in files:
        res = json.load(open(os.path.join(RES, f)))
        g = score(res, rows)
        a, n = g["ALL"], g.get("noise", {"n": 0, "false": 0})
        summary.append(
            f"| {res['model']} | {res['threads']} | {res['load']} | {100*a['we']/a['wn']:.1f} % | {100*a['ce']/a['cn']:.1f} % | "
            f"{a['keys']}/{a['keyn']} | {a['miss']}/{a['n']} | {a['script']}/{a['n']} | {n['false']}/{n['n']} | "
            f"{pct(a['ms'],50):.0f} | {pct(a['ms'],95):.0f} | {np.mean(a['rtf']):.3f} | {res['rss_loaded_mb']-res['rss_base_mb']:.0f} Mo | "
            f"{res['rss_peak_mb']-res['rss_base_mb']:.0f} Mo | {res['load_s']:.1f} s |")
        if res["load"] == 0 and res["threads"] == 4:
            detail.append(f"\n## {res['model']} (4 fils)\n\n| Groupe | n | WER | CER | Annonces | Manquées | p50 ms |\n|---|---|---|---|---|---|---|")
            for key in sorted(k for k in g if k not in ("ALL", "noise", "noref")):
                k = g[key]
                kk = f"{k['keys']}/{k['keyn']}" if k["keyn"] else "—"
                detail.append(f"| {key} | {k['n']} | {100*k['we']/k['wn']:.1f} % | {100*k['ce']/k['cn']:.1f} % | {kk} | {k['miss']} | {pct(k['ms'],50):.0f} |")
            for cid, text in g.get("noref", []):
                detail.append(f"\n- `{cid}` (sans référence) : « {text} »")
    lines += summary + detail
    if os.path.exists(os.path.join(RES, "vad.json")):
        v = json.load(open(os.path.join(RES, "vad.json")))
        sp = [o for o in v if o["category"] != "noise"]
        no = [o for o in v if o["category"] == "noise"]
        lines.append(f"\n## Silero VAD\n\nParole détectée : {sum(o['segments']>0 for o in sp)}/{len(sp)} clips ; bruit seul avec au moins un segment : {sum(o['segments']>0 for o in no)}/{len(no)}.")
    open(os.path.join(RES, "report.md"), "w", encoding="utf-8").write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "run":
        args = sys.argv[2:]
        model = args[0]
        threads = int(args[args.index("--threads") + 1]) if "--threads" in args else 4
        load = int(args[args.index("--load") + 1]) if "--load" in args else 0
        run(model, threads, load)
    elif cmd == "vad":
        vad()
    elif cmd == "report":
        report()
