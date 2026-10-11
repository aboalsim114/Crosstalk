#!/usr/bin/env python3
"""Downloads a small reference corpus for crosstalk-asr-bench.

FLEURS (google/fleurs, CC-BY-4.0): read sentences with expert transcriptions,
16 kHz WAV. Takes the first N distinct clips of the `dev` split for Russian and
English and writes them with a manifest:

    corpus/ru/<id>.wav, corpus/en/<id>.wav
    corpus/manifest.tsv   path<TAB>lang<TAB>reference

The archives are streamed and the download stops once N clips are found.
Read speech is cleaner than CS2 voice chat: these numbers are an upper bound,
completed by the synthetic combat noise of the bench and by real recordings.

Usage: python scripts/fetch-asr-corpus.py [--out corpus] [--per-lang 20]
"""
import argparse
import csv
import io
import os
import sys
import tarfile
import urllib.request

BASE = "https://huggingface.co/datasets/google/fleurs/resolve/main/data"
LANGS = {"ru": "ru_ru", "en": "en_us"}


def open_url(url):
    req = urllib.request.Request(url, headers={"User-Agent": "crosstalk-asr-bench"})
    token = os.environ.get("HF_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    return urllib.request.urlopen(req, timeout=60)


def fetch(lang, config, out, per_lang):
    with open_url(f"{BASE}/{config}/dev.tsv") as r:
        rows = list(csv.reader(io.TextIOWrapper(r, encoding="utf-8"), delimiter="\t", quoting=csv.QUOTE_NONE))
    # One clip per sentence id: distinct sentences and speakers.
    wanted, seen = {}, set()
    for row in rows:
        sid, fname, raw = row[0], row[1], row[2]
        if sid not in seen:
            seen.add(sid)
            wanted[fname] = raw
    os.makedirs(os.path.join(out, lang), exist_ok=True)
    found = []
    with open_url(f"{BASE}/{config}/audio/dev.tar.gz") as r:
        with tarfile.open(fileobj=r, mode="r|gz") as tar:
            for member in tar:
                name = os.path.basename(member.name)
                if not member.isfile() or name not in wanted:
                    continue
                data = tar.extractfile(member).read()
                rel = f"{lang}/{name}"
                with open(os.path.join(out, rel), "wb") as f:
                    f.write(data)
                found.append((rel, lang, wanted[name]))
                if len(found) >= per_lang:
                    break
    return found


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--out", default="corpus")
    p.add_argument("--per-lang", type=int, default=20)
    a = p.parse_args()
    lines = []
    for lang, config in LANGS.items():
        clips = fetch(lang, config, a.out, a.per_lang)
        print(f"{lang}: {len(clips)} extraits", file=sys.stderr)
        lines += clips
    with open(os.path.join(a.out, "manifest.tsv"), "w", encoding="utf-8", newline="\n") as f:
        f.write("# FLEURS dev (google/fleurs, CC-BY-4.0)\n")
        for rel, lang, ref in lines:
            f.write(f"{rel}\t{lang}\t{ref}\n")


if __name__ == "__main__":
    main()
