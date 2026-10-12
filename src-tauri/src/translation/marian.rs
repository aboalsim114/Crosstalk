//! Opus-MT (Marian) translation on the CPU with candle: the files of a
//! Hugging Face `Helsinki-NLP/opus-mt-xx-fr` repository, used as published.
//!
//! Two output heads:
//! - `Mode::Exact`: the full shared vocabulary in f32, token for token what
//!   transformers' greedy search produces (checked by the fixture tests);
//! - `Mode::Fast` (the app): only pieces of the French tokenizer, q8_0
//!   quantized. The output layer dominates the cost of each generated token
//!   (64k-row matrix): this halves its size and its memory traffic.

use std::collections::HashMap;
use std::path::Path;

use candle_core::quantized::{GgmlDType, QMatMul, QTensor};
use candle_core::{DType, Device, Module, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::marian::Config;

use super::network::Network;
use super::spm::SentencePiece;

/// Longest input accepted (tokens): callouts are short; longer text is cut.
const MAX_INPUT_TOKENS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Exact,
    Fast,
}

enum Head {
    /// Shared embeddings (vocab × d), bias (1 × vocab).
    Full(candle_nn::Linear, Tensor),
    /// French pieces only: their vocabulary ids, quantized rows, bias.
    Target(Vec<u32>, QMatMul, Tensor),
}

pub struct Marian {
    model: Network,
    head: Head,
    config: Config,
    source: SentencePiece,
    target: SentencePiece,
    vocab: HashMap<String, u32>,
    ids: Vec<String>,
    unk_id: u32,
    device: Device,
}

impl Marian {
    pub fn load(dir: &Path, mode: Mode) -> Result<Self, String> {
        let read = |name: &str| {
            std::fs::read(dir.join(name)).map_err(|e| format!("{name} illisible : {e}"))
        };
        let config: Config = serde_json::from_slice(&read("config.json")?)
            .map_err(|e| format!("config.json : {e}"))?;
        let vocab: HashMap<String, u32> = serde_json::from_slice(&read("vocab.json")?)
            .map_err(|e| format!("vocab.json : {e}"))?;
        let source = SentencePiece::from_bytes(&read("source.spm")?)?;
        let target = SentencePiece::from_bytes(&read("target.spm")?)?;
        let size = config.decoder_vocab_size.unwrap_or(config.vocab_size);
        let mut ids = vec![String::new(); size.max(vocab.len())];
        for (piece, &id) in &vocab {
            if let Some(slot) = ids.get_mut(id as usize) {
                *slot = piece.clone();
            }
        }
        let unk_id = *vocab.get("<unk>").ok_or("vocab.json sans <unk>")?;
        let device = Device::Cpu;
        let err = |e: candle_core::Error| format!("pytorch_model.bin : {e}");
        let vb = VarBuilder::from_pth(dir.join("pytorch_model.bin"), DType::F32, &device)
            .map_err(err)?;
        let model =
            Network::new(&config, vb.clone()).map_err(|e| format!("modèle Marian : {e}"))?;
        let shared = model.shared().clone();
        let bias = vb.get((1, size), "final_logits_bias").map_err(err)?;
        let head = match mode {
            Mode::Exact => Head::Full(candle_nn::Linear::new(shared, None), bias),
            Mode::Fast => {
                let mut allowed: Vec<u32> = target
                    .text_pieces()
                    .filter_map(|p| vocab.get(p).copied())
                    .filter(|&id| (id as usize) < size && id != config.pad_token_id)
                    .collect();
                allowed.push(config.eos_token_id);
                allowed.sort_unstable();
                allowed.dedup();
                let index = Tensor::new(allowed.as_slice(), &device).map_err(err)?;
                let rows = shared.index_select(&index, 0).map_err(err)?;
                let q = QTensor::quantize(&rows, GgmlDType::Q8_0).map_err(err)?;
                let bias = bias.index_select(&index, 1).map_err(err)?;
                Head::Target(allowed, QMatMul::from_qtensor(q).map_err(err)?, bias)
            }
        };
        Ok(Self {
            model,
            head,
            config,
            source,
            target,
            vocab,
            ids,
            unk_id,
            device,
        })
    }

    /// Source token ids, as MarianTokenizer builds them (pieces + `</s>`).
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .source
            .encode(text)
            .iter()
            .map(|p| *self.vocab.get(p).unwrap_or(&self.unk_id))
            .take(MAX_INPUT_TOKENS - 1)
            .collect();
        ids.push(self.config.eos_token_id);
        ids
    }

    /// Text of generated ids, special tokens removed.
    pub fn decode(&self, ids: &[u32]) -> String {
        let pieces: Vec<&str> = ids
            .iter()
            .filter(|&&id| {
                id != self.config.eos_token_id
                    && id != self.config.pad_token_id
                    && id != self.unk_id
            })
            .filter_map(|&id| self.ids.get(id as usize).map(String::as_str))
            .filter(|p| !self.target.is_special(p))
            .collect();
        self.target.decode(&pieces).trim().to_string()
    }

    /// Next token from the decoder state of the last position.
    fn next_token(&self, hidden: &Tensor) -> candle_core::Result<u32> {
        match &self.head {
            Head::Full(linear, bias) => {
                let logits = linear.forward(hidden)?.broadcast_add(bias)?;
                let mut scores = logits.flatten_all()?.to_vec1::<f32>()?;
                // bad_words_ids = [[pad]]: the padding token is never generated.
                if let Some(s) = scores.get_mut(self.config.pad_token_id as usize) {
                    *s = f32::NEG_INFINITY;
                }
                Ok(argmax(&scores) as u32)
            }
            Head::Target(allowed, q, bias) => {
                let logits = q.forward(hidden)?.broadcast_add(bias)?;
                let scores = logits.flatten_all()?.to_vec1::<f32>()?;
                Ok(allowed[argmax(&scores)])
            }
        }
    }

    /// Greedy decoding (num_beams = 1) of the source ids, at most `max_new` tokens.
    pub fn generate(&mut self, src: &[u32], max_new: usize) -> Result<Vec<u32>, String> {
        let err = |e: candle_core::Error| format!("traduction : {e}");
        let input = Tensor::new(src, &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(err)?;
        self.model.encode(&input).map_err(err)?;
        let eos = self.config.eos_token_id;
        let mut last = self.config.decoder_start_token_id;
        let mut generated = Vec::new();
        for _ in 0..max_new {
            let hidden = self.model.step(last).map_err(err)?;
            let next = self.next_token(&hidden).map_err(err)?;
            if next == eos {
                break;
            }
            generated.push(next);
            last = next;
        }
        Ok(generated)
    }

    pub fn translate(&mut self, text: &str) -> Result<String, String> {
        let src = self.encode(text);
        let max_new = (src.len() * 3).clamp(16, 256);
        let ids = self.generate(&src, max_new)?;
        Ok(self.decode(&ids))
    }
}

fn argmax(scores: &[f32]) -> usize {
    let mut best = 0;
    for (i, &s) in scores.iter().enumerate() {
        if s > scores[best] {
            best = i;
        }
    }
    best
}
