//! LRU cache of final translations, per route and normalised text.

use std::collections::{HashMap, VecDeque};

use super::CACHE_CAPACITY;

#[derive(Default)]
pub(super) struct Cache {
    /// Translation and the engine that produced it.
    map: HashMap<(String, String), (String, Option<String>)>,
    order: VecDeque<(String, String)>,
}

impl Cache {
    pub(super) fn get(&mut self, key: &(String, String)) -> Option<(String, Option<String>)> {
        let v = self.map.get(key)?.clone();
        if let Some(i) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(i).expect("present");
            self.order.push_back(k);
        }
        Some(v)
    }

    pub(super) fn put(&mut self, key: (String, String), value: (String, Option<String>)) {
        if self.map.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
            if self.order.len() > CACHE_CAPACITY {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }
}

/// Repeated callouts ("gg", "rush B") hit the cache whatever their case or
/// spacing (the cache is cleared when the glossary changes).
pub(super) fn cache_key(text: &str) -> String {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    words.join(" ")
}
