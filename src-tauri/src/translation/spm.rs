//! SentencePiece unigram tokenizer (the `source.spm` / `target.spm` files of
//! Opus-MT), in pure Rust: protobuf model, precompiled normalization charsmap,
//! Viterbi segmentation and decoding. Checked against the reference Python
//! implementation (`tests/fixtures/marian-tiny`, `scripts/make-marian-fixture.py`).

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Normal,
    Unknown,
    Control,
    UserDefined,
    Unused,
    Byte,
}

#[derive(Debug, Clone)]
struct Piece {
    text: String,
    score: f32,
    kind: Kind,
}

/// The escaped space of SentencePiece (U+2581).
pub const SPACE: char = '\u{2581}';
const UNK_PENALTY: f32 = 10.0;

#[derive(Debug)]
pub struct SentencePiece {
    pieces: Vec<Piece>,
    index: HashMap<String, usize>,
    max_piece_chars: usize,
    min_score: f32,
    max_score: f32,
    unk: usize,
    charsmap: Option<CharsMap>,
    add_dummy_prefix: bool,
    remove_extra_whitespaces: bool,
    escape_whitespaces: bool,
}

impl SentencePiece {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut pieces = Vec::new();
        let mut normalizer = None;
        let mut model_type = 1u64; // UNIGRAM
        for field in Fields::new(bytes) {
            match field? {
                (1, Value::Bytes(b)) => pieces.push(parse_piece(b)?),
                (2, Value::Bytes(b)) => {
                    for f in Fields::new(b) {
                        if let (3, Value::Varint(v)) = f? {
                            model_type = v;
                        }
                    }
                }
                (3, Value::Bytes(b)) => normalizer = Some(b),
                _ => {}
            }
        }
        if model_type != 1 {
            return Err(format!("modèle SentencePiece non unigram ({model_type})"));
        }
        let (mut charsmap, mut add_dummy_prefix, mut remove_extra, mut escape) =
            (None, true, true, true);
        if let Some(b) = normalizer {
            for f in Fields::new(b) {
                match f? {
                    (2, Value::Bytes(m)) if !m.is_empty() => charsmap = Some(CharsMap::new(m)?),
                    (3, Value::Varint(v)) => add_dummy_prefix = v != 0,
                    (4, Value::Varint(v)) => remove_extra = v != 0,
                    (5, Value::Varint(v)) => escape = v != 0,
                    _ => {}
                }
            }
        }
        let unk = pieces
            .iter()
            .position(|p| p.kind == Kind::Unknown)
            .ok_or("pas de pièce <unk>")?;
        let scored = || {
            pieces
                .iter()
                .filter(|p| p.kind == Kind::Normal)
                .map(|p| p.score)
        };
        let min_score = scored().fold(f32::INFINITY, f32::min);
        let max_score = scored().fold(f32::NEG_INFINITY, f32::max);
        let mut index = HashMap::new();
        let mut max_piece_chars = 1;
        for (i, p) in pieces.iter().enumerate() {
            if matches!(p.kind, Kind::Normal | Kind::UserDefined) {
                max_piece_chars = max_piece_chars.max(p.text.chars().count());
                index.insert(p.text.clone(), i);
            }
        }
        Ok(Self {
            pieces,
            index,
            max_piece_chars,
            min_score,
            max_score,
            unk,
            charsmap,
            add_dummy_prefix,
            remove_extra_whitespaces: remove_extra,
            escape_whitespaces: escape,
        })
    }

    /// The text as SentencePiece normalizes it before segmentation.
    pub fn normalize(&self, text: &str) -> String {
        let mapped = match &self.charsmap {
            Some(map) => map.apply(text),
            None => text.to_string(),
        };
        let mut out = String::with_capacity(mapped.len() + 3);
        if self.add_dummy_prefix {
            out.push(' ');
        }
        if self.remove_extra_whitespaces {
            let mut first = true;
            for word in mapped.split(' ').filter(|w| !w.is_empty()) {
                if !first {
                    out.push(' ');
                }
                out.push_str(word);
                first = false;
            }
        } else {
            out.push_str(&mapped);
        }
        if self.add_dummy_prefix && out.len() == 1 {
            out.clear();
        }
        if self.escape_whitespaces {
            out = out.replace(' ', &SPACE.to_string());
        }
        out
    }

    /// Splits the text into pieces (unknown characters merged, like spm_encode).
    pub fn encode(&self, text: &str) -> Vec<String> {
        let norm = self.normalize(text);
        let chars: Vec<(usize, char)> = norm.char_indices().collect();
        let n = chars.len();
        if n == 0 {
            return Vec::new();
        }
        let byte_at = |i: usize| if i == n { norm.len() } else { chars[i].0 };
        // best[i] = (score, start, piece) of the best segmentation of chars[..i].
        let mut best: Vec<Option<(f32, usize, usize)>> = vec![None; n + 1];
        best[0] = Some((0.0, 0, usize::MAX));
        for start in 0..n {
            let Some((base, _, _)) = best[start] else {
                continue;
            };
            let mut matched_single = false;
            for len in 1..=self.max_piece_chars.min(n - start) {
                let end = start + len;
                if let Some(&id) = self.index.get(&norm[byte_at(start)..byte_at(end)]) {
                    let p = &self.pieces[id];
                    let score = if p.kind == Kind::UserDefined {
                        len as f32 * self.max_score - 0.1
                    } else {
                        p.score
                    };
                    if len == 1 {
                        matched_single = true;
                    }
                    let cand = base + score;
                    if best[end].map_or(true, |(s, _, _)| cand > s) {
                        best[end] = Some((cand, start, id));
                    }
                }
            }
            if !matched_single {
                let cand = base + self.min_score - UNK_PENALTY;
                if best[start + 1].map_or(true, |(s, _, _)| cand > s) {
                    best[start + 1] = Some((cand, start, self.unk));
                }
            }
        }
        let mut spans = Vec::new();
        let mut end = n;
        while end > 0 {
            let (_, start, id) = best[end].expect("every position is reachable");
            spans.push((start, end, id));
            end = start;
        }
        spans.reverse();
        let mut out: Vec<String> = Vec::with_capacity(spans.len());
        let mut prev_unk = false;
        for (start, end, id) in spans {
            let piece = &norm[byte_at(start)..byte_at(end)];
            let unk = id == self.unk;
            if unk && prev_unk {
                out.last_mut().expect("previous piece").push_str(piece);
            } else {
                out.push(piece.to_string());
            }
            prev_unk = unk;
        }
        out
    }

    /// Joins pieces back into text (the inverse of `encode` for known pieces).
    pub fn decode(&self, pieces: &[&str]) -> String {
        let text: String = pieces.concat().replace(SPACE, " ");
        if self.add_dummy_prefix {
            text.strip_prefix(' ').unwrap_or(&text).to_string()
        } else {
            text
        }
    }

    /// Pieces that can appear in a text of this language.
    pub fn text_pieces(&self) -> impl Iterator<Item = &str> {
        self.pieces
            .iter()
            .filter(|p| matches!(p.kind, Kind::Normal | Kind::UserDefined))
            .map(|p| p.text.as_str())
    }

    /// True for pieces that never carry text (`<s>`, `</s>`, `<unk>`, padding).
    pub fn is_special(&self, piece: &str) -> bool {
        self.pieces
            .iter()
            .any(|p| p.text == piece && matches!(p.kind, Kind::Control | Kind::Unknown))
    }
}

fn parse_piece(bytes: &[u8]) -> Result<Piece, String> {
    let (mut text, mut score, mut kind) = (String::new(), 0.0, Kind::Normal);
    for f in Fields::new(bytes) {
        match f? {
            (1, Value::Bytes(b)) => {
                text = String::from_utf8(b.to_vec()).map_err(|_| "pièce non UTF-8")?
            }
            (2, Value::Fixed32(v)) => score = f32::from_bits(v),
            (3, Value::Varint(v)) => {
                kind = match v {
                    2 => Kind::Unknown,
                    3 => Kind::Control,
                    4 => Kind::UserDefined,
                    5 => Kind::Unused,
                    6 => Kind::Byte,
                    _ => Kind::Normal,
                }
            }
            _ => {}
        }
    }
    Ok(Piece { text, score, kind })
}

// ---------- minimal protobuf reader ----------

enum Value<'a> {
    Varint(u64),
    Fixed64,
    Bytes(&'a [u8]),
    Fixed32(u32),
}

struct Fields<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Fields<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *self.buf.get(self.pos).ok_or("protobuf tronqué")?;
            self.pos += 1;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("varint trop long".into())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).ok_or("protobuf invalide")?;
        let s = self.buf.get(self.pos..end).ok_or("protobuf tronqué")?;
        self.pos = end;
        Ok(s)
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u64, Value<'a>), String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buf.len() {
            return None;
        }
        let item = (|| {
            let key = self.varint()?;
            let value = match key & 7 {
                0 => Value::Varint(self.varint()?),
                1 => {
                    self.take(8)?;
                    Value::Fixed64
                }
                2 => {
                    let n = self.varint()? as usize;
                    Value::Bytes(self.take(n)?)
                }
                5 => {
                    let b = self.take(4)?;
                    Value::Fixed32(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                }
                t => return Err(format!("type protobuf {t} non géré")),
            };
            Ok((key >> 3, value))
        })();
        if item.is_err() {
            self.pos = self.buf.len();
        }
        Some(item)
    }
}

// ---------- precompiled normalization (Darts double-array trie) ----------

#[derive(Debug)]
struct CharsMap {
    trie: Vec<u32>,
    normalized: Vec<u8>,
}

impl CharsMap {
    fn new(blob: &[u8]) -> Result<Self, String> {
        let size = u32::from_le_bytes(blob.get(..4).ok_or("charsmap tronqué")?.try_into().unwrap())
            as usize;
        let trie_bytes = blob.get(4..4 + size).ok_or("charsmap tronqué")?;
        let trie = trie_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        Ok(Self {
            trie,
            normalized: blob[4 + size..].to_vec(),
        })
    }

    /// Longest prefix of `key` present in the trie: (bytes matched, value).
    fn longest_prefix(&self, key: &[u8]) -> Option<(usize, usize)> {
        let unit = |pos: usize| self.trie.get(pos).copied();
        let offset = |u: u32| ((u >> 10) << ((u & (1 << 9)) >> 6)) as usize;
        let mut pos = 0usize;
        pos ^= offset(unit(pos)?);
        let mut found = None;
        for (i, &c) in key.iter().enumerate() {
            pos ^= c as usize;
            let u = unit(pos)?;
            if (u & ((1 << 31) | 0xff)) != c as u32 {
                break;
            }
            pos ^= offset(u);
            if (u >> 8) & 1 == 1 {
                found = Some((i + 1, (unit(pos)? & ((1 << 31) - 1)) as usize));
            }
        }
        found
    }

    fn apply(&self, text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match self.longest_prefix(&bytes[i..]) {
                Some((len, value)) if value < self.normalized.len() => {
                    let rest = &self.normalized[value..];
                    let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
                    out.extend_from_slice(&rest[..end]);
                    i += len;
                }
                _ => {
                    let ch_len = text[i..].chars().next().map_or(1, char::len_utf8);
                    out.extend_from_slice(&bytes[i..i + ch_len]);
                    i += ch_len;
                }
            }
        }
        String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into())
    }
}
