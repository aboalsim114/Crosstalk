//! `crosstalk-audio-probe`: diagnostic tool for the audio capture, without the UI.
//!
//! ```text
//! crosstalk-audio-probe --list
//! crosstalk-audio-probe [--mode auto|process|device] [--device <id>] [--seconds 10] [--wav out.wav]
//! ```
//!
//! Prints the Windows build, whether CS2 is running, the source actually opened
//! and a live level meter. `--wav` writes the capture to a 16-bit WAV file, only
//! when explicitly requested (the application itself never writes audio to disk).

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crosstalk_lib::audio::{
    self, platform, AudioBlock, AudioEvents, AudioService, BlockSink, CaptureMode, CaptureState,
    CaptureStatus, LevelReading, SourceInfo, StopReason,
};
use crosstalk_lib::game::Cs2Watcher;

struct Console {
    peak_db: Mutex<f32>,
    readings: AtomicU32,
}

impl AudioEvents for Console {
    fn status(&self, s: &CaptureStatus) {
        println!("[état] {:?}", s.state);
        if let Some(e) = &s.error {
            println!("        erreur {} : {}", e.code, e.message);
        }
        if let Some(r) = &s.fallback_reason {
            println!("        REPLI sur le périphérique : {r}");
        }
    }

    fn level(&self, r: LevelReading) {
        let n = self.readings.fetch_add(1, Ordering::Relaxed);
        let mut peak = self.peak_db.lock().unwrap();
        *peak = peak.max(r.peak_db);
        if n % 3 == 0 {
            let width = (((r.rms_db + 60.0) / 60.0).clamp(0.0, 1.0) * 40.0) as usize;
            println!(
                "  {:>6.1} dBFS rms  {:>6.1} dBFS crête  |{:<40}|",
                r.rms_db,
                r.peak_db,
                "#".repeat(width)
            );
        }
    }
}

/// 16-bit PCM WAV writer; the header is patched with the final size on drop.
struct WavSink {
    out: BufWriter<File>,
    data_bytes: u32,
    header_written: bool,
}

impl WavSink {
    fn create(path: &str) -> std::io::Result<Self> {
        Ok(Self {
            out: BufWriter::new(File::create(path)?),
            data_bytes: 0,
            header_written: false,
        })
    }

    fn header(&mut self, channels: u16, rate: u32) -> std::io::Result<()> {
        let block = channels * 2;
        let w = &mut self.out;
        w.write_all(b"RIFF\0\0\0\0WAVEfmt ")?;
        w.write_all(&16u32.to_le_bytes())?;
        w.write_all(&1u16.to_le_bytes())?;
        w.write_all(&channels.to_le_bytes())?;
        w.write_all(&rate.to_le_bytes())?;
        w.write_all(&(rate * block as u32).to_le_bytes())?;
        w.write_all(&block.to_le_bytes())?;
        w.write_all(&16u16.to_le_bytes())?;
        w.write_all(b"data\0\0\0\0")?;
        self.header_written = true;
        Ok(())
    }

    fn samples(&mut self, samples: impl Iterator<Item = f32>) {
        for s in samples {
            let v = (s.clamp(-1.0, 1.0) * 32_767.0) as i16;
            if self.out.write_all(&v.to_le_bytes()).is_ok() {
                self.data_bytes += 2;
            }
        }
    }
}

impl BlockSink for WavSink {
    fn block(&mut self, b: &AudioBlock) {
        if !self.header_written && self.header(b.channels, b.sample_rate).is_err() {
            return;
        }
        self.samples(b.samples.iter().copied());
    }

    fn silence(&mut self, frames: usize, channels: u16, sample_rate: u32) {
        if !self.header_written && self.header(channels, sample_rate).is_err() {
            return;
        }
        self.samples(std::iter::repeat(0.0).take(frames * channels as usize));
    }
}

impl Drop for WavSink {
    fn drop(&mut self) {
        let _ = (|| -> std::io::Result<()> {
            self.out.flush()?;
            let f = self.out.get_mut();
            f.seek(SeekFrom::Start(4))?;
            f.write_all(&(36 + self.data_bytes).to_le_bytes())?;
            f.seek(SeekFrom::Start(40))?;
            f.write_all(&self.data_bytes.to_le_bytes())?;
            Ok(())
        })();
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage : crosstalk-audio-probe [--list] [--mode auto|process|device] [--device <id>] [--seconds N] [--wav fichier.wav]");
        return;
    }

    let p = platform::info();
    println!(
        "Windows build : {}  (capture par processus documentée : {})",
        p.windows_build.map_or("?".into(), |b| b.to_string()),
        p.process_loopback_documented
            .map_or("?", |d| if d { "oui" } else { "non, sera tentée" })
    );

    let events = Arc::new(Console {
        peak_db: Mutex::new(audio::level::DB_FLOOR),
        readings: AtomicU32::new(0),
    });
    let service = AudioService::new(audio::default_backend(), events.clone());

    match service.devices() {
        Ok(list) => {
            println!("Périphériques de sortie :");
            for d in &list {
                println!(
                    "  {} {}\n      id : {}",
                    if d.is_default { "*" } else { " " },
                    d.name,
                    d.id
                );
            }
        }
        Err(e) => println!("Périphériques : erreur — {e}"),
    }
    if args.iter().any(|a| a == "--list") {
        return;
    }

    let watcher = Cs2Watcher::default();
    watcher.poll();
    let cs2 = watcher.status();
    println!(
        "CS2 : {}",
        cs2.pid
            .map_or("non détecté".into(), |pid| format!("cs2.exe, PID {pid}"))
    );

    let mode = match arg(&args, "--mode").as_deref() {
        Some("process") => CaptureMode::Process,
        Some("device") => CaptureMode::Device,
        _ => CaptureMode::Auto,
    };
    let seconds: u64 = arg(&args, "--seconds")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    if let Some(path) = arg(&args, "--wav") {
        match WavSink::create(&path) {
            Ok(sink) => {
                println!("Enregistrement WAV : {path}");
                service.set_sink(Some(Box::new(sink)));
            }
            Err(e) => println!("Impossible de créer {path} : {e}"),
        }
    }

    let started = Instant::now();
    let st = service.start(mode, arg(&args, "--device"), cs2.pid);
    println!("Ouverture en {} ms", started.elapsed().as_millis());
    if st.state != CaptureState::Capturing {
        std::process::exit(1);
    }
    match st.source {
        Some(SourceInfo::Process { pid, process_name }) => {
            println!("Source : {process_name} (PID {pid}), capture par processus")
        }
        Some(SourceInfo::Device { name, .. }) => println!("Source : périphérique « {name} »"),
        None => {}
    }
    if let Some(f) = st.format {
        println!(
            "Format : {} Hz, {} canaux, {:?}",
            f.sample_rate, f.channels, f.sample_format
        );
    }

    std::thread::sleep(Duration::from_secs(seconds));
    let t = Instant::now();
    service.stop(StopReason::User);
    service.set_sink(None);
    println!("Arrêt en {} ms", t.elapsed().as_millis());
    let peak = *events.peak_db.lock().unwrap();
    println!(
        "Résultat : {} mesures, crête max {:.1} dBFS — {}",
        events.readings.load(Ordering::Relaxed),
        peak,
        if peak > -50.0 {
            "signal reçu"
        } else {
            "aucun signal (le jeu émet-il du son ?)"
        }
    );
}
