//! Minimal reader for Valve's text KeyValues files (`libraryfolders.vdf`,
//! `localconfig.vdf`): quoted or bare tokens, nested `{ }` blocks, `//`
//! comments. Keys are compared case-insensitively, as Steam does.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Obj(Vec<(String, Value)>),
}

impl Value {
    /// Child value by key (case-insensitive).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(items) => items
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Value::Str(_) => None,
        }
    }

    /// Follows a path of keys.
    pub fn path(&self, keys: &[&str]) -> Option<&Value> {
        keys.iter().try_fold(self, |v, k| v.get(k))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Value::Obj(items) => items,
            Value::Str(_) => &[],
        }
    }
}

enum Token {
    Text(String),
    Open,
    Close,
}

fn tokens(src: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            c if c.is_whitespace() => {}
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '/' if it.peek() == Some(&'/') => {
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                let mut s = String::new();
                loop {
                    match it.next() {
                        None => return Err("chaîne non terminée".into()),
                        Some('"') => break,
                        Some('\\') => match it.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(c) => s.push(c),
                            None => return Err("chaîne non terminée".into()),
                        },
                        Some(c) => s.push(c),
                    }
                }
                out.push(Token::Text(s));
            }
            c => {
                let mut s = String::from(c);
                while let Some(&n) = it.peek() {
                    if n.is_whitespace() || n == '{' || n == '}' || n == '"' {
                        break;
                    }
                    s.push(n);
                    it.next();
                }
                out.push(Token::Text(s));
            }
        }
    }
    Ok(out)
}

/// Parses a whole file into an object of its top-level pairs.
pub fn parse(src: &str) -> Result<Value, String> {
    let toks = tokens(src.trim_start_matches('\u{feff}'))?;
    let mut pos = 0;
    let root = block(&toks, &mut pos, false)?;
    Ok(root)
}

fn block(toks: &[Token], pos: &mut usize, nested: bool) -> Result<Value, String> {
    let mut items = Vec::new();
    while *pos < toks.len() {
        match &toks[*pos] {
            Token::Close if nested => {
                *pos += 1;
                return Ok(Value::Obj(items));
            }
            Token::Close => return Err("accolade fermante en trop".into()),
            Token::Open => return Err("bloc sans clé".into()),
            Token::Text(key) => {
                *pos += 1;
                match toks.get(*pos) {
                    Some(Token::Text(v)) => {
                        items.push((key.clone(), Value::Str(v.clone())));
                        *pos += 1;
                    }
                    Some(Token::Open) => {
                        *pos += 1;
                        items.push((key.clone(), block(toks, pos, true)?));
                    }
                    _ => return Err(format!("valeur manquante pour « {key} »")),
                }
            }
        }
    }
    if nested {
        return Err("bloc non fermé".into());
    }
    Ok(Value::Obj(items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nested_blocks_comments_and_escapes() {
        let v = parse(
            "\u{feff}\"libraryfolders\"\n{\n  // comment\n  \"0\" { \"path\" \"C:\\\\Program Files (x86)\\\\Steam\" \"apps\" { \"730\" \"123\" } }\n  bare value\n}\n",
        )
        .unwrap();
        let lf = v.get("LibraryFolders").unwrap();
        assert_eq!(
            lf.path(&["0", "path"]).unwrap().as_str(),
            Some(r"C:\Program Files (x86)\Steam")
        );
        assert!(lf.path(&["0", "apps", "730"]).is_some());
        assert_eq!(lf.get("bare").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn rejects_broken_files() {
        assert!(parse("\"a\" { \"b\" \"c\"").is_err());
        assert!(parse("\"a\" \"b\" }").is_err());
        assert!(parse("\"a\" \"unterminated").is_err());
    }
}
