//! Piper, kept running for the chosen voice: one JSON line per sentence on
//! stdin (`--json-input`), Piper writes the WAV and prints its path. The model
//! loads once (about a second); a sentence then takes a few hundred ms on the
//! CPU. Nothing goes on the network.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

use super::wav::{self, Pcm};

/// Longest wait for one sentence (model load included).
const SENTENCE_TIMEOUT: Duration = Duration::from_secs(20);
/// A reply is one sentence: longer text is cut.
pub const MAX_CHARS: usize = 300;

/// One line of `--json-input`.
pub fn request_line(text: &str, output: &Path) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_CHARS)
        .collect();
    serde_json::json!({ "text": text.trim(), "output_file": output.to_string_lossy() }).to_string()
}

struct Process {
    voice: PathBuf,
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Process {
    fn spawn(exe: &Path, model: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(exe);
        cmd.arg("-m")
            .arg(model)
            .arg("--json-input")
            .arg("-q")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(dir) = exe.parent() {
            cmd.current_dir(dir); // espeak-ng-data sits next to piper.exe
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("moteur de voix introuvable : {e}"))?;
        let stdin = child.stdin.take().ok_or("moteur de voix : entrée fermée")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("moteur de voix : sortie fermée")?;
        let (tx, lines) = channel();
        std::thread::Builder::new()
            .name("piper-out".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            voice: model.to_path_buf(),
            child,
            stdin,
            lines,
        })
    }

    fn say(&mut self, text: &str, out: &Path) -> Result<Pcm, String> {
        let _ = std::fs::remove_file(out);
        writeln!(self.stdin, "{}", request_line(text, out))
            .and_then(|_| self.stdin.flush())
            .map_err(|_| "le moteur de voix s'est arrêté".to_string())?;
        match self.lines.recv_timeout(SENTENCE_TIMEOUT) {
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                return Err("le moteur de voix ne répond pas".into())
            }
            Err(_) => return Err("le moteur de voix s'est arrêté".into()),
        }
        let bytes = std::fs::read(out).map_err(|_| "aucun son produit".to_string())?;
        let _ = std::fs::remove_file(out);
        wav::parse(&bytes)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The running voice; started on the first sentence, replaced when the voice changes.
pub struct Piper {
    exe: PathBuf,
    work: PathBuf,
    current: Mutex<Option<Process>>,
}

impl Piper {
    pub fn new(exe: PathBuf, work: PathBuf) -> Self {
        Self {
            exe,
            work,
            current: Mutex::new(None),
        }
    }

    /// Speaks `text` with the voice model `model`.
    pub fn say(&self, model: &Path, text: &str) -> Result<Pcm, String> {
        if text.trim().is_empty() {
            return Err("rien à dire".into());
        }
        std::fs::create_dir_all(&self.work).map_err(|e| e.to_string())?;
        let out = self.work.join("phrase.wav");
        let mut cur = self.current.lock().unwrap();
        if cur.as_ref().is_some_and(|p| p.voice != model) {
            *cur = None;
        }
        if cur.is_none() {
            *cur = Some(Process::spawn(&self.exe, model)?);
        }
        let result = cur.as_mut().unwrap().say(text, &out);
        if result.is_err() {
            // A dead or stuck process is restarted on the next sentence.
            *cur = None;
        }
        result
    }

    /// Stops the process (voice turned off).
    pub fn stop(&self) {
        *self.current.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_json_line_per_sentence_without_control_characters() {
        let line = request_line("Двое на B\nосторожно \"флешка\"", Path::new("C:\\t\\a.wav"));
        assert!(!line.contains('\n'));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["text"], "Двое на B осторожно \"флешка\"");
        assert_eq!(v["output_file"], "C:\\t\\a.wav");
        let long = request_line(&"а".repeat(1000), Path::new("x.wav"));
        let v: serde_json::Value = serde_json::from_str(&long).unwrap();
        assert_eq!(v["text"].as_str().unwrap().chars().count(), MAX_CHARS);
    }

    #[test]
    fn a_missing_engine_is_a_clear_error() {
        let p = Piper::new(
            "/nonexistent/piper".into(),
            std::env::temp_dir().join("crosstalk-piper-none"),
        );
        let e = p
            .say(Path::new("/nonexistent/v.onnx"), "привет")
            .unwrap_err();
        assert!(e.contains("moteur de voix"), "{e}");
        assert_eq!(p.say(Path::new("v.onnx"), "  ").unwrap_err(), "rien à dire");
    }

    /// Real synthesis, when a Piper build and a voice are given:
    /// `CROSSTALK_PIPER=/path/piper CROSSTALK_PIPER_VOICE=/path/voice.onnx`.
    #[test]
    fn real_piper_speaks_two_sentences_with_one_process() {
        let (Some(exe), Some(model)) = (
            std::env::var_os("CROSSTALK_PIPER"),
            std::env::var_os("CROSSTALK_PIPER_VOICE"),
        ) else {
            eprintln!("CROSSTALK_PIPER non défini : test réel ignoré");
            return;
        };
        let work = std::env::temp_dir().join(format!("crosstalk-piper-{}", std::process::id()));
        let p = Piper::new(exe.into(), work.clone());
        let model = PathBuf::from(model);
        let t = std::time::Instant::now();
        let a = p.say(&model, "Двое на банане, осторожно флешка.").unwrap();
        let first = t.elapsed();
        let t = std::time::Instant::now();
        let b = p.say(&model, "Бомба на А.").unwrap();
        let second = t.elapsed();
        eprintln!("première phrase {first:?} (chargement compris), seconde {second:?}");
        assert!(a.rate >= 16_000 && a.samples.len() > a.rate as usize);
        assert!(b.samples.len() > b.rate as usize / 2);
        assert!(a.samples.iter().any(|s| s.abs() > 0.05), "silence");
        let _ = std::fs::remove_dir_all(work);
    }
}
