//! The translation thread: takes the queued lines (final ones first) and
//! translates them with the lexicon, the glossary, the cache and the LLM.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::cache::cache_key;
use super::{failure, Inner, Job, Translation, MAX_WAIT};
use crate::translation::{callouts, glossary, guard, languages};

/// Start of the reason shown when translation::guard rejected both answers.
pub const REJECTED: &str = "traduction écartée";

pub(super) fn worker_loop(inner: &Inner) {
    crate::cpu::lower_current_thread_priority();
    while !inner.shutdown.load(Ordering::SeqCst) {
        let job = {
            let mut q = inner.queue.lock().unwrap();
            loop {
                if inner.shutdown.load(Ordering::SeqCst) {
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
        let result = run(inner, &job);
        match (&job.reply, result) {
            (Some(tx), Some(t)) => {
                let _ = tx.send(t);
            }
            (None, Some(t)) => inner.events.result(&t),
            _ => {}
        }
    }
}

/// Translates one job; `None` when it was purged or is a live line that
/// gave nothing (nothing is published, the final line will say why).
pub(super) fn run(inner: &Inner, job: &Job) -> Option<Translation> {
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
    // Known Counter-Strike callouts: exact and instant, without the LLM
    // (F-36). Their renderings are French: only when translating into French.
    if target == "fr" {
        if let Some(text) = callouts::translate(&job.lang, &job.text) {
            return Some(Translation {
                translated: Some(text),
                partial: job.partial,
                engine: Some("lexicon".into()),
                ..base()
            });
        }
    }
    let unsure = job.lang == languages::UNSURE;
    if !unsure && !languages::is_language(&job.lang) {
        return fail("langue non gérée");
    }
    if !manual && !unsure && !config.sources.contains(&job.lang) {
        return fail("langue non choisie (Langues)");
    }
    // A live line older than 2 s has been replaced on screen: skip it.
    if job.partial && job.queued.elapsed() > Duration::from_secs(2) {
        return None;
    }
    if !manual && job.queued.elapsed() > MAX_WAIT {
        return fail("délai dépassé : la ligne reste dans sa langue");
    }
    let translator = &inner.translator;
    if let Some(problem) = translator.problem() {
        return if job.partial { None } else { fail(&problem) };
    }
    let started = Instant::now();
    if job.partial {
        let out = glossary::translate(&job.text, &config.glossary, |t| {
            translator.translate_partial(t, &job.lang, &target)
        });
        if job.generation != inner.generation.load(Ordering::SeqCst) {
            return None;
        }
        return match out {
            // A live line that changed a count, a site or a side is not shown.
            Ok((text, _))
                if guard::check(
                    &job.text,
                    &job.lang,
                    &callouts::clean_output(&text),
                    &target,
                )
                .is_err() =>
            {
                None
            }
            Ok((text, protected)) if !callouts::clean_output(&text).is_empty() => {
                Some(Translation {
                    translated: Some(callouts::clean_output(&text)),
                    error: None,
                    ms: started.elapsed().as_millis() as u32,
                    glossary: protected,
                    partial: true,
                    engine: Some(translator.name().into()),
                    ..base()
                })
            }
            _ => None,
        };
    }
    let key = (format!("{}>{target}", job.lang), cache_key(&job.text));
    if let Some((hit, used)) = inner.cache.lock().unwrap().get(&key) {
        return Some(Translation {
            translated: Some(hit),
            error: None,
            cached: true,
            engine: used,
            ms: started.elapsed().as_millis() as u32,
            ..base()
        });
    }
    let outcome = glossary::translate(&job.text, &config.glossary, |t| {
        translator.translate(t, &job.lang, &target)
    })
    .map(|(t, p)| (callouts::clean_output(&t), p));
    // Smart Comms: a count, a site or a side changed → once more, firmer;
    // still changed → the line stays in its language, with why.
    let checked = |text: &str| guard::check(&job.text, &job.lang, text, &target);
    let outcome = match outcome {
        Ok((text, p)) => match checked(&text) {
            Ok(()) => Ok((text, p)),
            Err(_) => glossary::translate(&job.text, &config.glossary, |t| {
                translator.translate_strict(t, &job.lang, &target)
            })
            .map(|(t, p)| (callouts::clean_output(&t), p))
            .and_then(|(t, p)| match checked(&t) {
                Ok(()) => Ok((t, p)),
                Err(issue) => Err(format!("{REJECTED} : {}", issue.reason())),
            }),
        },
        e => e,
    };
    let ms = started.elapsed().as_millis() as u32;
    if !manual && job.generation != inner.generation.load(Ordering::SeqCst) {
        return None;
    }
    match outcome {
        Ok((text, protected)) if !text.trim().is_empty() => {
            let used = Some(translator.name().to_string());
            inner
                .cache
                .lock()
                .unwrap()
                .put(key, (text.clone(), used.clone()));
            Some(Translation {
                translated: Some(text),
                error: None,
                ms,
                glossary: protected,
                engine: used,
                ..base()
            })
        }
        Ok(_) => fail("traduction vide"),
        Err(e) if e.starts_with(REJECTED) => fail(&e),
        Err(e) => fail(&format!("LLM : {e}")),
    }
}
