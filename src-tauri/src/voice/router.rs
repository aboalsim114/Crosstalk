//! The virtual cable feed (Windows): the user's real microphone, passed
//! through as is, plus the AI voice when it speaks. CS2 listens to the cable
//! ("CABLE Output" as microphone), so teammates hear both. Optionally the AI
//! voice is also played in the user's headset ("m'entendre").
//!
//! Nothing is pressed in the game: with push-to-talk the user holds their
//! voice key while the AI voice speaks; with an open microphone it just works.

// Used by the Windows router; elsewhere only by the tests.
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use super::mix::{self, Queue};

/// Microphone kept in advance at most (latency of the passthrough).
const MIC_MAX_SECONDS: f32 = 0.12;
/// AI voice waiting at most (several sentences).
const VOICE_MAX_SECONDS: usize = 30;

/// Shared between the router thread and the service.
pub struct Feed {
    pub rate: u32,
    pub voice: Queue,
    pub monitor_rate: AtomicU32,
    pub monitor: Queue,
    pub mic_gain: std::sync::Mutex<f32>,
    pub voice_gain: std::sync::Mutex<f32>,
}

impl Feed {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            voice: Queue::new(rate as usize * VOICE_MAX_SECONDS),
            monitor_rate: AtomicU32::new(0),
            monitor: Queue::new(48_000 * VOICE_MAX_SECONDS),
            mic_gain: std::sync::Mutex::new(1.0),
            voice_gain: std::sync::Mutex::new(1.0),
        }
    }

    /// Queues a sentence for the cable (and the headset when monitored).
    pub fn speak(&self, samples: &[f32], rate: u32) {
        self.voice.push(&mix::resample(samples, rate, self.rate));
        let m = self.monitor_rate.load(Ordering::Relaxed);
        if m > 0 {
            self.monitor.push(&mix::resample(samples, rate, m));
        }
    }

    /// The AI voice is still playing.
    pub fn speaking(&self) -> bool {
        !self.voice.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteConfig {
    /// Real microphone (None: no passthrough, only the AI voice goes to the cable).
    pub mic: Option<String>,
    pub cable: String,
    /// Also play the AI voice on the default output.
    pub monitor: bool,
}

pub struct Router {
    pub config: RouteConfig,
    pub feed: Arc<Feed>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Router {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Router {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Starts routing; errors (cable or microphone missing) come back before the thread runs.
#[cfg(windows)]
pub fn start(
    config: RouteConfig,
    on_error: impl Fn(String) + Send + 'static,
) -> Result<Router, String> {
    use crate::audio::backend::Packet;
    use crate::audio::wasapi;

    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<Arc<Feed>, String>>();
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let cfg = config.clone();
    let thread = std::thread::Builder::new()
        .name("voice-router".into())
        .spawn(move || {
            let mut cable = match wasapi::open_render(Some(&cfg.cable)) {
                Ok(c) => c,
                Err(e) => return drop(ready_tx.send(Err(format!("câble virtuel : {e}")))),
            };
            let rate = cable.format().sample_rate;
            let feed = Arc::new(Feed::new(rate));
            let mut mic = match cfg.mic.as_deref() {
                Some(id) => match wasapi::open_microphone(Some(id)) {
                    Ok(m) => Some(m),
                    Err(e) => return drop(ready_tx.send(Err(format!("micro : {e}")))),
                },
                None => None,
            };
            let mut monitor = if cfg.monitor {
                wasapi::open_render(None).ok()
            } else {
                None
            };
            if let Some(m) = &monitor {
                feed.monitor_rate
                    .store(m.format().sample_rate, Ordering::Relaxed);
            }
            let _ = ready_tx.send(Ok(feed.clone()));
            let mic_q = Queue::new((rate as f32 * MIC_MAX_SECONDS) as usize);
            let mut mono = Vec::new();
            while !stop2.load(Ordering::SeqCst) {
                if let Some(m) = mic.as_mut() {
                    let mic_rate = m.format().sample_rate;
                    let res = m.read(2, &mut |p| match p {
                        Packet::Audio(block) => {
                            mono.clear();
                            mono.extend(crate::audio::convert::mono_frames(
                                block.samples,
                                block.channels,
                            ));
                            mic_q.push(&mix::resample(&mono, mic_rate, rate));
                        }
                        Packet::Silence { frames, .. } => {
                            let n = frames as u64 * rate as u64 / mic_rate.max(1) as u64;
                            mic_q.push(&vec![0.0; n as usize]);
                        }
                    });
                    if let Err(e) = res {
                        on_error(format!("micro : {e}"));
                        mic = None;
                    }
                }
                let mic_gain = *feed.mic_gain.lock().unwrap();
                let voice_gain = *feed.voice_gain.lock().unwrap();
                let res = cable.write(3, &mut |n| {
                    let m = mic_q.take(n);
                    let v = feed.voice.take(n);
                    mix::mix(&m, &v, n, mic_gain, voice_gain)
                });
                if let Err(e) = res {
                    on_error(format!("câble virtuel : {e}"));
                    break;
                }
                if let Some(out) = monitor.as_mut() {
                    if out
                        .write(0, &mut |n| {
                            let v = feed.monitor.take(n);
                            mix::mix(&[], &v, n, 0.0, voice_gain)
                        })
                        .is_err()
                    {
                        monitor = None;
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    match ready_rx.recv() {
        Ok(Ok(feed)) => Ok(Router {
            config,
            feed,
            stop,
            thread: Some(thread),
        }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => Err("routage audio interrompu".into()),
    }
}

#[cfg(not(windows))]
pub fn start(
    _config: RouteConfig,
    _on_error: impl Fn(String) + Send + 'static,
) -> Result<Router, String> {
    Err("la voix IA en jeu fonctionne sous Windows".into())
}

/// Plays a sentence once on the default output (voice preview, without the cable).
#[cfg(windows)]
pub fn preview(samples: Vec<f32>, rate: u32) -> Result<(), String> {
    std::thread::Builder::new()
        .name("voice-preview".into())
        .spawn(move || {
            let Ok(mut out) = crate::audio::wasapi::open_render(None) else {
                return;
            };
            let q = Queue::new(usize::MAX / 2);
            q.push(&mix::resample(&samples, rate, out.format().sample_rate));
            // Until played, plus the output buffer.
            let mut tail = 6;
            while tail > 0 {
                if q.is_empty() {
                    tail -= 1;
                }
                if out.write(20, &mut |n| q.take(n)).is_err() {
                    break;
                }
            }
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn preview(_samples: Vec<f32>, _rate: u32) -> Result<(), String> {
    Err("aperçu disponible sous Windows".into())
}

/// VB-CABLE's playback side ("CABLE Input (VB-Audio Virtual Cable)"), among outputs.
pub fn find_cable(outputs: &[crate::audio::AudioDevice]) -> Option<&crate::audio::AudioDevice> {
    outputs
        .iter()
        .find(|d| d.name.to_lowercase().contains("cable input"))
        .or_else(|| {
            outputs
                .iter()
                .find(|d| d.name.to_lowercase().contains("vb-audio"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioDevice;

    fn dev(name: &str) -> AudioDevice {
        AudioDevice {
            id: name.into(),
            name: name.into(),
            is_default: false,
        }
    }

    #[test]
    fn finds_vb_cable_among_the_outputs() {
        let outs = vec![
            dev("Speakers (Realtek(R) Audio)"),
            dev("CABLE Input (VB-Audio Virtual Cable)"),
            dev("Headset (HyperX)"),
        ];
        assert_eq!(
            find_cable(&outs).unwrap().name,
            "CABLE Input (VB-Audio Virtual Cable)"
        );
        assert!(find_cable(&[dev("Speakers")]).is_none());
        assert_eq!(
            find_cable(&[dev("VoiceMeeter Input (VB-Audio VoiceMeeter VAIO)")])
                .unwrap()
                .name,
            "VoiceMeeter Input (VB-Audio VoiceMeeter VAIO)"
        );
    }

    #[test]
    fn a_sentence_goes_to_the_cable_at_its_rate_and_to_the_headset_when_monitored() {
        let feed = Feed::new(48_000);
        feed.speak(&vec![0.5; 22_050], 22_050);
        assert_eq!(feed.voice.len(), 48_000);
        assert!(feed.monitor.is_empty());
        assert!(feed.speaking());
        feed.monitor_rate.store(44_100, Ordering::Relaxed);
        feed.speak(&vec![0.5; 22_050], 22_050);
        assert_eq!(feed.monitor.len(), 44_100);
        feed.voice.take(usize::MAX);
        assert!(!feed.speaking());
    }
}
