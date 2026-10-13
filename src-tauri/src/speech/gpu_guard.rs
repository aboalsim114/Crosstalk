//! Keeps a faulty graphics driver from ever stopping Crosstalk.
//!
//! The first time a build runs (and the user did not choose the CPU), the
//! graphics card is tested in a *separate process* (`crosstalk.exe --gpu-probe
//! <model>`): Vulkan initialisation, model upload and a 3 s transcription. If
//! the driver crashes or hangs there, only the probe dies; the app runs on the
//! CPU. The result is cached per build (`gpu-probe.json`).
//!
//! When the GPU is used in the app itself, `gpu-session.lock` is written and
//! removed on a clean exit: if it is still there at the next launch,
//! Crosstalk ended abruptly while using the GPU, which is then turned off.
//!
//! Turning Vulkan off means `CROSSTALK_NO_VULKAN=1` in the environment before
//! any ggml call: the loader shim (scripts/cross/vulkan_shim.c) then reports no
//! Vulkan and ggml never touches the driver.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const NO_VULKAN_ENV: &str = "CROSSTALK_NO_VULKAN";
const PROBE_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeCache {
    pub build: u32,
    pub ok: bool,
    pub gpu: Option<String>,
    pub gpu_ms: Option<u32>,
    pub cpu_ms: Option<u32>,
    /// Why the GPU is not used, in French.
    pub reason: Option<String>,
}

/// What the probe process prints on its last line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReport {
    pub gpu: Option<String>,
    pub gpu_ms: Option<u32>,
    pub cpu_ms: Option<u32>,
    pub error: Option<String>,
}

/// The decision for this run.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub allowed: bool,
    /// Shown in the engine card when the GPU is not used (French).
    pub reason: Option<String>,
}

fn state_dir() -> PathBuf {
    std::env::var_os("CROSSTALK_GPU_STATE_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Crosstalk")))
        .unwrap_or_else(std::env::temp_dir)
}

pub fn cache_path(dir: &Path) -> PathBuf {
    dir.join("gpu-probe.json")
}

pub fn marker_path(dir: &Path) -> PathBuf {
    dir.join("gpu-session.lock")
}

pub fn read_cache(dir: &Path) -> Option<ProbeCache> {
    serde_json::from_str(&std::fs::read_to_string(cache_path(dir)).ok()?).ok()
}

fn write_cache(dir: &Path, cache: &ProbeCache) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(t) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(cache_path(dir), t);
    }
}

/// Outcome of a probe process: exit code (None: killed after the timeout) and
/// its standard output.
pub fn judge_probe(build: u32, exit: Option<i32>, stdout: &str) -> ProbeCache {
    let fail = |reason: &str| ProbeCache {
        build,
        ok: false,
        gpu: None,
        gpu_ms: None,
        cpu_ms: None,
        reason: Some(reason.into()),
    };
    let Some(code) = exit else {
        return fail("le test de la carte graphique ne répond plus (pilote bloqué)");
    };
    let report = stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<ProbeReport>(l.trim()).ok());
    if code != 0 {
        return fail("le pilote graphique a planté pendant le test");
    }
    match report {
        None => fail("test de la carte graphique illisible"),
        Some(ProbeReport { error: Some(e), .. }) => {
            fail(&format!("test de la carte graphique en échec : {e}"))
        }
        Some(ProbeReport { gpu: None, .. }) => fail("aucune carte graphique Vulkan"),
        Some(r) => ProbeCache {
            build,
            ok: true,
            gpu: r.gpu,
            gpu_ms: r.gpu_ms,
            cpu_ms: r.cpu_ms,
            reason: None,
        },
    }
}

/// A stored setting, read without touching the file (the UI reports
/// unreadable files itself).
fn stored_setting(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(crate::settings::dir().join("config.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let s = v.get("settings").unwrap_or(&v);
    s.get(key).and_then(|a| a.as_str()).map(str::to_string)
}

/// The user's choice in the stored settings ("cpu", "gpu", "auto").
pub fn stored_accel() -> Option<String> {
    stored_setting("accel")
}

/// Decides once per run whether ggml may use Vulkan; `model` is a Whisper
/// model file to probe with.
pub fn decide(
    dir: &Path,
    build: u32,
    accel: Option<&str>,
    model: Option<&Path>,
    probe: impl FnOnce(&Path) -> (Option<i32>, String),
) -> Decision {
    let no = |reason: Option<String>| Decision {
        allowed: false,
        reason,
    };
    if accel == Some("cpu") {
        return no(None);
    }
    if marker_path(dir).exists() {
        let _ = std::fs::remove_file(marker_path(dir));
        let cache = ProbeCache {
            build,
            ok: false,
            gpu: None,
            gpu_ms: None,
            cpu_ms: None,
            reason: Some(
                "Crosstalk s'est fermé brutalement en utilisant la carte graphique".into(),
            ),
        };
        write_cache(dir, &cache);
        crate::logs::warn(
            "gpu",
            "fermeture brutale avec la carte graphique : processeur utilisé",
        );
        return no(cache.reason);
    }
    let cache = match read_cache(dir).filter(|c| c.build == build) {
        Some(c) => c,
        None => {
            let Some(model) = model else {
                return no(Some(
                    "carte graphique testée dès qu'un modèle sera installé".into(),
                ));
            };
            let (exit, out) = probe(model);
            let c = judge_probe(build, exit, &out);
            crate::logs::info(
                "gpu",
                &match (&c.gpu, c.reason.as_deref()) {
                    (Some(g), _) => format!(
                        "test de la carte graphique réussi : {g} (GPU {:?} ms, CPU {:?} ms)",
                        c.gpu_ms, c.cpu_ms
                    ),
                    (None, Some(r)) => format!("carte graphique non utilisée : {r}"),
                    _ => "carte graphique non utilisée".into(),
                },
            );
            write_cache(dir, &c);
            c
        }
    };
    if !cache.ok {
        return no(cache.reason);
    }
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(marker_path(dir), b"gpu");
    Decision {
        allowed: true,
        reason: None,
    }
}

/// Runs `crosstalk.exe --gpu-probe <model>` and waits for it (bounded).
fn run_probe(model: &Path) -> (Option<i32>, String) {
    let Ok(exe) = std::env::current_exe() else {
        return (Some(1), String::new());
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--gpu-probe")
        .arg(model)
        .env_remove(NO_VULKAN_ENV)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let Ok(mut child) = cmd.spawn() else {
        return (Some(1), String::new());
    };
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(out) = stdout.as_mut() {
            use std::io::Read;
            let _ = out.read_to_string(&mut s);
        }
        s
    });
    let started = Instant::now();
    let exit = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status.code().unwrap_or(1)),
            Ok(None) if started.elapsed() < PROBE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(100))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    (exit, reader.join().unwrap_or_default())
}

static DECISION: OnceLock<Decision> = OnceLock::new();
static STATE_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Once per run, before the first ggml call: decides and, when the GPU is not
/// allowed, turns Vulkan off for this process.
pub fn ensure(models_dir: &Path) -> &'static Decision {
    DECISION.get_or_init(|| {
        if !cfg!(feature = "gpu") {
            return Decision {
                allowed: false,
                reason: None,
            };
        }
        let dir = state_dir();
        *STATE_DIR.lock().unwrap() = Some(dir.clone());
        let accel = stored_accel();
        let model = probe_model(models_dir, accel.as_deref());
        let d = decide(
            &dir,
            crate::updater::current_build(),
            accel.as_deref(),
            model.as_deref(),
            run_probe,
        );
        if !d.allowed {
            std::env::set_var(NO_VULKAN_ENV, "1");
        }
        d
    })
}

pub fn decision() -> Option<&'static Decision> {
    DECISION.get()
}

/// A model to probe with: the chosen one if installed, else the smallest.
fn probe_model(models_dir: &Path, _accel: Option<&str>) -> Option<PathBuf> {
    stored_setting("model")
        .iter()
        .map(String::as_str)
        .chain(["tiny", "base", "small"])
        .find_map(|m| super::models::model_path(models_dir, m))
}

/// Clean exit: the GPU session ended normally.
pub fn clean_exit() {
    if let Some(dir) = STATE_DIR.lock().unwrap().as_ref() {
        let _ = std::fs::remove_file(marker_path(dir));
    }
}

/// `crosstalk.exe --gpu-probe <model>`: tests the GPU and prints a
/// `ProbeReport` line. A driver crash here only ends this process.
pub fn probe_main(model: Option<&str>) -> i32 {
    let report = probe_report(model);
    println!("{}", serde_json::to_string(&report).unwrap_or_default());
    0
}

#[cfg(feature = "whisper")]
fn probe_report(model: Option<&str>) -> ProbeReport {
    use super::engine::{vulkan_gpu, WhisperRecognizer};
    let err = |e: String| ProbeReport {
        gpu: None,
        gpu_ms: None,
        cpu_ms: None,
        error: Some(e),
    };
    let Some(path) = model.map(Path::new) else {
        return err("modèle manquant".into());
    };
    let Some(name) = vulkan_gpu() else {
        return ProbeReport {
            gpu: None,
            gpu_ms: None,
            cpu_ms: None,
            error: None,
        };
    };
    let gpu_ms = match WhisperRecognizer::load(path, true).and_then(|mut r| r.bench_ms()) {
        Ok(ms) => ms,
        Err(e) => return err(e.to_string()),
    };
    let cpu_ms = WhisperRecognizer::load(path, false)
        .and_then(|mut r| r.bench_ms())
        .ok();
    ProbeReport {
        gpu: Some(name),
        gpu_ms: Some(gpu_ms),
        cpu_ms,
        error: None,
    }
}

#[cfg(not(feature = "whisper"))]
fn probe_report(_model: Option<&str>) -> ProbeReport {
    ProbeReport {
        gpu: None,
        gpu_ms: None,
        cpu_ms: None,
        error: Some("build sans whisper.cpp".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crosstalk-gpu-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const OK: &str = r#"{"gpu":"NVIDIA GeForce RTX 3060","gpuMs":180,"cpuMs":900,"error":null}"#;

    #[test]
    fn a_crashing_or_hanging_probe_turns_the_gpu_off() {
        let c = judge_probe(7, Some(-1073741819), ""); // 0xC0000005
        assert!(!c.ok && c.reason.unwrap().contains("planté"));
        assert!(judge_probe(7, None, "").reason.unwrap().contains("bloqué"));
        assert!(!judge_probe(7, Some(0), "garbage").ok);
        let c = judge_probe(
            7,
            Some(0),
            r#"{"gpu":null,"gpuMs":null,"cpuMs":null,"error":null}"#,
        );
        assert_eq!(c.reason.as_deref(), Some("aucune carte graphique Vulkan"));
        let c = judge_probe(7, Some(0), &format!("ggml log line\n{OK}"));
        assert!(c.ok);
        assert_eq!(c.gpu.as_deref(), Some("NVIDIA GeForce RTX 3060"));
    }

    #[test]
    fn probes_once_per_build_and_caches_the_result() {
        let d = dir("cache");
        let model = d.join("ggml-base.bin");
        let mut runs = 0;
        let r = decide(&d, 10, Some("auto"), Some(&model), |_| {
            runs += 1;
            (Some(0), OK.into())
        });
        assert!(r.allowed);
        assert!(marker_path(&d).exists(), "GPU session marked");
        clean_exit_in(&d);
        // Same build: no new probe.
        let r = decide(&d, 10, Some("auto"), Some(&model), |_| {
            panic!("probed again")
        });
        assert!(r.allowed);
        clean_exit_in(&d);
        // New build: probed again; a crash turns the GPU off.
        let r = decide(&d, 11, Some("auto"), Some(&model), |_| {
            (Some(3), String::new())
        });
        assert!(!r.allowed && !marker_path(&d).exists());
        assert_eq!(runs, 1);
        assert!(!read_cache(&d).unwrap().ok);
    }

    #[test]
    fn an_abrupt_end_with_the_gpu_turns_it_off() {
        let d = dir("marker");
        let model = d.join("ggml-base.bin");
        assert!(decide(&d, 5, None, Some(&model), |_| (Some(0), OK.into())).allowed);
        // No clean exit: the marker is still there at the next launch.
        let r = decide(&d, 5, None, Some(&model), |_| panic!("not probed"));
        assert!(!r.allowed);
        assert!(r.reason.unwrap().contains("brutalement"));
        // And it stays off for this build.
        assert!(!decide(&d, 5, None, Some(&model), |_| panic!("not probed")).allowed);
    }

    #[test]
    fn cpu_choice_or_no_model_never_probes() {
        let d = dir("cpu");
        assert!(!decide(&d, 1, Some("cpu"), None, |_| panic!("probed")).allowed);
        let r = decide(&d, 1, Some("auto"), None, |_| panic!("probed"));
        assert!(!r.allowed && r.reason.unwrap().contains("modèle"));
        assert!(read_cache(&d).is_none(), "nothing cached without a probe");
    }

    fn clean_exit_in(d: &Path) {
        let _ = std::fs::remove_file(marker_path(d));
    }
}
