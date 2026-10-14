#!/usr/bin/env python3
"""Builds src-tauri/tests/fixtures/marian-tiny: a tiny Opus-MT-format model
(random weights) with its expected outputs computed by Hugging Face
transformers, the reference the Rust translation engine is checked against.

    python -m venv venv && venv/bin/pip install "transformers==4.46.3" "jax[cpu]" flax sentencepiece numpy
    venv/bin/python scripts/make-marian-fixture.py [OUT_DIR] [--full-size]

--full-size writes a model with the real Opus-MT dimensions (d_model 512,
6+6 layers, 64k vocabulary) for timing only: never committed, never accurate.

The weights file is written in PyTorch's zip format (data.pkl + storages)
without PyTorch, exactly as `torch.save` lays it out.

    python scripts/make-marian-fixture.py --legacy-copy [OUT_DIR]

rewrites OUT_DIR/pytorch_model.bin as pytorch_model.legacy.bin in PyTorch's
legacy (pre-1.6) format, the one of the real Helsinki-NLP checkpoints: same
weights, tied embeddings sharing one storage, `_metadata` like a state dict.
"""
import collections
import io
import json
import os
import pickle
import sys
import tempfile
import types
import zipfile

import numpy as np
import sentencepiece as spm

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FULL = "--full-size" in sys.argv
args = [a for a in sys.argv[1:] if not a.startswith("--")]
OUT = args[0] if args else os.path.join(ROOT, "src-tauri", "tests", "fixtures", "marian-tiny")

SOURCE_TEXT = """Двое идут на B, у них снайпер.
Бомба на A, нужен флеш и смок.
Раш Б, раш Б, не стоим!
Я без денег, играем эко.
Один на миде, он с AWP.
Сзади, сзади, они обходят!
Дефуз, кто-нибудь дефузит?
Two players on B, they have an AWP.
Bomb planted on A, I need a flash and a smoke.
Rush B, don't stop!
I have no money, we play eco.
One on mid with the AWP.
Behind us, they are flanking!
Who is defusing? Defuse the bomb now.
Хорошо, жду вас на точке. Спасибо, ребята.
Nice shot, good game everyone.
"""
TARGET_TEXT = """Deux joueurs sur B, ils ont un sniper.
La bombe est sur A, il faut une flash et une smoke.
Rush B, on ne s'arrête pas !
Je n'ai plus d'argent, on joue eco.
Un au milieu avec l'AWP.
Derrière, derrière, ils nous contournent !
Qui désamorce ? Désamorcez la bombe maintenant.
D'accord, je vous attends sur le point. Merci les gars.
Joli tir, bonne partie à tous.
"""

TESTS = [
    "Двое идут на B, у них снайпер.",
    "Rush B, don't stop!",
    "  Бомба   на A,\tнужен флеш  ",
    "Ｆｕｌｌ ｗｉｄｔｈ ＡＷＰ ①",
    "emoji 😀 unknown ☃☃ chars",
    "Nice shot everyone",
    "",
    "smoke",
    "ÉTÉ déjà vu, ça marche",
    "Сзади!!! Они обходят...",
]


def train(text, prefix, vocab):
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False, encoding="utf-8") as f:
        f.write(text * 20)
        path = f.name
    spm.SentencePieceTrainer.train(
        input=path, model_prefix=prefix, vocab_size=vocab, model_type="unigram",
        character_coverage=1.0, hard_vocab_limit=False, minloglevel=2,
    )
    os.unlink(path)
    return spm.SentencePieceProcessor(model_file=prefix + ".model")


# ---------- PyTorch zip writer (no torch needed) ----------

class _Storage:  # stands for torch.FloatStorage
    pass


def _rebuild_tensor_v2(*a):  # stands for torch._utils._rebuild_tensor_v2
    raise RuntimeError("write-only stub")


torch_mod = types.ModuleType("torch")
utils_mod = types.ModuleType("torch._utils")
_Storage.__module__, _Storage.__qualname__, _Storage.__name__ = "torch", "FloatStorage", "FloatStorage"
_rebuild_tensor_v2.__module__, _rebuild_tensor_v2.__qualname__ = "torch._utils", "_rebuild_tensor_v2"
torch_mod.FloatStorage = _Storage
utils_mod._rebuild_tensor_v2 = _rebuild_tensor_v2
torch_mod._utils = utils_mod


class _Tensor:
    def __init__(self, key, array):
        self.key, self.array = key, array

    def __reduce__(self):
        a = self.array
        stride = tuple(s // a.itemsize for s in a.strides)
        return (_rebuild_tensor_v2,
                (_StorageRef(self.key, a.size), 0, a.shape, stride, False, collections.OrderedDict()))


class _StorageRef:
    def __init__(self, key, numel):
        self.key, self.numel = key, numel


class _Pickler(pickle.Pickler):
    def persistent_id(self, obj):
        if isinstance(obj, _StorageRef):
            return ("storage", _Storage, obj.key, "cpu", obj.numel)
        return None


def save_pth(state, path):
    sys.modules["torch"], sys.modules["torch._utils"] = torch_mod, utils_mod
    try:
        tensors = collections.OrderedDict()
        blobs = []
        for i, (name, arr) in enumerate(state.items()):
            arr = np.ascontiguousarray(arr, dtype=np.float32)
            tensors[name] = _Tensor(str(i), arr)
            blobs.append((str(i), arr.tobytes()))
        buf = io.BytesIO()
        _Pickler(buf, protocol=2).dump(tensors)
    finally:
        del sys.modules["torch"], sys.modules["torch._utils"]
    with zipfile.ZipFile(path, "w", zipfile.ZIP_STORED) as z:
        z.writestr("archive/data.pkl", buf.getvalue())
        z.writestr("archive/byteorder", "little")
        for key, data in blobs:
            z.writestr(f"archive/data/{key}", data)
        z.writestr("archive/version", "3\n")


def load_pth(path):
    """Reads a zip checkpoint written by save_pth back into numpy arrays."""
    with zipfile.ZipFile(path) as z:
        def persistent_load(pid):
            return np.frombuffer(z.read(f"archive/data/{pid[2]}"), dtype=np.float32)

        class U(pickle.Unpickler):
            def find_class(self, module, name):
                if (module, name) == ("torch._utils", "_rebuild_tensor_v2"):
                    return lambda st, off, size, stride, *_: np.lib.stride_tricks.as_strided(
                        st[off:], shape=size, strides=[x * 4 for x in stride]).copy()
                if (module, name) == ("torch", "FloatStorage"):
                    return _Storage
                return super().find_class(module, name)

        u = U(io.BytesIO(z.read("archive/data.pkl")))
        u.persistent_load = persistent_load
        return u.load()


def save_pth_legacy(state, path):
    """torch.serialization._legacy_save, protocol 2, without PyTorch."""
    sys.modules["torch"], sys.modules["torch._utils"] = torch_mod, utils_mod
    try:
        tensors, storages, seen = collections.OrderedDict(), {}, {}
        for name, arr in state.items():
            arr = np.ascontiguousarray(arr, dtype=np.float32)
            data = arr.tobytes()
            key = seen.setdefault(data, str(140000000000 + 64 * len(seen)))  # like str(id(storage))
            storages[key] = (arr.size, data)
            tensors[name] = _Tensor(key, arr)
        tensors._metadata = collections.OrderedDict([("", {"version": 1})])

        class P(_Pickler):
            def persistent_id(self, obj):
                if isinstance(obj, _StorageRef):
                    return ("storage", _Storage, obj.key, "cpu", obj.numel, None)
                return None

        buf = io.BytesIO()
        pickle.dump(0x1950A86A20F9469CFC6C, buf, protocol=2)
        pickle.dump(1001, buf, protocol=2)
        pickle.dump({"protocol_version": 1001, "little_endian": True,
                     "type_sizes": {"short": 2, "int": 4, "long": 4}}, buf, protocol=2)
        P(buf, protocol=2).dump(tensors)
        keys = sorted(storages)
        pickle.dump(keys, buf, protocol=2)
    finally:
        del sys.modules["torch"], sys.modules["torch._utils"]
    with open(path, "wb") as f:
        f.write(buf.getvalue())
        for k in keys:
            n, data = storages[k]
            f.write(n.to_bytes(8, "little"))
            f.write(data)


# ---------- PyTorch beam search, ported to numpy ----------

EOS_BOOST = float(os.environ.get("EOS_BOOST", "6"))


def pt_beam_search(model, ids, num_beams, max_new, eos, pad, start, eos_boost):
    """transformers 4.46 `_beam_search` + `BeamSearchScorer` (batch 1,
    length_penalty 1, early_stopping False, forced_eos at max length), with
    `eos_boost` added to the </s> logit. Logits from full forward passes."""
    enc = np.array([ids])

    def log_probs(prefix):
        logits = np.asarray(model(input_ids=enc, decoder_input_ids=np.array([prefix])).logits[0, -1], dtype=np.float64)
        logits[eos] += eos_boost
        m = logits.max()
        return logits - m - np.log(np.exp(logits - m).sum())

    max_length = 1 + max_new
    seqs = [[start] for _ in range(num_beams)]
    beam_scores = np.array([0.0] + [-1e9] * (num_beams - 1))
    hyps, worst = [], 1e9

    def add(tokens, sum_lp, generated_len):
        nonlocal worst
        score = sum_lp / generated_len
        if len(hyps) < num_beams or score > worst:
            hyps.append((score, tokens))
            if len(hyps) > num_beams:
                hyps.sort(key=lambda h: h[0])
                hyps.pop(0)
                worst = hyps[0][0]
            else:
                worst = min(score, worst)

    done = False
    while len(seqs[0]) < max_length:
        cur = len(seqs[0])
        scores = np.stack([log_probs(sq) for sq in seqs])
        scores[:, pad] = -np.inf  # bad_words_ids, after log_softmax
        if cur == max_length - 1:  # ForcedEOSTokenLogitsProcessor
            scores[:, :] = -np.inf
            scores[:, eos] = 0.0
        nxt = scores + beam_scores[:, None]
        flat = nxt.reshape(-1)
        top = np.argsort(-flat, kind="stable")[: 2 * num_beams]
        vocab = scores.shape[1]
        new = []
        for rank, i in enumerate(top):
            tok, b, sc = int(i % vocab), int(i // vocab), float(flat[i])
            if tok == eos:
                if rank >= num_beams:
                    continue
                add(seqs[b][1:], sc, cur + 1 - 1)
            else:
                new.append((sc, tok, b))
            if len(new) == num_beams:
                break
        best = float(flat[top[0]])
        if len(hyps) >= num_beams and worst >= best / (cur + 1 - 1):
            done = True
            break
        seqs = [seqs[b] + [tok] for _, tok, b in new]
        beam_scores = np.array([sc for sc, _, _ in new])
    if not done:
        for sq, sc in zip(seqs, beam_scores):
            add(sq[1:], float(sc), len(sq) - 1)
    return max(hyps, key=lambda h: h[0])[1]


# ---------- model ----------

def flax_to_pt(params):
    out = collections.OrderedDict()

    def walk(prefix, node):
        for k, v in node.items():
            name = f"{prefix}.{k}" if prefix else k
            if isinstance(v, dict):
                walk(name, v)
                continue
            v = np.asarray(v)
            if name.endswith(".kernel"):
                out[name[: -len("kernel")] + "weight"] = v.T
            elif name.endswith(".scale"):
                out[name[: -len("scale")] + "weight"] = v
            elif name.endswith(".embedding"):
                out[name[: -len("embedding")] + "weight"] = v
            else:
                out[name] = v
    walk("", params)
    if "final_logits_bias" in out:
        out["final_logits_bias"] = out["final_logits_bias"].reshape(1, -1)
    return out


def main():
    if "--legacy-copy" in sys.argv:
        state = load_pth(os.path.join(OUT, "pytorch_model.bin"))
        save_pth_legacy(state, os.path.join(OUT, "pytorch_model.legacy.bin"))
        print(len(state), "tensors written to", os.path.join(OUT, "pytorch_model.legacy.bin"))
        return
    from transformers import FlaxMarianMTModel, MarianConfig, MarianTokenizer

    os.makedirs(OUT, exist_ok=True)
    src = train(SOURCE_TEXT, os.path.join(OUT, "source"), 160)
    tgt = train(TARGET_TEXT, os.path.join(OUT, "target"), 140)
    os.replace(os.path.join(OUT, "source.model"), os.path.join(OUT, "source.spm"))
    os.replace(os.path.join(OUT, "target.model"), os.path.join(OUT, "target.spm"))
    for f in ("source.vocab", "target.vocab"):
        os.remove(os.path.join(OUT, f))

    vocab = {"</s>": 0, "<unk>": 1}
    for sp in (src, tgt):
        for i in range(sp.get_piece_size()):
            p = sp.id_to_piece(i)
            if p not in vocab and p not in ("<s>", "</s>", "<unk>"):
                vocab[p] = len(vocab)
    if FULL:
        while len(vocab) < 64376:
            vocab[f"<filler{len(vocab)}>"] = len(vocab)
    vocab["<pad>"] = len(vocab)
    v = len(vocab)
    with open(os.path.join(OUT, "vocab.json"), "w", encoding="utf-8") as f:
        json.dump(vocab, f, ensure_ascii=False, indent=1)

    dims = dict(d_model=512, encoder_layers=6, decoder_layers=6, encoder_attention_heads=8,
                decoder_attention_heads=8, encoder_ffn_dim=2048, decoder_ffn_dim=2048,
                max_position_embeddings=512) if FULL else dict(
                d_model=32, encoder_layers=2, decoder_layers=2, encoder_attention_heads=4,
                decoder_attention_heads=4, encoder_ffn_dim=64, decoder_ffn_dim=64,
                max_position_embeddings=128)
    config = MarianConfig(vocab_size=v, decoder_vocab_size=v, pad_token_id=v - 1, eos_token_id=0,
                          decoder_start_token_id=v - 1, forced_eos_token_id=0, bos_token_id=0,
                          activation_function="swish", scale_embedding=True,
                          share_encoder_decoder_embeddings=True, static_position_embeddings=True,
                          bad_words_ids=[[v - 1]], init_std=0.5, **dims)
    model = FlaxMarianMTModel(config, seed=7)
    rng = np.random.default_rng(3)
    params = model.params
    params["final_logits_bias"] = rng.normal(0, 0.5, size=np.asarray(params["final_logits_bias"]).shape).astype(np.float32)
    model.params = params
    cfg = config.to_dict()
    cfg["architectures"] = ["MarianMTModel"]
    with open(os.path.join(OUT, "config.json"), "w") as f:
        json.dump(cfg, f, indent=1)
    save_pth(flax_to_pt(model.params), os.path.join(OUT, "pytorch_model.bin"))
    if FULL:
        print("full-size timing model written to", OUT)
        return

    tok = MarianTokenizer(source_spm=os.path.join(OUT, "source.spm"),
                          target_spm=os.path.join(OUT, "target.spm"),
                          vocab=os.path.join(OUT, "vocab.json"))
    cases = []
    for text in TESTS:
        pieces = src.encode(text, out_type=str)
        ids = tok(text).input_ids
        case = {"text": text, "normalized": src.normalize(text) if hasattr(src, "normalize") else None,
                "pieces": pieces, "ids": ids}
        if len(ids) > 1:
            gen = model.generate(np.array([ids]), num_beams=1, do_sample=False, max_new_tokens=12,
                                 bad_words_ids=[[v - 1]]).sequences[0].tolist()
            case["generated"] = gen
            # Beam search as the app runs it (num_beams=4, length_penalty=1, no early stop).
            beam = model.generate(np.array([ids]), num_beams=4, do_sample=False, max_new_tokens=12,
                                  bad_words_ids=[[v - 1]], early_stopping=False,
                                  length_penalty=1.0).sequences[0].tolist()
            case["beam4"] = beam
            # Hypotheses that end early (</s> made likelier): exercises the
            # finished-hypotheses logic, against a numpy port of PyTorch's
            # beam search (the Flax one above uses a different algorithm).
            case["beam4_eos"] = pt_beam_search(model, ids, num_beams=4, max_new=12, eos=0, pad=v - 1,
                                               start=v - 1, eos_boost=EOS_BOOST)
            case["decoded"] = tok.decode(gen, skip_special_tokens=True)
        cases.append(case)
    # The target tokenizer round trip, independent of the random model.
    tgt_cases = [{"text": t, "pieces": tgt.encode(t, out_type=str),
                  "decoded": tgt.decode(tgt.encode(t, out_type=str))} for t in TARGET_TEXT.splitlines() if t]
    with open(os.path.join(OUT, "expected.json"), "w", encoding="utf-8") as f:
        json.dump({"source": cases, "target": tgt_cases}, f, ensure_ascii=False, indent=1)
    print("fixture written to", OUT, "vocab", v)


if __name__ == "__main__":
    main()
