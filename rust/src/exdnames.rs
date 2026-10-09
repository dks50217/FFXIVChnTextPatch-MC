//! EXD 表名 → 遊戲內文本位置說明，資料在 conf/exd-names.csv。查無說明時退回原名，缺檔也不影響功能。對應 C# ExdNames。
use crate::p;
use std::collections::HashMap;
use std::sync::OnceLock;

/// key 一律小寫（C# 版用 OrdinalIgnoreCase）。資料夾 key 以 / 結尾，避免 quest/ 與 Quest 撞名。
fn map() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let text = std::fs::read_to_string(p("conf/exd-names.csv")).unwrap_or_default();
        text.trim_start_matches('\u{feff}')
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once(','))
            .map(|(name, desc)| (name.trim().to_lowercase(), desc.trim().to_string()))
            .filter(|(name, desc)| !name.is_empty() && !desc.is_empty())
            .collect()
    })
}

/// 接受 "Item"、"Item.csv"、"EXD/Item.EXH"、"quest/000/xxx"、"quest/" 等形式；子目錄檔案退回第一段資料夾的說明。
pub fn describe(sheet: &str) -> Option<&'static str> {
    let s = sheet.replace('\\', "/");
    let mut s = s.trim_start_matches('/');
    if s.len() >= 4 && s[..4].eq_ignore_ascii_case("exd/") {
        s = &s[4..];
    }
    if let Some(dot) = s.rfind('.').filter(|&d| d > 0) {
        s = &s[..dot];
    }
    let s = s.to_lowercase();
    let m = map();
    m.get(&s).or_else(|| s.find('/').filter(|&i| i > 0).and_then(|i| m.get(&s[..=i]))).map(String::as_str)
}

/// "Item" → "Item（道具）"；查無說明時原樣回傳。
pub fn label(name: &str) -> String {
    match describe(name) {
        Some(desc) => format!("{name}（{desc}）"),
        None => name.to_string(),
    }
}
