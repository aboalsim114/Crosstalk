//! Command line and JSON-lines messages between the app and the helper.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Longest generation accepted, whatever the app asks (a callout is short).
pub const MAX_TOKENS_CAP: u32 = 256;

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub model: PathBuf,
    /// Generation threads; 0: llama.cpp's default.
    pub threads: i32,
    /// Layers on the GPU (0: CPU only, 99: all).
    pub gpu_layers: u32,
    pub ctx: u32,
    /// Below-normal priority (default): CS2 and Whisper go first.
    pub below_normal: bool,
    /// GPU used only with at least this much free video memory (MB): CS2
    /// and Whisper need theirs. 0: no check.
    pub gpu_min_free_mb: u64,
    /// Load the model, print the ready line and exit (installer check).
    pub check: bool,
}

impl Args {
    pub fn parse(mut it: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut a = Args {
            model: PathBuf::new(),
            threads: 0,
            gpu_layers: 0,
            ctx: 1024,
            below_normal: true,
            gpu_min_free_mb: 0,
            check: false,
        };
        while let Some(k) = it.next() {
            let mut val = |name: &str| it.next().ok_or(format!("{name} : valeur manquante"));
            match k.as_str() {
                "--model" => a.model = PathBuf::from(val("--model")?),
                "--threads" => a.threads = num(&val("--threads")?, "--threads")?,
                "--gpu-layers" => a.gpu_layers = num(&val("--gpu-layers")?, "--gpu-layers")?,
                "--ctx" => a.ctx = num(&val("--ctx")?, "--ctx")?,
                "--normal-priority" => a.below_normal = false,
                "--check" => a.check = true,
                "--gpu-min-free-mb" => {
                    a.gpu_min_free_mb = num(&val("--gpu-min-free-mb")?, "--gpu-min-free-mb")?
                }
                other => return Err(format!("option inconnue : {other}")),
            }
        }
        if a.model.as_os_str().is_empty() {
            return Err("--model manquant".into());
        }
        if !(256..=8192).contains(&a.ctx) {
            return Err("--ctx hors limites (256 à 8192)".into());
        }
        Ok(a)
    }
}

fn num<T: std::str::FromStr>(v: &str, name: &str) -> Result<T, String> {
    v.parse().map_err(|_| format!("{name} : nombre attendu"))
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Request {
    pub id: u64,
    pub prompt: String,
    #[serde(default = "default_max")]
    pub max_tokens: u32,
}

fn default_max() -> u32 {
    96
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Reply {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub tokens: u32,
    pub prompt_tokens: u32,
    /// Prompt tokens reused from the previous request (same instruction).
    pub reused_tokens: u32,
    pub ms: u32,
    /// Stopped by the token limit rather than by the end of the answer.
    pub truncated: bool,
    /// Resident memory of the helper after the request (MB).
    pub rss_mb: u32,
}

impl Reply {
    pub fn error(id: u64, e: impl Into<String>) -> Self {
        Reply {
            id,
            text: None,
            error: Some(e.into()),
            tokens: 0,
            prompt_tokens: 0,
            reused_tokens: 0,
            ms: 0,
            truncated: false,
            rss_mb: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Ready {
        /// "cpu" or the GPU backend ("vulkan").
        backend: String,
        /// GPU name when layers are offloaded.
        device: Option<String>,
        gpu_layers: u32,
        threads: i32,
        load_ms: u32,
        model_bytes: u64,
        params: u64,
        ctx: u32,
        /// `general.name` of the GGUF, for the logs.
        name: Option<String>,
        architecture: Option<String>,
        /// Why the GPU was not used although asked for.
        gpu_skipped: Option<String>,
        rss_mb: u32,
    },
    Error {
        message: String,
    },
}

impl Event {
    pub fn error(message: impl Into<String>) -> Self {
        Event::Error {
            message: message.into(),
        }
    }
}

pub fn write_line<T: Serialize>(msg: &T) {
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, msg);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// Longest prefix shared by two token lists.
pub fn common_prefix<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Result<Args, String> {
        Args::parse(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_the_command_line() {
        let a = args(&["--model", "m.gguf", "--threads", "4", "--gpu-layers", "99"]).unwrap();
        assert_eq!(a.model, PathBuf::from("m.gguf"));
        assert_eq!((a.threads, a.gpu_layers, a.ctx), (4, 99, 1024));
        assert!(a.below_normal);
        assert!(!args(&["--model", "m", "--normal-priority"]).unwrap().below_normal);
        assert!(args(&["--model", "m", "--check"]).unwrap().check);
        assert!(args(&[]).is_err());
        assert!(args(&["--model", "m", "--ctx", "10"]).is_err());
        assert!(args(&["--model", "m", "--threads"]).is_err());
        assert!(args(&["--model", "m", "--bogus"]).is_err());
        let g = args(&["--model", "m", "--gpu-min-free-mb", "4500"]).unwrap();
        assert_eq!(g.gpu_min_free_mb, 4500);
    }

    #[test]
    fn messages_round_trip() {
        let r: Request = serde_json::from_str(r#"{"id":3,"prompt":"x"}"#).unwrap();
        assert_eq!(r.max_tokens, 96);
        let e = serde_json::to_string(&Event::error("boom")).unwrap();
        assert_eq!(e, r#"{"event":"error","message":"boom"}"#);
        let j = serde_json::to_string(&Reply::error(7, "x")).unwrap();
        assert!(j.contains(r#""id":7"#) && !j.contains("text"));
    }

    #[test]
    fn common_prefix_of_token_lists() {
        assert_eq!(common_prefix(&[1, 2, 3], &[1, 2, 4, 5]), 2);
        assert_eq!(common_prefix::<i32>(&[], &[1]), 0);
        assert_eq!(common_prefix(&[1, 2], &[1, 2]), 2);
    }
}
