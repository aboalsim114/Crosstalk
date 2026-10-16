"""Builds the Crosstalk ASR benchmark corpus (local, reproducible, seed 7).

- Tactical callouts in Russian and English, French dictation phrases, spoken by
  several Piper voices (sherpa-onnx VITS exports), normal and fast speech.
- Conditions: clean, combat (synthetic shots / explosions / steps mixed in),
  weak (-24 dB with room noise), overlap (two speakers).
- Noise-only clips (no voice) and near-silence, for false transcripts.
- Real human recordings shipped with the ASR model archives (test_wavs).

Writes corpus/<id>.wav (16 kHz mono) and corpus/manifest.tsv:
    id  path  lang  category  condition  reference  keys
`keys`: the words that carry the callout (count, place), "|" separated.
"""
import glob
import json
import os
import random
import sys

import numpy as np
import sherpa_onnx
import soundfile as sf

ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(ROOT, "corpus")
SR = 16000
rng = np.random.default_rng(7)
random.seed(7)

RU = [
    ("Двое на Б", "двое|б"), ("Один мид", "один|мид"), ("Раш А", "раш|а"),
    ("Он за тобой", "за|тобой"), ("Трое на банане", "трое|банане"),
    ("Снайпер на миде", "снайпер|миде"), ("Бомба на А", "бомба|а"),
    ("Сейвимся", "сейвимся"), ("Эко раунд", "эко|раунд"), ("Покупаем всё", "покупаем"),
    ("Один на лонге", "один|лонге"), ("Двое в коннекторе", "двое|коннекторе"),
    ("Дай мне калаш", "калаш"), ("Минус один", "минус|один"),
    ("Он на плэнте Б", "плэнте|б"), ("Кидаю флешку", "флешку"),
    ("Ротейт на Б", "ротейт|б"), ("Помогите на А", "помогите|а"),
    ("Последний в окне", "последний|окне"), ("Отходим", "отходим"),
]
EN = [
    ("One mid", "one|mid"), ("Two B", "two|b"), ("Rush A", "rush|a"),
    ("He's behind you", "behind|you"), ("Three banana", "three|banana"),
    ("AWP mid", "awp|mid"), ("Bomb is A", "bomb|a"), ("Save this round", "save|round"),
    ("Eco round", "eco|round"), ("Full buy", "full|buy"), ("One long", "one|long"),
    ("Two in connector", "two|connector"), ("Drop me an AK", "drop|ak"),
    ("One down", "one|down"), ("He's on B site", "b|site"), ("Flashing now", "flashing"),
    ("Rotate to B", "rotate|b"), ("Need help at A", "help|a"),
    ("Last one in window", "last|window"), ("Fall back", "fall|back"),
]
FR = [
    ("Deux en B", "deux|b"), ("Un au milieu", "un|milieu"), ("On rush A", "rush|a"),
    ("Il est derrière toi", "derrière|toi"), ("Achète-moi une arme", "achète|arme"),
    ("Je n'ai plus de balles", "balles"), ("Attention au sniper", "attention|sniper"),
    ("Bien joué l'équipe", "joué|équipe"), ("On garde nos armes ce round", "armes|round"),
    ("Je lance une flash", "lance|flash"), ("La bombe est en A", "bombe|a"),
    ("Rotation vers B", "rotation|b"),
]
# What the TTS reads (letters spelled the way a player says them).
SAY = {
    "ru": {"Б": "бэ", "А": "а"},
    "en": {},
    "fr": {" B": " bé", " A": " a"},
}
VOICES = {
    "ru": ["dmitri", "denis", "ruslan", "irina"],
    "en": ["ryan", "amy", "alan", "joe"],
    "fr": ["siwis", "tom", "upmc"],
}
LOC = {"ru": "ru_RU", "en": {"alan": "en_GB"}, "fr": "fr_FR"}


def voice_dir(lang, name):
    hits = glob.glob(os.path.join(ROOT, "tts", f"vits-piper-*-{name}-medium"))
    return hits[0] if hits else None


def tts(dirpath):
    onnx = glob.glob(os.path.join(dirpath, "*.onnx"))[0]
    cfg = sherpa_onnx.OfflineTtsConfig(
        model=sherpa_onnx.OfflineTtsModelConfig(
            vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                model=onnx,
                tokens=os.path.join(dirpath, "tokens.txt"),
                data_dir=os.path.join(dirpath, "espeak-ng-data"),
            ),
            num_threads=4,
        )
    )
    return sherpa_onnx.OfflineTts(cfg)


def resample(x, sr):
    if sr == SR:
        return x.astype(np.float32)
    n = int(round(len(x) * SR / sr))
    t = np.linspace(0, len(x) - 1, n)
    return np.interp(t, np.arange(len(x)), x).astype(np.float32)


def db(x):
    return 20 * np.log10(max(np.sqrt(np.mean(x**2)), 1e-9))


def gain_to(x, target_db):
    return x * (10 ** ((target_db - db(x)) / 20))


# ---------- synthetic CS2-like noise ----------
def shot(n):
    d = int(SR * rng.uniform(0.08, 0.22))
    t = np.arange(d) / SR
    burst = rng.normal(0, 1, d) * np.exp(-t / rng.uniform(0.02, 0.06))
    burst += 0.6 * np.sin(2 * np.pi * rng.uniform(60, 140) * t) * np.exp(-t / 0.05)
    return burst / np.max(np.abs(burst))


def explosion():
    d = int(SR * rng.uniform(0.8, 1.6))
    t = np.arange(d) / SR
    x = rng.normal(0, 1, d)
    x = np.convolve(x, np.ones(40) / 40, mode="same")  # low-pass
    x *= np.exp(-t / rng.uniform(0.3, 0.6))
    return x / np.max(np.abs(x))


def steps(n):
    x = np.zeros(n)
    pos = int(SR * rng.uniform(0, 0.3))
    while pos < n - 400:
        click = rng.normal(0, 1, 300) * np.exp(-np.arange(300) / 60)
        x[pos : pos + 300] += click * rng.uniform(0.3, 0.7)
        pos += int(SR * rng.uniform(0.28, 0.4))
    return x


def music(n):
    t = np.arange(n) / SR
    f0 = rng.choice([110, 130.8, 146.8, 164.8])
    x = sum(np.sin(2 * np.pi * f0 * k * t) / k for k in (1, 1.5, 2, 3))
    return x * (0.5 + 0.5 * np.sin(2 * np.pi * 0.5 * t))


def combat_noise(n, events=True):
    x = 0.02 * rng.normal(0, 1, n)  # room / fans
    x += 0.15 * steps(n)
    if rng.random() < 0.4:
        x += 0.08 * music(n)
    if events:
        for _ in range(rng.integers(2, 6)):
            s = shot(n)
            p = rng.integers(0, max(1, n - len(s)))
            x[p : p + len(s)] += s * rng.uniform(0.5, 1.0)
        if rng.random() < 0.5:
            e = explosion()
            p = rng.integers(0, max(1, n - len(e)))
            x[p : p + len(e)] += e[: n - p] * rng.uniform(0.6, 1.0)
    return x.astype(np.float32)


def pad(x, before=0.4, after=0.5):
    return np.concatenate([np.zeros(int(SR * before)), x, np.zeros(int(SR * after))]).astype(np.float32)


def write(rows, cid, audio, lang, cat, cond, ref, keys):
    audio = np.clip(audio, -1, 1).astype(np.float32)
    path = os.path.join(OUT, f"{cid}.wav")
    sf.write(path, audio, SR, subtype="PCM_16")
    rows.append([cid, f"corpus/{cid}.wav", lang, cat, cond, ref, keys])


def main():
    os.makedirs(OUT, exist_ok=True)
    rows = []
    speech = {}
    for lang, phrases in (("ru", RU), ("en", EN), ("fr", FR)):
        for vi, name in enumerate(VOICES[lang]):
            d = voice_dir(lang, name)
            if not d:
                print("voice missing", lang, name, file=sys.stderr)
                continue
            engine = tts(d)
            for pi, (ref, keys) in enumerate(phrases):
                text = ref
                for a, b in SAY[lang].items():
                    text = text.replace(a, b)
                fast = (pi + vi) % 2 == 1
                speed = 1.25 if fast else 1.0
                a = engine.generate(text, sid=0, speed=speed)
                x = resample(np.array(a.samples), a.sample_rate)
                x = gain_to(x, -20)
                speech[(lang, vi, pi)] = (x, ref, keys)
                cid = f"{lang}-v{vi}-p{pi:02d}"
                cat = "dictation" if lang == "fr" else "callout"
                write(rows, cid + "-clean", pad(x), lang, cat, "clean" + ("-fast" if fast else ""), ref, keys)
                if lang != "fr" or vi == 0:
                    v = pad(x, 0.8, 0.8)
                    noise = combat_noise(len(v))
                    write(rows, cid + "-combat", v + gain_to(noise, rng.uniform(-24, -18)) * 1.0 + 0.0, lang, cat, "combat", ref, keys)
                if vi in (0, 1) and lang != "fr":
                    w = pad(gain_to(x, -42))
                    w += 0.004 * rng.normal(0, 1, len(w)).astype(np.float32)
                    write(rows, cid + "-weak", w, lang, cat, "weak", ref, keys)
    # Overlap: two speakers, the second starts 0.35 s later (reference = first).
    keys_ = [k for k in speech if k[0] in ("ru", "en")]
    for i in range(16):
        a, b = random.sample(keys_, 2)
        xa, ra, ka = speech[a]
        xb, rb, kb = speech[b]
        off = int(SR * 0.35)
        n = max(len(xa), len(xb) + off)
        m = np.zeros(n, np.float32)
        m[: len(xa)] += xa
        m[off : off + len(xb)] += xb * 0.8
        write(rows, f"overlap-{i:02d}", pad(m), a[0], "overlap", f"with-{b[0]}", ra, ka + "||" + rb)
    # No voice at all.
    for i in range(40):
        n = int(SR * rng.uniform(3, 6))
        write(rows, f"noise-{i:02d}", gain_to(combat_noise(n), rng.uniform(-26, -14)), "-", "noise", "combat-only", "", "")
    for i in range(10):
        n = int(SR * 4)
        write(rows, f"silence-{i:02d}", 0.002 * rng.normal(0, 1, n).astype(np.float32), "-", "noise", "near-silence", "", "")
    # Real human speech from the model archives (references from their trans.txt / README).
    real = os.path.join(ROOT, "real.tsv")
    if os.path.exists(real):
        for line in open(real, encoding="utf-8"):
            p, lang, ref = line.rstrip("\n").split("\t")
            x, sr = sf.read(p, dtype="float32")
            if x.ndim > 1:
                x = x.mean(axis=1)
            x = resample(x, sr)
            cid = "real-" + lang + "-" + os.path.splitext(os.path.basename(p))[0]
            write(rows, cid, x, lang, "real", "read", ref, "")
            v = pad(x, 0.6, 0.6)
            write(rows, cid + "-combat", v + gain_to(combat_noise(len(v)), db(x) - 4), lang, "real", "read-combat", ref, "")
    with open(os.path.join(OUT, "manifest.tsv"), "w", encoding="utf-8") as f:
        f.write("id\tpath\tlang\tcategory\tcondition\treference\tkeys\n")
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(len(rows), "clips")


if __name__ == "__main__":
    main()
