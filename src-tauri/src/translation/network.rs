//! The Marian transformer, adapted from candle-transformers' `models::marian`
//! (Copyright The candle authors, MIT OR Apache-2.0) for incremental greedy
//! decoding of one sentence at a time:
//! - the cross-attention keys/values of the encoded sentence are computed once
//!   per sentence instead of at every generated token (the largest per-token
//!   cost on short callouts);
//! - the encoder keeps no key/value cache between sentences.
//!
//! Weight names and maths are those of transformers' MarianMTModel.

use candle_core::{Result, Tensor};
use candle_nn::{embedding, layer_norm, linear, Embedding, LayerNorm, Linear, VarBuilder};
use candle_transformers::models::marian::Config;

fn sinusoids(cfg: &Config, vb: &VarBuilder) -> Result<Tensor> {
    let dev = vb.device();
    let dim = cfg.d_model;
    let inv_freq: Vec<f32> = (0..dim)
        .step_by(2)
        .map(|i| 1f32 / 10000f32.powf(i as f32 / dim as f32))
        .collect();
    let half = inv_freq.len();
    let inv_freq = Tensor::from_vec(inv_freq, (1, half), dev)?;
    let n = cfg.max_position_embeddings;
    let t = Tensor::arange(0u32, n as u32, dev)?
        .to_dtype(candle_core::DType::F32)?
        .reshape((n, 1))?;
    let freqs = t.matmul(&inv_freq)?;
    Tensor::cat(&[&freqs.sin()?, &freqs.cos()?], 1)?.contiguous()
}

struct Attention {
    q: Linear,
    k: Linear,
    v: Linear,
    out: Linear,
    heads: usize,
    head_dim: usize,
    scaling: f64,
    /// Self-attention: keys/values of the tokens generated so far.
    /// Cross-attention: keys/values of the encoded sentence.
    cache: Option<(Tensor, Tensor)>,
}

impl Attention {
    fn new(cfg: &Config, heads: usize, vb: VarBuilder) -> Result<Self> {
        let d = cfg.d_model;
        let head_dim = d / heads;
        Ok(Self {
            q: linear(d, d, vb.pp("q_proj"))?,
            k: linear(d, d, vb.pp("k_proj"))?,
            v: linear(d, d, vb.pp("v_proj"))?,
            out: linear(d, d, vb.pp("out_proj"))?,
            heads,
            head_dim,
            scaling: (head_dim as f64).powf(-0.5),
            cache: None,
        })
    }

    /// (b, t, d) → (b, heads, t, head_dim)
    fn split(&self, x: &Tensor) -> Result<Tensor> {
        let (b, t, _) = x.dims3()?;
        x.reshape((b, t, self.heads, self.head_dim))?
            .transpose(1, 2)?
            .contiguous()
    }

    fn attend(&self, q: &Tensor, k: &Tensor, v: &Tensor, mask: Option<&Tensor>) -> Result<Tensor> {
        let (b, _, t, _) = q.dims4()?;
        let w = q.matmul(&k.transpose(2, 3)?.contiguous()?)?;
        let w = match mask {
            Some(m) => w.broadcast_add(m)?,
            None => w,
        };
        let p = candle_nn::ops::softmax_last_dim(&w)?;
        p.matmul(v)?
            .transpose(1, 2)?
            .reshape((b, t, self.heads * self.head_dim))?
            .apply(&self.out)
    }

    /// Encoder self-attention over the whole sentence (no cache).
    fn full(&self, x: &Tensor) -> Result<Tensor> {
        let q = self.split(&(x.apply(&self.q)? * self.scaling)?)?;
        let k = self.split(&x.apply(&self.k)?)?;
        let v = self.split(&x.apply(&self.v)?)?;
        self.attend(&q, &k, &v, None)
    }

    /// Decoder self-attention: appends the new token's keys/values.
    fn incremental(&mut self, x: &Tensor) -> Result<Tensor> {
        let q = self.split(&(x.apply(&self.q)? * self.scaling)?)?;
        let k = self.split(&x.apply(&self.k)?)?;
        let v = self.split(&x.apply(&self.v)?)?;
        let (k, v) = match &self.cache {
            Some((pk, pv)) => (Tensor::cat(&[pk, &k], 2)?, Tensor::cat(&[pv, &v], 2)?),
            None => (k, v),
        };
        let y = self.attend(&q, &k, &v, None)?;
        self.cache = Some((k, v));
        Ok(y)
    }

    /// Decoder cross-attention: keys/values of the encoded sentence, computed once.
    fn cross(&mut self, x: &Tensor, encoded: &Tensor) -> Result<Tensor> {
        if self.cache.is_none() {
            self.cache = Some((
                self.split(&encoded.apply(&self.k)?)?,
                self.split(&encoded.apply(&self.v)?)?,
            ));
        }
        let (k, v) = self.cache.as_ref().expect("filled above");
        let q = self.split(&(x.apply(&self.q)? * self.scaling)?)?;
        self.attend(&q, k, v, None)
    }
}

struct Feed {
    fc1: Linear,
    fc2: Linear,
    act: candle_nn::Activation,
    norm: LayerNorm,
}

impl Feed {
    fn new(cfg: &Config, ffn: usize, vb: &VarBuilder) -> Result<Self> {
        Ok(Self {
            fc1: linear(cfg.d_model, ffn, vb.pp("fc1"))?,
            fc2: linear(ffn, cfg.d_model, vb.pp("fc2"))?,
            act: cfg.activation_function,
            norm: layer_norm(cfg.d_model, 1e-5, vb.pp("final_layer_norm"))?,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let y = x.apply(&self.fc1)?.apply(&self.act)?.apply(&self.fc2)?;
        (y + x)?.apply(&self.norm)
    }
}

struct EncoderLayer {
    attn: Attention,
    attn_norm: LayerNorm,
    feed: Feed,
}

struct DecoderLayer {
    attn: Attention,
    attn_norm: LayerNorm,
    cross: Attention,
    cross_norm: LayerNorm,
    feed: Feed,
}

pub struct Network {
    shared: Embedding,
    positions: Tensor,
    scale: f64,
    encoder: Vec<EncoderLayer>,
    decoder: Vec<DecoderLayer>,
    encoded: Option<Tensor>,
    step: usize,
}

impl Network {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let m = vb.pp("model");
        let shared = embedding(cfg.vocab_size, cfg.d_model, m.pp("shared"))?;
        let mut encoder = Vec::with_capacity(cfg.encoder_layers);
        for i in 0..cfg.encoder_layers {
            let l = m.pp("encoder").pp("layers").pp(i);
            encoder.push(EncoderLayer {
                attn: Attention::new(cfg, cfg.encoder_attention_heads, l.pp("self_attn"))?,
                attn_norm: layer_norm(cfg.d_model, 1e-5, l.pp("self_attn_layer_norm"))?,
                feed: Feed::new(cfg, cfg.encoder_ffn_dim, &l)?,
            });
        }
        let mut decoder = Vec::with_capacity(cfg.decoder_layers);
        for i in 0..cfg.decoder_layers {
            let l = m.pp("decoder").pp("layers").pp(i);
            decoder.push(DecoderLayer {
                attn: Attention::new(cfg, cfg.decoder_attention_heads, l.pp("self_attn"))?,
                attn_norm: layer_norm(cfg.d_model, 1e-5, l.pp("self_attn_layer_norm"))?,
                cross: Attention::new(cfg, cfg.decoder_attention_heads, l.pp("encoder_attn"))?,
                cross_norm: layer_norm(cfg.d_model, 1e-5, l.pp("encoder_attn_layer_norm"))?,
                feed: Feed::new(cfg, cfg.decoder_ffn_dim, &l)?,
            });
        }
        Ok(Self {
            shared,
            positions: sinusoids(cfg, &vb)?,
            scale: if cfg.scale_embedding {
                (cfg.d_model as f64).sqrt()
            } else {
                1.0
            },
            encoder,
            decoder,
            encoded: None,
            step: 0,
        })
    }

    pub fn shared(&self) -> &Tensor {
        self.shared.embeddings()
    }

    fn embed(&self, ids: &Tensor, first: usize) -> Result<Tensor> {
        let t = ids.dim(1)?;
        let x = (ids.apply(&self.shared)? * self.scale)?;
        x.broadcast_add(&self.positions.narrow(0, first, t)?.unsqueeze(0)?)
    }

    /// Encodes a sentence (1 × n ids) and resets the decoder for it.
    pub fn encode(&mut self, ids: &Tensor) -> Result<()> {
        let mut x = self.embed(ids, 0)?;
        for l in &self.encoder {
            x = (l.attn.full(&x)? + &x)?.apply(&l.attn_norm)?;
            x = l.feed.forward(&x)?;
        }
        for l in &mut self.decoder {
            l.attn.cache = None;
            l.cross.cache = None;
        }
        self.encoded = Some(x);
        self.step = 0;
        Ok(())
    }

    /// Decoder state (1 × 1 × d) after feeding the next token.
    pub fn step(&mut self, token: u32) -> Result<Tensor> {
        self.step_batch(&[token])
    }

    /// Decoder states (b × 1 × d) after feeding one token per hypothesis
    /// (beam search). The caches must hold `b` rows: see [`Network::reorder`].
    pub fn step_batch(&mut self, tokens: &[u32]) -> Result<Tensor> {
        let encoded = self.encoded.clone().expect("encode() before step()");
        let ids = Tensor::new(tokens, encoded.device())?.unsqueeze(1)?;
        let mut x = self.embed(&ids, self.step)?;
        for l in &mut self.decoder {
            x = (l.attn.incremental(&x)? + &x)?.apply(&l.attn_norm)?;
            x = (l.cross.cross(&x, &encoded)? + &x)?.apply(&l.cross_norm)?;
            x = l.feed.forward(&x)?;
        }
        self.step += 1;
        Ok(x)
    }

    /// Keeps, for each new hypothesis, the cached keys/values of its parent
    /// (row `parents[i]`): beams are pruned and duplicated between steps.
    pub fn reorder(&mut self, parents: &[u32]) -> Result<()> {
        let Some(device) = self.encoded.as_ref().map(|e| e.device().clone()) else {
            return Ok(());
        };
        let idx = Tensor::new(parents, &device)?;
        for l in &mut self.decoder {
            for a in [&mut l.attn, &mut l.cross] {
                if let Some((k, v)) = &a.cache {
                    a.cache = Some((k.index_select(&idx, 0)?, v.index_select(&idx, 0)?));
                }
            }
        }
        Ok(())
    }
}
