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
        // Opus-MT checkpoints use PyTorch's legacy (pre-1.6) format.
        let weights = super::pth::load(&dir.join("pytorch_model.bin"), &device)
            .map_err(|e| format!("pytorch_model.bin : {e}"))?;
        let vb = VarBuilder::from_tensors(weights, DType::F32, &device);
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

    /// Log-probabilities of the next token for each hypothesis (rows of
    /// `hidden`, b × 1 × d), as `(token, logprob)` lists. `forced_eos`: last
    /// allowed step, where only `</s>` remains (HF `forced_eos_token_id`).
    fn log_probs(
        &self,
        hidden: &Tensor,
        forced_eos: bool,
    ) -> candle_core::Result<Vec<Vec<(u32, f32)>>> {
        let eos = self.config.eos_token_id;
        let rows = match &self.head {
            Head::Full(linear, bias) => {
                let logits = linear.forward(hidden)?.broadcast_add(bias)?.squeeze(1)?;
                candle_nn::ops::log_softmax(&logits, 1)?.to_vec2::<f32>()?
            }
            Head::Target(_, q, bias) => {
                let logits = q.forward(hidden)?.broadcast_add(bias)?.squeeze(1)?;
                candle_nn::ops::log_softmax(&logits, 1)?.to_vec2::<f32>()?
            }
        };
        Ok(rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .enumerate()
                    .map(|(i, lp)| {
                        let tok = match &self.head {
                            Head::Full(..) => i as u32,
                            Head::Target(allowed, ..) => allowed[i],
                        };
                        // bad_words_ids = [[pad]], applied after log_softmax as HF does.
                        let lp = if tok == self.config.pad_token_id {
                            f32::NEG_INFINITY
                        } else if forced_eos {
                            if tok == eos {
                                0.0
                            } else {
                                f32::NEG_INFINITY
                            }
                        } else {
                            lp
                        };
                        (tok, lp)
                    })
                    .collect()
            })
            .collect())
    }

    /// Beam search with `beams` hypotheses, as transformers' `generate`
    /// (length_penalty 1, early_stopping false, `</s>` forced at the last of
    /// `max_new` steps). More faithful than greedy decoding on short callouts.
    pub fn generate_beam(
        &mut self,
        src: &[u32],
        max_new: usize,
        beams: usize,
    ) -> Result<Vec<u32>, String> {
        let err = |e: candle_core::Error| format!("traduction : {e}");
        if beams <= 1 {
            return self.generate(src, max_new);
        }
        let input = Tensor::new(src, &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(err)?;
        self.model.encode(&input).map_err(err)?;
        let eos = self.config.eos_token_id;
        // Live hypotheses: generated tokens and summed log-probability.
        let mut live: Vec<(Vec<u32>, f32)> = vec![(Vec::new(), 0.0)];
        let mut last = vec![self.config.decoder_start_token_id];
        let mut done = Hypotheses::new(beams);
        let mut finished = false;
        for step in 0..max_new {
            let hidden = self.model.step_batch(&last).map_err(err)?;
            let lps = self.log_probs(&hidden, step + 1 == max_new).map_err(err)?;
            // The 2 x beams best continuations over all hypotheses.
            let mut cands: Vec<(f32, usize, u32)> = Vec::with_capacity(lps.len() * 2 * beams);
            for (b, row) in lps.iter().enumerate() {
                let base = live[b].1;
                let mut best: Vec<(f32, usize, u32)> = row
                    .iter()
                    .map(|&(tok, lp)| (base + lp, b, tok))
                    .filter(|c| c.0.is_finite())
                    .collect();
                let keep = (2 * beams).min(best.len());
                if keep == 0 {
                    continue;
                }
                best.select_nth_unstable_by(keep - 1, |a, b| b.0.total_cmp(&a.0));
                best.truncate(keep);
                cands.extend(best);
            }
            cands.sort_by(|a, b| b.0.total_cmp(&a.0));
            cands.truncate(2 * beams);
            // transformers counts the new token: cur_len - decoder_prompt_len.
            let generated = step + 1;
            let mut next: Vec<(Vec<u32>, f32)> = Vec::with_capacity(beams);
            let mut parents = Vec::with_capacity(beams);
            for (rank, &(score, b, tok)) in cands.iter().enumerate() {
                if tok == eos {
                    if rank < beams {
                        done.add(live[b].0.clone(), score, generated);
                    }
                } else {
                    let mut t = live[b].0.clone();
                    t.push(tok);
                    next.push((t, score));
                    parents.push(b as u32);
                }
                if next.len() == beams {
                    break;
                }
            }
            let best_score = cands.first().map_or(f32::NEG_INFINITY, |c| c.0);
            if done.is_done(best_score, generated) || next.is_empty() {
                finished = true;
                break;
            }
            self.model.reorder(&parents).map_err(err)?;
            last = next
                .iter()
                .map(|(t, _)| *t.last().expect("non-empty"))
                .collect();
            live = next;
        }
        if !finished {
            for (t, score) in live {
                let n = t.len();
                done.add(t, score, n);
            }
        }
        Ok(done.best().unwrap_or_default())
    }

    pub fn translate(&mut self, text: &str) -> Result<String, String> {
        self.translate_with(text, BEAMS)
    }

    /// `beams` = 1: greedy, for live (partial) lines that will be replaced.
    pub fn translate_with(&mut self, text: &str, beams: usize) -> Result<String, String> {
        let src = self.encode(text);
        let max_new = (src.len() * 3).clamp(16, 256);
        let ids = self.generate_beam(&src, max_new, beams)?;
        Ok(self.decode(&ids))
    }
}

#[cfg(test)]
impl Marian {
    /// Adds `delta` to one output logit (tests: makes `</s>` likelier).
    pub fn boost_logit(&mut self, token: u32, delta: f32) {
        if let Head::Full(_, bias) = &mut self.head {
            let mut b: Vec<f32> = bias.flatten_all().unwrap().to_vec1().unwrap();
            b[token as usize] += delta;
            *bias = Tensor::from_vec(b, bias.shape().clone(), bias.device()).unwrap();
        }
    }
}

/// Beam width of the final translations (generation_config of Opus-MT).
pub const BEAMS: usize = 4;

/// Finished hypotheses, best `n` by length-normalised score (HF BeamHypotheses).
struct Hypotheses {
    n: usize,
    items: Vec<(f32, Vec<u32>)>,
    worst: f32,
}

impl Hypotheses {
    fn new(n: usize) -> Self {
        Self {
            n,
            items: Vec::new(),
            worst: 1e9,
        }
    }

    fn add(&mut self, tokens: Vec<u32>, sum_logprobs: f32, generated: usize) {
        let score = sum_logprobs / generated as f32;
        if self.items.len() < self.n || score > self.worst {
            self.items.push((score, tokens));
            if self.items.len() > self.n {
                let (i, _) = self
                    .items
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1 .0.total_cmp(&b.1 .0))
                    .expect("non-empty");
                self.items.remove(i);
            }
            self.worst = self.items.iter().map(|h| h.0).fold(f32::INFINITY, f32::min);
        }
    }

    /// No live hypothesis can beat the finished ones any more.
    fn is_done(&self, best_sum_logprobs: f32, generated: usize) -> bool {
        self.items.len() >= self.n && self.worst >= best_sum_logprobs / generated as f32
    }

    fn best(self) -> Option<Vec<u32>> {
        self.items
            .into_iter()
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|h| h.1)
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
