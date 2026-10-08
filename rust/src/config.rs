//! 讀寫 conf/global.properties（Java Properties 相容：\: \\ \uXXXX 跳脫，UTF-8）。與 C# 版共用同一檔。
use std::path::{Path, PathBuf};

pub struct Config {
    path: PathBuf,
    props: Vec<(String, String)>, // 保留原檔順序
}

impl Config {
    pub fn load(path: &Path) -> Config {
        let mut cfg = Config { path: path.to_path_buf(), props: Vec::new() };
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for raw in text.trim_start_matches('\u{feff}').lines() {
            let line = raw.trim_start();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            if let Some(sep) = find_separator(line) {
                cfg.set(&unescape(line[..sep].trim()), &unescape(&line[sep + 1..]));
            }
        }
        cfg
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.props.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    pub fn set(&mut self, key: &str, value: &str) {
        match self.props.iter_mut().find(|(k, _)| k == key) {
            Some(kv) => kv.1 = value.to_string(),
            None => self.props.push((key.to_string(), value.to_string())),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut out = String::from("#FFXIVChnTextPatch\n");
        for (k, v) in &self.props {
            out += &format!("{}={}\n", escape(k), escape(v));
        }
        std::fs::write(&self.path, out)
    }
}

fn find_separator(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'=' | b':' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' || it.peek().is_none() {
            out.push(c);
            continue;
        }
        match it.next().unwrap() {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'u' => {
                let hex: String = it.clone().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) if hex.len() == 4 => {
                        out.push(ch);
                        it.nth(3);
                    }
                    _ => out.push('u'),
                }
            }
            n => out.push(n),
        }
    }
    out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | ':' | '=' | '#' | '!') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
