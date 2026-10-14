//! llama.cpp: model loaded once, one context, deterministic (greedy) decoding.
//! Consecutive prompts share their instruction prefix (same language pair):
//! its key/value cache is kept, only the new text is evaluated.

use std::num::NonZeroU32;
use std::time::Instant;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::{list_llama_ggml_backend_devices, LlamaBackendDeviceType};

use crate::protocol::{common_prefix, Args, Event, Reply, Request, MAX_TOKENS_CAP};

pub struct Engine {
    model: &'static LlamaModel,
    ctx: LlamaContext<'static>,
    /// Tokens currently in the key/value cache (positions 0..len).
    cached: Vec<LlamaToken>,
    batch: LlamaBatch<'static>,
    n_batch: usize,
    ready: Event,
}

impl Engine {
    pub fn load(args: &Args) -> Result<Self, String> {
        if !args.model.is_file() {
            return Err(format!("modèle introuvable : {}", args.model.display()));
        }
        let t = Instant::now();
        let mut backend = LlamaBackend::init().map_err(|e| format!("llama.cpp : {e}"))?;
        // llama.cpp's logs only on request (stderr is discarded by the app).
        if std::env::var_os("CROSSTALK_LLM_LOG").is_none() {
            backend.void_logs();
        }
        // The process lives as long as the model: leaking them gives the
        // context its 'static borrow.
        let backend: &'static LlamaBackend = Box::leak(Box::new(backend));
        let gpu = list_llama_ggml_backend_devices().into_iter().find(|d| {
            matches!(
                d.device_type,
                LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::IntegratedGpu
            )
        });
        let (gpu_layers, gpu_skipped) = gpu_choice(args, gpu.as_ref().map(|d| d.memory_free as u64));
        let params = LlamaModelParams::default().with_n_gpu_layers(gpu_layers);
        let model = LlamaModel::load_from_file(backend, &args.model, &params)
            .map_err(|e| format!("chargement du modèle impossible : {e}"))?;
        let model: &'static LlamaModel = Box::leak(Box::new(model));
        let n_batch = 512u32;
        let mut cp = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(args.ctx))
            .with_n_batch(n_batch);
        if args.threads > 0 {
            cp = cp
                .with_n_threads(args.threads)
                .with_n_threads_batch(args.threads);
        }
        let ctx = model
            .new_context(backend, cp)
            .map_err(|e| format!("contexte llama.cpp : {e}"))?;
        let threads = ctx_threads(args);
        let meta = |k: &str| model.meta_val_str(k).ok();
        let ready = Event::Ready {
            backend: match (&gpu, gpu_layers) {
                (Some(d), n) if n > 0 => d.backend.to_lowercase(),
                _ => "cpu".into(),
            },
            device: gpu
                .as_ref()
                .filter(|_| gpu_layers > 0)
                .map(|d| d.description.clone()),
            gpu_layers,
            threads,
            load_ms: t.elapsed().as_millis() as u32,
            model_bytes: model.size(),
            params: model.n_params(),
            ctx: args.ctx,
            name: meta("general.name"),
            architecture: meta("general.architecture"),
            gpu_skipped,
            rss_mb: rss_mb(),
        };
        Ok(Self {
            model,
            ctx,
            cached: Vec::new(),
            batch: LlamaBatch::new(n_batch as usize, 1),
            n_batch: n_batch as usize,
            ready,
        })
    }

    pub fn ready_event(&self) -> Event {
        self.ready.clone()
    }

    pub fn run(&mut self, req: &Request) -> Reply {
        let t = Instant::now();
        match self.generate(req) {
            Ok((text, tokens, prompt_tokens, reused, truncated)) => Reply {
                id: req.id,
                text: Some(text),
                error: None,
                tokens,
                prompt_tokens,
                reused_tokens: reused,
                ms: t.elapsed().as_millis() as u32,
                truncated,
                rss_mb: rss_mb(),
            },
            Err(e) => {
                // Unknown cache state after a failure: start clean next time.
                self.ctx.clear_kv_cache();
                self.cached.clear();
                Reply::error(req.id, e)
            }
        }
    }

    /// Feeds `tokens` at positions `start..`, logits for the last one only.
    fn feed(&mut self, tokens: &[LlamaToken], start: usize) -> Result<(), String> {
        for (c, chunk) in tokens.chunks(self.n_batch).enumerate() {
            self.batch.clear();
            let base = start + c * self.n_batch;
            let last_chunk = base + chunk.len() == start + tokens.len();
            for (i, &tok) in chunk.iter().enumerate() {
                let logits = last_chunk && i + 1 == chunk.len();
                self.batch
                    .add(tok, (base + i) as i32, &[0], logits)
                    .map_err(|e| format!("lot : {e}"))?;
            }
            self.ctx
                .decode(&mut self.batch)
                .map_err(|e| format!("décodage : {e}"))?;
        }
        Ok(())
    }

    fn generate(&mut self, req: &Request) -> Result<(String, u32, u32, u32, bool), String> {
        let vocab = self.model.vocab();
        // The prompt carries its own <bos> and control tokens.
        let prompt = vocab.tokenize(req.prompt.as_bytes(), false, true);
        if prompt.is_empty() {
            return Err("invite vide".into());
        }
        let max_new = req.max_tokens.clamp(1, MAX_TOKENS_CAP) as usize;
        let n_ctx = self.ctx.n_ctx() as usize;
        if prompt.len() + max_new > n_ctx {
            return Err(format!("texte trop long ({} jetons)", prompt.len()));
        }
        // Keep the shared prefix, evaluate at least the last prompt token.
        let keep = common_prefix(&self.cached, &prompt).min(prompt.len() - 1);
        self.ctx
            .clear_kv_cache_seq(Some(0), Some(keep as u32), None)
            .map_err(|e| format!("cache : {e}"))?;
        self.cached.truncate(keep);
        self.feed(&prompt[keep..], keep)?;
        self.cached.extend_from_slice(&prompt[keep..]);
        let mut out = Vec::new();
        let mut generated = 0u32;
        let mut truncated = true;
        for _ in 0..max_new {
            let next = argmax(self.ctx.get_logits_ith(self.batch.n_tokens() - 1));
            let tok = LlamaToken(next as i32);
            if vocab.is_eog(tok) {
                truncated = false;
                break;
            }
            out.extend(vocab.token_to_piece(tok, false, None));
            generated += 1;
            let pos = self.cached.len();
            self.batch.clear();
            self.batch
                .add(tok, pos as i32, &[0], true)
                .map_err(|e| format!("lot : {e}"))?;
            self.ctx
                .decode(&mut self.batch)
                .map_err(|e| format!("décodage : {e}"))?;
            self.cached.push(tok);
        }
        Ok((
            String::from_utf8_lossy(&out).trim().to_string(),
            generated,
            prompt.len() as u32,
            keep as u32,
            truncated,
        ))
    }
}

/// Layers to offload and, when the GPU is refused, why.
pub fn gpu_choice(args: &Args, gpu_free_bytes: Option<u64>) -> (u32, Option<String>) {
    if args.gpu_layers == 0 {
        return (0, None);
    }
    let Some(free) = gpu_free_bytes else {
        return (0, Some("aucune carte graphique compatible".into()));
    };
    let free_mb = free / (1024 * 1024);
    // Some drivers report no free-memory figure (0): trust the user's choice.
    if args.gpu_min_free_mb > 0 && free_mb > 0 && free_mb < args.gpu_min_free_mb {
        return (
            0,
            Some(format!(
                "mémoire vidéo libre insuffisante ({free_mb} Mo, {} Mo demandés)",
                args.gpu_min_free_mb
            )),
        );
    }
    (args.gpu_layers, None)
}

/// Resident memory of this process (MB).
#[cfg(windows)]
pub fn rss_mb() -> u32 {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_non_paged: usize,
        quota_non_paged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        ..Default::default()
    };
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok == 0 {
        0
    } else {
        (c.working_set / (1024 * 1024)) as u32
    }
}

#[cfg(not(windows))]
pub fn rss_mb() -> u32 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map_or(0, |pages| (pages * 4096 / (1024 * 1024)) as u32)
}

fn ctx_threads(args: &Args) -> i32 {
    if args.threads > 0 {
        args.threads
    } else {
        std::thread::available_parallelism().map_or(4, |n| n.get() as i32)
    }
}

/// Greedy decoding: the most likely token (translation wants no sampling).
pub fn argmax(logits: &[f32]) -> usize {
    let mut best = 0;
    for (i, &v) in logits.iter().enumerate() {
        if v > logits[best] {
            best = i;
        }
    }
    best
}

/// Below-normal priority for the whole helper: the game and Whisper first.
#[cfg(windows)]
pub fn lower_priority() {
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn SetPriorityClass(process: isize, class: u32) -> i32;
    }
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x4000;
    unsafe {
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
pub fn lower_priority() {}

#[cfg(test)]
mod tests {
    use crate::protocol::Args;

    fn args(gpu_layers: u32, min: u64) -> Args {
        Args {
            model: "m".into(),
            threads: 0,
            gpu_layers,
            ctx: 1024,
            below_normal: true,
            gpu_min_free_mb: min,
            check: false,
        }
    }

    #[test]
    fn the_gpu_is_used_only_with_enough_free_memory() {
        const GB: u64 = 1 << 30;
        assert_eq!(super::gpu_choice(&args(0, 0), Some(8 * GB)), (0, None));
        assert_eq!(super::gpu_choice(&args(99, 4500), Some(8 * GB)), (99, None));
        let (n, why) = super::gpu_choice(&args(99, 4500), Some(2 * GB));
        assert_eq!(n, 0);
        assert!(why.unwrap().contains("2048 Mo"));
        assert_eq!(super::gpu_choice(&args(99, 4500), None).0, 0);
        // No figure from the driver: the GPU is tried.
        assert_eq!(super::gpu_choice(&args(99, 4500), Some(0)).0, 99);
    }

    #[test]
    fn resident_memory_is_measured() {
        assert!(super::rss_mb() > 0);
    }

    #[test]
    fn argmax_takes_the_first_best() {
        assert_eq!(super::argmax(&[0.1, 0.9, 0.9, -1.0]), 1);
        assert_eq!(super::argmax(&[3.0]), 0);
    }
}
