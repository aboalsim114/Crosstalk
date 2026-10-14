//! The `crosstalk-llm` helper as seen by the app: started without a console,
//! spoken to over its stdin/stdout pipes (no network port), stopped by
//! closing stdin (it exits by itself) and killed if it does not.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Launch {
    pub exe: PathBuf,
    pub model: PathBuf,
    pub threads: u32,
    pub gpu_layers: u32,
    pub gpu_min_free_mb: u64,
    pub ctx: u32,
}

impl Launch {
    pub fn args(&self) -> Vec<String> {
        vec![
            "--model".into(),
            self.model.display().to_string(),
            "--threads".into(),
            self.threads.to_string(),
            "--gpu-layers".into(),
            self.gpu_layers.to_string(),
            "--gpu-min-free-mb".into(),
            self.gpu_min_free_mb.to_string(),
            "--ctx".into(),
            self.ctx.to_string(),
        ]
    }
}

/// First line of the helper once the model is loaded.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Ready {
    pub backend: String,
    pub device: Option<String>,
    pub gpu_layers: u32,
    pub threads: i32,
    pub load_ms: u32,
    pub model_bytes: u64,
    pub params: u64,
    pub name: Option<String>,
    pub architecture: Option<String>,
    pub gpu_skipped: Option<String>,
    #[serde(default)]
    pub rss_mb: u32,
    #[serde(default)]
    pub vram_used_mb: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Reply {
    pub id: u64,
    pub text: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub tokens: u32,
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub reused_tokens: u32,
    #[serde(default)]
    pub ms: u32,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub rss_mb: u32,
    #[serde(default)]
    pub cpu_ms: u64,
}

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum Event {
    Ready(Ready),
    Error { message: String },
}

/// Where the helper is: next to the app (installed), or given by
/// `CROSSTALK_LLM_EXE` (development, tests).
pub fn helper_path() -> PathBuf {
    if let Some(p) = std::env::var_os("CROSSTALK_LLM_EXE") {
        return PathBuf::from(p);
    }
    let name = if cfg!(windows) {
        "crosstalk-llm.exe"
    } else {
        "crosstalk-llm"
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

pub struct LlmProcess {
    child: Option<Child>,
    stdin: Option<Box<dyn Write + Send>>,
    lines: Receiver<String>,
    next_id: u64,
    pub ready: Ready,
}

impl LlmProcess {
    /// Starts the helper and waits for the model to be loaded.
    pub fn spawn(launch: &Launch, load_timeout: Duration) -> Result<Self, String> {
        if !launch.exe.is_file() {
            return Err(format!(
                "moteur introuvable ({}) : réinstallez Crosstalk",
                launch.exe.display()
            ));
        }
        let mut cmd = Command::new(&launch.exe);
        cmd.args(launch.args())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hide_console(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("démarrage du moteur impossible : {e}"))?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stdin = child.stdin.take().expect("piped stdin");
        let lines = reader(stdout);
        let mut p = Self {
            child: Some(child),
            stdin: Some(Box::new(stdin)),
            lines,
            next_id: 1,
            ready: empty_ready(),
        };
        match p.lines.recv_timeout(load_timeout) {
            Ok(line) => match serde_json::from_str::<Event>(&line) {
                Ok(Event::Ready(r)) => {
                    p.ready = r;
                    Ok(p)
                }
                Ok(Event::Error { message }) => Err(message),
                Err(e) => Err(format!("réponse du moteur illisible : {e}")),
            },
            Err(RecvTimeoutError::Timeout) => Err(format!(
                "chargement du modèle trop long (plus de {} s)",
                load_timeout.as_secs()
            )),
            Err(RecvTimeoutError::Disconnected) => {
                Err("le moteur s’est arrêté pendant le chargement".into())
            }
        }
    }

    /// From already open pipes (tests).
    pub fn from_parts(stdin: Box<dyn Write + Send>, lines: Receiver<String>, ready: Ready) -> Self {
        Self {
            child: None,
            stdin: Some(stdin),
            lines,
            next_id: 1,
            ready,
        }
    }

    /// One generation. On timeout the helper is killed (it cannot be
    /// interrupted mid-generation) and `dead()` turns true.
    pub fn request(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        timeout: Duration,
    ) -> Result<Reply, String> {
        let id = self.next_id;
        self.next_id += 1;
        let line = serde_json::json!({"id": id, "prompt": prompt, "max_tokens": max_tokens});
        let stdin = self.stdin.as_mut().ok_or("moteur arrêté")?;
        let sent = writeln!(stdin, "{line}").and_then(|_| stdin.flush());
        if sent.is_err() {
            self.kill();
            return Err("le moteur s’est arrêté".into());
        }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(l) => {
                    let r: Reply = serde_json::from_str(&l)
                        .map_err(|e| format!("réponse du moteur illisible : {e}"))?;
                    // A late answer to a request that timed out: skip it.
                    if r.id != id {
                        continue;
                    }
                    return match &r.error {
                        Some(e) => Err(e.clone()),
                        None => Ok(r),
                    };
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.kill();
                    return Err(format!(
                        "traduction trop longue (plus de {} ms), moteur redémarré",
                        timeout.as_millis()
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.kill();
                    return Err("le moteur s’est arrêté".into());
                }
            }
        }
    }

    pub fn dead(&mut self) -> bool {
        if self.stdin.is_none() {
            return true;
        }
        match self.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(Some(_))) | Some(Err(_)) => {
                self.stdin = None;
                true
            }
            _ => false,
        }
    }

    fn kill(&mut self) {
        self.stdin = None;
        if let Some(c) = self.child.as_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    /// Closes stdin (the helper exits and frees the model's memory), then
    /// kills it if it is still there after a second.
    pub fn stop(&mut self) {
        self.stdin = None;
        if let Some(c) = self.child.as_mut() {
            for _ in 0..20 {
                if matches!(c.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        self.kill();
    }
}

impl Drop for LlmProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

fn reader(stdout: impl std::io::Read + Send + 'static) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("llm-stdout".into())
        .spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        })
        .expect("llm reader thread");
    rx
}

fn empty_ready() -> Ready {
    Ready {
        backend: String::new(),
        device: None,
        gpu_layers: 0,
        threads: 0,
        load_ms: 0,
        model_bytes: 0,
        params: 0,
        name: None,
        architecture: None,
        gpu_skipped: None,
        rss_mb: 0,
        vram_used_mb: None,
    }
}

#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_: &mut Command) {}

/// Whether `path` looks like the helper (used before offering the engine).
pub fn helper_present(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// What the app wrote to the helper's stdin.
    #[derive(Clone, Default)]
    pub struct Sink(pub Arc<Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    pub fn ready() -> Ready {
        Ready {
            backend: "cpu".into(),
            threads: 4,
            load_ms: 1200,
            model_bytes: 2_489_909_760,
            ..empty_ready()
        }
    }

    #[test]
    fn sends_requests_and_matches_replies() {
        let (tx, rx) = mpsc::channel();
        let sink = Sink::default();
        let mut p = LlmProcess::from_parts(Box::new(sink.clone()), rx, ready());
        // A stale reply (id 0) is skipped.
        tx.send(r#"{"id":0,"text":"vieux"}"#.into()).unwrap();
        tx.send(r#"{"id":1,"text":"deux en B","tokens":4,"ms":310}"#.into())
            .unwrap();
        let r = p.request("PROMPT", 32, Duration::from_secs(1)).unwrap();
        assert_eq!(r.text.as_deref(), Some("deux en B"));
        assert_eq!(r.ms, 310);
        let sent = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        let v: serde_json::Value = serde_json::from_str(sent.trim()).unwrap();
        assert_eq!(v["id"], 1);
        assert_eq!(v["prompt"], "PROMPT");
        assert_eq!(v["max_tokens"], 32);

        tx.send(r#"{"id":2,"error":"texte trop long"}"#.into())
            .unwrap();
        assert_eq!(
            p.request("x", 8, Duration::from_secs(1)).unwrap_err(),
            "texte trop long"
        );
        assert!(!p.dead());
    }

    #[test]
    fn a_timeout_or_a_closed_pipe_kills_the_engine() {
        let (tx, rx) = mpsc::channel::<String>();
        let mut p = LlmProcess::from_parts(Box::new(Sink::default()), rx, ready());
        let e = p.request("x", 8, Duration::from_millis(30)).unwrap_err();
        assert!(e.contains("trop longue"), "{e}");
        assert!(p.dead());
        assert!(p.request("x", 8, Duration::from_millis(30)).is_err());
        drop(tx);

        let (tx, rx) = mpsc::channel::<String>();
        drop(tx);
        let mut p = LlmProcess::from_parts(Box::new(Sink::default()), rx, ready());
        assert!(p
            .request("x", 8, Duration::from_secs(1))
            .unwrap_err()
            .contains("arrêté"));
        assert!(p.dead());
    }

    #[test]
    fn launch_arguments() {
        let l = Launch {
            exe: "crosstalk-llm".into(),
            model: "m.gguf".into(),
            threads: 4,
            gpu_layers: 99,
            gpu_min_free_mb: 4000,
            ctx: 1024,
        };
        let a = l.args().join(" ");
        assert_eq!(
            a,
            "--model m.gguf --threads 4 --gpu-layers 99 --gpu-min-free-mb 4000 --ctx 1024"
        );
        assert!(LlmProcess::spawn(&l, Duration::from_secs(1))
            .err()
            .unwrap()
            .contains("introuvable"));
    }

    /// With the real helper and the tiny model (CROSSTALK_LLM_EXE set, see
    /// docs/reports/translategemma-integration.md): spawn, translate, stop.
    #[test]
    fn real_helper_round_trip_when_built() {
        let Some(exe) = std::env::var_os("CROSSTALK_LLM_EXE") else {
            eprintln!("CROSSTALK_LLM_EXE absent : test du vrai moteur sauté");
            return;
        };
        let model = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../src-llm/tests/data/tiny-gemma3.gguf");
        let l = Launch {
            exe: exe.into(),
            model,
            threads: 2,
            gpu_layers: 0,
            gpu_min_free_mb: 0,
            ctx: 512,
        };
        let mut p = LlmProcess::spawn(&l, Duration::from_secs(30)).unwrap();
        assert_eq!(p.ready.architecture.as_deref(), Some("gemma3"));
        let prompt = super::super::prompt::translategemma("ru", "fr", "два на Б").unwrap();
        let r = p.request(&prompt, 8, Duration::from_secs(10)).unwrap();
        assert!(r.tokens <= 8);
        p.stop();
        assert!(p.dead());
    }
}
