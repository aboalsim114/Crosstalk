#!/usr/bin/env python3
"""Writes a tiny Gemma 3 GGUF with random weights for crosstalk-llm's tests.

It has TranslateGemma's architecture ("gemma3"), a SentencePiece vocabulary
with Gemma's control tokens (<bos>, <start_of_turn>, <end_of_turn>) and byte
fallback, so llama.cpp loads it, tokenizes the real prompt format, generates
and stops on <end_of_turn> exactly as with the real model. The text it
generates is meaningless: translation quality needs the real model
(docs/reports/translategemma-integration.md).

  pip install gguf numpy
  python scripts/make-tiny-gemma3-gguf.py src-llm/tests/data/tiny-gemma3.gguf
  python scripts/make-tiny-gemma3-gguf.py --stop src-llm/tests/data/tiny-gemma3-stop.gguf

--stop: the first generated token is always <end_of_turn> (tests the stop).
"""
import argparse

import gguf
import numpy as np

EMBD, LAYERS, HEADS, KV_HEADS, HEAD_DIM, FF = 64, 2, 2, 1, 32, 128


def vocab():
    tokens = [("<pad>", 3), ("<eos>", 3), ("<bos>", 3), ("<unk>", 2),
              ("<start_of_turn>", 3), ("<end_of_turn>", 3)]
    tokens += [(f"<0x{b:02X}>", 6) for b in range(256)]
    pieces = ["▁"] + [chr(c) for c in range(ord("a"), ord("z") + 1)]
    pieces += ["▁" + w for w in ["the", "to", "a", "French", "Russian", "English", "text", "translat"]]
    tokens += [(p, 1) for p in pieces]
    return tokens


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--stop", action="store_true")
    a = ap.parse_args()
    rng = np.random.default_rng(7)
    toks = vocab()
    n_vocab = len(toks)
    eot = 5

    w = gguf.GGUFWriter(a.out, "gemma3")
    w.add_name("tiny-gemma3-test")
    w.add_context_length(512)
    w.add_embedding_length(EMBD)
    w.add_block_count(LAYERS)
    w.add_feed_forward_length(FF)
    w.add_head_count(HEADS)
    w.add_head_count_kv(KV_HEADS)
    w.add_key_length(HEAD_DIM)
    w.add_value_length(HEAD_DIM)
    w.add_layer_norm_rms_eps(1e-6)
    w.add_rope_freq_base(10000.0)
    w.add_sliding_window(512)
    w.add_file_type(gguf.LlamaFileType.MOSTLY_F16)

    w.add_tokenizer_model("llama")
    w.add_tokenizer_pre("default")
    w.add_token_list([t for t, _ in toks])
    w.add_token_scores([0.0 if ty != 1 else -float(len(t)) for t, ty in toks])
    w.add_token_types([ty for _, ty in toks])
    w.add_bos_token_id(2)
    w.add_eos_token_id(1)
    w.add_unk_token_id(3)
    w.add_pad_token_id(0)
    w.add_eot_token_id(eot)
    w.add_add_bos_token(True)
    w.add_add_eos_token(False)

    def t(*shape, scale=0.2):
        return (rng.standard_normal(shape) * scale).astype(np.float16)

    emb = t(n_vocab, EMBD)
    if a.stop:
        # Every embedding leans one way, <end_of_turn> leans furthest: with
        # tied output weights it always has the highest logit.
        d = np.ones(EMBD, dtype=np.float32) / np.sqrt(EMBD)
        emb = (emb.astype(np.float32) * 0.05 + d).astype(np.float16)
        emb[eot] = (d * 8).astype(np.float16)
    name = gguf.TENSOR_NAMES
    T = gguf.MODEL_TENSOR
    w.add_tensor(name[T.TOKEN_EMBD] + ".weight", emb)
    w.add_tensor(name[T.OUTPUT_NORM] + ".weight", np.ones(EMBD, dtype=np.float32))
    for i in range(LAYERS):
        def n(k):
            return name[k].format(bid=i) + ".weight"
        zero = 0.0 if a.stop else 0.2
        w.add_tensor(n(T.ATTN_NORM), np.ones(EMBD, dtype=np.float32))
        w.add_tensor(n(T.ATTN_Q), t(HEADS * HEAD_DIM, EMBD, scale=zero))
        w.add_tensor(n(T.ATTN_K), t(KV_HEADS * HEAD_DIM, EMBD, scale=zero))
        w.add_tensor(n(T.ATTN_V), t(KV_HEADS * HEAD_DIM, EMBD, scale=zero))
        w.add_tensor(n(T.ATTN_OUT), t(EMBD, HEADS * HEAD_DIM, scale=zero))
        w.add_tensor(n(T.ATTN_Q_NORM), np.ones(HEAD_DIM, dtype=np.float32))
        w.add_tensor(n(T.ATTN_K_NORM), np.ones(HEAD_DIM, dtype=np.float32))
        w.add_tensor(n(T.ATTN_POST_NORM), np.ones(EMBD, dtype=np.float32))
        w.add_tensor(n(T.FFN_PRE_NORM), np.ones(EMBD, dtype=np.float32))
        w.add_tensor(n(T.FFN_GATE), t(FF, EMBD, scale=zero))
        w.add_tensor(n(T.FFN_UP), t(FF, EMBD, scale=zero))
        w.add_tensor(n(T.FFN_DOWN), t(EMBD, FF, scale=zero))
        w.add_tensor(n(T.FFN_POST_NORM), np.ones(EMBD, dtype=np.float32))
    w.write_header_to_file()
    w.write_kv_data_to_file()
    w.write_tensors_to_file()
    w.close()


if __name__ == "__main__":
    main()
