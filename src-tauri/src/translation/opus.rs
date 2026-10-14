//! The Opus-MT engines (Helsinki-NLP Marian models, run by candle).

use std::path::Path;

use super::engine::{Chain, Engine, EngineLoader};
use super::models;

/// Real models from `<models dir>/opus-mt-<src>-<tgt>/`, one or two in a
/// row (through English when there is no direct model).
pub struct OpusLoader;

impl EngineLoader for OpusLoader {
    fn installed(&self, dir: &Path, source: &str, target: &str) -> bool {
        models::route_installed(dir, source, target)
    }

    fn load(&self, dir: &Path, source: &str, target: &str) -> Result<Box<dyn Engine>, String> {
        let route = models::route(source, target).ok_or("langue non gérée")?;
        let mut steps: Vec<Box<dyn Engine>> = Vec::new();
        for p in route {
            let m =
                super::marian::Marian::load(&models::pair_dir(dir, p), super::marian::Mode::Fast)
                    .map_err(|e| format!("{} : {e}", p.dir))?;
            steps.push(Box::new(m));
        }
        Ok(if steps.len() == 1 {
            steps.pop().expect("one")
        } else {
            Box::new(Chain(steps))
        })
    }
}
