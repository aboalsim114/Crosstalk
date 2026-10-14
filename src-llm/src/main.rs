//! crosstalk-llm: Crosstalk's local LLM helper (llama.cpp). The app starts it
//! when the translation engine starts and talks to it over stdin/stdout, one
//! JSON object per line: no network port, nothing reachable from outside the
//! app. It exits as soon as stdin closes (the app stopped the engine, quit or
//! crashed), so no process outlives Crosstalk.
//!
//! A separate process because whisper.cpp (in the app) and llama.cpp each
//! link their own copy of ggml, which cannot live in the same executable; it
//! also keeps a model crash or an out-of-memory error out of the app.
//!
//!   crosstalk-llm --model translategemma-4b-it.Q4_K_M.gguf [--threads 4]
//!                 [--gpu-layers 99] [--gpu-min-free-mb 4500] [--ctx 1024]
//!                 [--normal-priority] [--check]
//!
//! --check: loads the model, prints the ready line and exits (installer).
//! CROSSTALK_LLM_LOG=1 keeps llama.cpp's own logs on stderr.
//!
//! First line written: {"event":"ready",...} or {"event":"error",...}.
//! Requests: {"id":1,"prompt":"...","max_tokens":96}
//! Replies:  {"id":1,"text":"...","tokens":12,"prompt_tokens":80,"ms":345}
//!           {"id":1,"error":"..."}
//! The prompt is the model's own format, special tokens included: the app
//! builds it (src-tauri/src/translation/llm/prompt.rs), this helper does not
//! know which model it runs.

mod engine;
mod protocol;

use std::io::{BufRead, Write};

use protocol::{Args, Reply, Request};

fn main() {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            protocol::write_line(&protocol::Event::error(e));
            std::process::exit(2);
        }
    };
    if args.below_normal {
        engine::lower_priority();
    }
    let mut llm = match engine::Engine::load(&args) {
        Ok(e) => e,
        Err(e) => {
            protocol::write_line(&protocol::Event::error(e));
            std::process::exit(1);
        }
    };
    protocol::write_line(&llm.ready_event());
    if args.check {
        return;
    }
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(req) => llm.run(&req),
            Err(e) => Reply::error(0, format!("requête illisible : {e}")),
        };
        protocol::write_line(&reply);
        let _ = std::io::stdout().flush();
    }
}
