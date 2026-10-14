//! Runs the real crosstalk-llm binary on tiny Gemma 3 models with random
//! weights (scripts/make-tiny-gemma3-gguf.py): llama.cpp loads the gemma3
//! architecture, parses TranslateGemma's control tokens, generates greedily,
//! stops on <end_of_turn>, reuses the shared prompt prefix and exits when
//! stdin closes. The generated text itself is meaningless here.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

struct Helper {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Helper {
    fn start(model: &str, extra: &[&str]) -> (Self, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_crosstalk-llm"))
            .arg("--model")
            .arg(data(model))
            .args(["--threads", "2", "--ctx", "512"])
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("helper");
        let stdin = child.stdin.take();
        let mut h = Helper {
            stdout: BufReader::new(child.stdout.take().unwrap()),
            stdin,
            child,
        };
        let ready = h.line();
        (h, ready)
    }

    fn line(&mut self) -> Value {
        let mut s = String::new();
        self.stdout.read_line(&mut s).unwrap();
        serde_json::from_str(&s).unwrap_or_else(|e| panic!("{e}: {s:?}"))
    }

    fn ask(&mut self, req: &str) -> Value {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(req.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
        self.line()
    }
}

const PROMPT: &str = "<bos><start_of_turn>user\nYou are a professional Russian (ru) to French (fr) translator. Your goal is to accurately convey the meaning and nuances of the original Russian text while adhering to French grammar, vocabulary, and cultural sensitivities.\nProduce only the French translation, without any additional explanations or commentary. Please translate the following Russian text into French:\n\n\n{}<end_of_turn>\n<start_of_turn>model\n";

fn request(id: u64, text: &str, max: u32) -> String {
    serde_json::json!({"id": id, "prompt": PROMPT.replace("{}", text), "max_tokens": max})
        .to_string()
}

#[test]
fn loads_generates_reuses_the_prefix_and_exits_with_stdin() {
    let (mut h, ready) = Helper::start("tiny-gemma3.gguf", &[]);
    assert_eq!(ready["event"], "ready", "{ready}");
    assert_eq!(ready["backend"], "cpu");
    assert_eq!(ready["architecture"], "gemma3");
    assert_eq!(ready["name"], "tiny-gemma3-test");
    assert_eq!(ready["threads"], 2);

    let a = h.ask(&request(1, "два на Б", 8));
    assert_eq!(a["id"], 1, "{a}");
    assert!(a["error"].is_null(), "{a}");
    assert!(a["tokens"].as_u64().unwrap() <= 8);
    assert_eq!(a["reused_tokens"], 0);
    let prompt_tokens = a["prompt_tokens"].as_u64().unwrap();
    // Control tokens are single tokens, not spelled out byte by byte.
    assert!(prompt_tokens < PROMPT.len() as u64, "{prompt_tokens}");

    // Same instruction, other text: the instruction's cache is reused.
    let b = h.ask(&request(2, "раш Б", 8));
    assert!(b["reused_tokens"].as_u64().unwrap() > prompt_tokens / 2, "{b}");
    // Deterministic: the same request gives the same output.
    let a2 = h.ask(&request(3, "два на Б", 8));
    assert_eq!(a2["text"], a["text"]);
    assert_eq!(a2["tokens"], a["tokens"]);

    let bad = h.ask("not json");
    assert!(bad["error"].as_str().unwrap().contains("illisible"));
    let long = h.ask(&request(4, &"слово ".repeat(400), 8));
    assert!(long["error"].as_str().unwrap().contains("trop long"), "{long}");
    // Still usable after an error.
    assert!(h.ask(&request(5, "да", 4))["error"].is_null());

    drop(h.stdin.take());
    let t = Instant::now();
    loop {
        if let Some(status) = h.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "still running");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn stops_on_end_of_turn() {
    let (mut h, ready) = Helper::start("tiny-gemma3-stop.gguf", &[]);
    assert_eq!(ready["event"], "ready", "{ready}");
    let r = h.ask(&request(1, "go", 32));
    assert_eq!(r["tokens"], 0, "{r}");
    assert_eq!(r["text"], "");
    assert_eq!(r["truncated"], false);
}

#[test]
fn reports_a_missing_model_and_bad_arguments() {
    let (mut h, ev) = Helper::start("absent.gguf", &[]);
    assert_eq!(ev["event"], "error");
    assert!(ev["message"].as_str().unwrap().contains("introuvable"));
    assert!(!h.child.wait().unwrap().success());
    let (mut h, ev) = Helper::start("tiny-gemma3.gguf", &["--bogus"]);
    assert_eq!(ev["event"], "error");
    assert!(!h.child.wait().unwrap().success());
}

#[test]
fn falls_back_to_the_cpu_without_a_gpu() {
    let (_h, ready) = Helper::start("tiny-gemma3.gguf", &["--gpu-layers", "99"]);
    assert_eq!(ready["event"], "ready", "{ready}");
    // CPU-only build or no GPU here: the helper says why instead of failing.
    if ready["backend"] == "cpu" {
        assert!(ready["gpu_skipped"].is_string(), "{ready}");
        assert_eq!(ready["gpu_layers"], 0);
    }
}

#[test]
fn check_mode_loads_and_exits() {
    let (mut h, ready) = Helper::start("tiny-gemma3.gguf", &["--check"]);
    assert_eq!(ready["event"], "ready", "{ready}");
    assert!(h.child.wait().unwrap().success());
}
