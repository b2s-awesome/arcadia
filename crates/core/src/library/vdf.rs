//! Minimal Valve KeyValues (VDF/ACF) text parser — enough for
//! `libraryfolders.vdf` and `appmanifest_*.acf`.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Obj(BTreeMap<String, Vdf>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            // Keys are case-insensitive in practice ("AppState" vs "appstate").
            Vdf::Obj(m) => m
                .get(key)
                .or_else(|| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)),
            _ => None,
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Vdf::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn entries(&self) -> impl Iterator<Item = (&String, &Vdf)> {
        let m = match self {
            Vdf::Obj(m) => Some(m),
            _ => None,
        };
        m.into_iter().flat_map(|m| m.iter())
    }
}

#[derive(Debug, PartialEq)]
enum Tok {
    S(String),
    Open,
    Close,
}

fn tokenize(src: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(c) = it.next() {
                    match c {
                        '\\' => match it.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(o) => s.push(o),
                            None => {}
                        },
                        '"' => break,
                        o => s.push(o),
                    }
                }
                out.push(Tok::S(s));
            }
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '/' if it.peek() == Some(&'/') => {
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            c => {
                // Unquoted token (rare, but legal).
                let mut s = String::from(c);
                while let Some(&n) = it.peek() {
                    if n.is_whitespace() || n == '{' || n == '}' || n == '"' {
                        break;
                    }
                    s.push(n);
                    it.next();
                }
                out.push(Tok::S(s));
            }
        }
    }
    out
}

pub fn parse(src: &str) -> Vdf {
    let toks = tokenize(src);
    let mut pos = 0;
    Vdf::Obj(parse_obj(&toks, &mut pos))
}

fn parse_obj(t: &[Tok], pos: &mut usize) -> BTreeMap<String, Vdf> {
    let mut m = BTreeMap::new();
    while *pos < t.len() {
        match &t[*pos] {
            Tok::Close => {
                *pos += 1;
                break;
            }
            Tok::S(k) => {
                let key = k.clone();
                *pos += 1;
                match t.get(*pos) {
                    Some(Tok::S(v)) => {
                        m.insert(key, Vdf::Str(v.clone()));
                        *pos += 1;
                    }
                    Some(Tok::Open) => {
                        *pos += 1;
                        m.insert(key, Vdf::Obj(parse_obj(t, pos)));
                    }
                    _ => break,
                }
            }
            Tok::Open => *pos += 1,
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_libraryfolders() {
        let src = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "123" "730" "999" }
	}
	"1" { "path" "D:\\SteamLibrary" }
}"#;
        let v = parse(src);
        let lf = v.get("libraryfolders").unwrap();
        let paths: Vec<_> = lf.entries().filter_map(|(_, e)| e.str("path")).collect();
        assert_eq!(paths, vec![r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]);
    }

    #[test]
    fn parses_acf_case_insensitive() {
        let v = parse(r#""AppState" { "appid" "730" "name" "Counter-Strike 2" // c
          "installdir" "Counter-Strike Global Offensive" }"#);
        let s = v.get("appstate").unwrap();
        assert_eq!(s.str("appid"), Some("730"));
        assert_eq!(s.str("name"), Some("Counter-Strike 2"));
    }
}
