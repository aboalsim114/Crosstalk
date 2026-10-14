//! The translation thread: loads the models of the chosen languages, then
//! translates the queued lines (final ones first) with the lexicon, the
//! glossary, the cache and the engines.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::cache::cache_key;
use super::{failure, Inner, Job, PairState, Translation, MAX_WAIT};
use crate::translation::engine::Engine;
use crate::translation::{callouts, glossary, models};

pub(super) type Engines = HashMap<(String, String), Box<dyn Engine>>;

pub(super) fn worker_loop(inner: &Inner) {
    crate::cpu::lower_current_thread_priority();
    let mut engines: Engines = HashMap::new();
    while !inner.shutdown.load(Ordering::SeqCst) {
        if inner.reload.swap(false, Ordering::SeqCst) {
            load_installed(inner, &mut engines);
        }
        let job = {
            let mut q = inner.queue.lock().unwrap();
            loop {
                if inner.shutdown.load(Ordering::SeqCst) || inner.reload.load(Ordering::SeqCst) {
                    break None;
                }
                // Final lines before the pending live one.
                let next = q
                    .iter()
                    .position(|j| !j.partial)
                    .or((!q.is_empty()).then_some(0));
                if let Some(j) = next.and_then(|i| q.remove(i)) {
                    break Some(j);
                }
                q = inner
                    .wake
                    .wait_timeout(q, Duration::from_millis(500))
                    .unwrap()
                    .0;
            }
        };
        let Some(job) = job else { continue };
        let result = run(inner, &mut engines, &job);
        match (&job.reply, result) {
            (Some(tx), Some(t)) => {
                let _ = tx.send(t);
            }
            (None, Some(t)) => inner.events.result(&t),
            _ => {}
        }
    }
}

fn load_installed(inner: &Inner, engines: &mut Engines) {
    let config = inner.config.lock().unwrap().clone();
    let wanted: Vec<(String, String)> = config
        .translated()
        .into_iter()
        .map(|s| (s, config.target.clone()))
        .collect();
    // Models of languages no longer chosen free their memory.
    engines.retain(|k, _| wanted.contains(k));
    for (src, tgt) in wanted {
        if engines.contains_key(&(src.clone(), tgt.clone())) {
            inner.set_pair(&src, PairState::Ready, None, None);
            continue;
        }
        if !inner.loader.installed(&inner.models_dir, &src, &tgt) {
            inner.set_pair(&src, PairState::Missing, None, None);
            continue;
        }
        inner.set_pair(&src, PairState::Loading, None, None);
        let t = Instant::now();
        match inner.loader.load(&inner.models_dir, &src, &tgt) {
            Ok(e) => {
                inner.set_pair(
                    &src,
                    PairState::Ready,
                    None,
                    Some(t.elapsed().as_millis() as u32),
                );
                engines.insert((src, tgt), e);
            }
            Err(e) => inner.set_pair(&src, PairState::Error, Some(e), None),
        }
    }
}

/// Translates one job; `None` when it was purged (nothing is published).
pub(super) fn run(inner: &Inner, engines: &mut Engines, job: &Job) -> Option<Translation> {
    let manual = job.reply.is_some();
    if !manual && job.generation != inner.generation.load(Ordering::SeqCst) {
        return None;
    }
    let queue_ms = job.queued.elapsed().as_millis() as u32;
    let mut config = inner.config.lock().unwrap().clone();
    let target = config.target.clone();
    let base = || Translation {
        target: target.clone(),
        ..failure(job.id, &job.lang, &job.text, "", queue_ms)
    };
    let fail = |why: &str| {
        Some(Translation {
            error: Some(why.into()),
            ..base()
        })
    };
    for t in inner.map_terms.lock().unwrap().iter() {
        if !config
            .glossary
            .iter()
            .any(|g| g.source.eq_ignore_ascii_case(&t.source))
        {
            config.glossary.push(t.clone());
        }
    }
    if !config.enabled && !manual {
        return fail("traduction désactivée");
    }
    if job.lang == target {
        // F-32: already in the user's language, shown as is.
        return Some(Translation {
            translated: Some(job.text.clone()),
            ..base()
        });
    }
    // Known Counter-Strike callouts: exact, instant, no model needed (F-36).
    // Their renderings are French: only when translating into French.
    if target == "fr" {
        if let Some(text) = callouts::translate(&job.lang, &job.text) {
            return Some(Translation {
                translated: Some(text),
                partial: job.partial,
                ..base()
            });
        }
    }
    if models::route(&job.lang, &target).is_none() {
        return fail("langue non gérée");
    }
    // A language the user did not choose (Langues) is shown as is.
    if !manual && !config.sources.contains(&job.lang) {
        return fail("langue non choisie (Langues)");
    }
    let route_key = (job.lang.clone(), target.clone());
    // A live line older than 2 s has been replaced on screen: skip it.
    if job.partial && job.queued.elapsed() > Duration::from_secs(2) {
        return None;
    }
    if !manual && job.queued.elapsed() > MAX_WAIT {
        return fail("moteur indisponible (délai dépassé)");
    }
    let Some(engine) = engines.get_mut(&route_key) else {
        if job.partial {
            return None; // the final line will say why
        }
        // Present but unreadable is not the same as absent.
        let broken = inner
            .pairs
            .lock()
            .unwrap()
            .get(&job.lang)
            .is_some_and(|p| p.state == PairState::Error);
        if broken {
            return fail("modèle de traduction illisible (voir Audio & IA)");
        }
        return fail("modèle de traduction absent");
    };
    let key = (format!("{}>{target}", job.lang), cache_key(&job.text));
    let started = Instant::now();
    if job.partial {
        let out = glossary::translate(&job.text, &config.glossary, |t| engine.translate_fast(t));
        if job.generation != inner.generation.load(Ordering::SeqCst) {
            return None;
        }
        return match out {
            Ok((text, protected)) if !callouts::clean_output(&text).is_empty() => {
                Some(Translation {
                    translated: Some(callouts::clean_output(&text)),
                    error: None,
                    ms: started.elapsed().as_millis() as u32,
                    glossary: protected,
                    partial: true,
                    ..base()
                })
            }
            _ => None,
        };
    }
    if let Some(hit) = inner.cache.lock().unwrap().get(&key) {
        return Some(Translation {
            translated: Some(hit),
            error: None,
            cached: true,
            ms: started.elapsed().as_millis() as u32,
            ..base()
        });
    }
    let outcome = glossary::translate(&job.text, &config.glossary, |t| engine.translate(t));
    let ms = started.elapsed().as_millis() as u32;
    if !manual && job.generation != inner.generation.load(Ordering::SeqCst) {
        return None;
    }
    match outcome.map(|(t, p)| (callouts::clean_output(&t), p)) {
        Ok((text, protected)) if !text.trim().is_empty() => {
            inner.cache.lock().unwrap().put(key, text.clone());
            Some(Translation {
                id: job.id,
                lang: job.lang.clone(),
                original: job.text.clone(),
                translated: Some(text),
                error: None,
                ms,
                queue_ms,
                cached: false,
                glossary: protected,
                partial: false,
                target: target.clone(),
            })
        }
        Ok(_) => fail("traduction vide"),
        Err(e) => fail(&format!("moteur indisponible : {e}")),
    }
}
