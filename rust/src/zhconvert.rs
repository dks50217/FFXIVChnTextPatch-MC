//! 簡→繁（台灣正體＋台灣用語），等同 OpenCC s2twp 再加 FFXIV 專屬校訂。對應 C# ZhConvert。
//! 最長正向匹配，三輪：
//!   1. GPPhrases（FFXIV 例外詞彙）+ STPhrases + STCharacters（簡→繁，詞優先於單字）
//!   2. TWPhrases（台灣用語）
//!   3. TWVariants（台灣異體字）
//! UserPhrases.txt 掛在第 1、2 輪最前面，優先權最高。字典在 resource/opencc/。
//! 長度以 Unicode 字元計（C# 版以 UTF-16 計，只在 CJK 擴展 B 以後的字才有差，且不會把代理對切半）。
use crate::p;
use std::collections::HashMap;
use std::sync::OnceLock;

struct Round {
    dict: HashMap<String, String>,
    max_len: usize,
    /// 首字元 → 以它開頭的 key 有哪些長度（見 len_bit）。不在表裡的字直接放行
    /// （GPPhrases 有引號、英文開頭的 key，不能只看是不是 CJK）；在表裡的也只試存在的長度。
    lengths: HashMap<char, u64>,
}

/// 長度 n 對應的位元；63 字以上共用 bit 0（目前最長 39 字，用不到）。
fn len_bit(n: usize) -> u64 {
    1 << if n < 64 { n } else { 0 }
}

fn load(files: &[&str]) -> Round {
    let mut dict = HashMap::new();
    for file in files {
        // UserPhrases.txt 可有可無
        let Ok(text) = std::fs::read_to_string(p("resource/opencc").join(file)) else { continue };
        for line in text.trim_start_matches('\u{feff}').lines() {
            if line.starts_with('#') {
                continue;
            }
            let Some((key, values)) = line.split_once('\t') else { continue };
            if key.is_empty() {
                continue;
            }
            // 先載入的字典優先；一對多取第一個候選（OpenCC 預設）
            dict.entry(key.to_string()).or_insert_with(|| values.split(' ').next().unwrap_or("").to_string());
        }
    }
    let mut lengths = HashMap::new();
    for k in dict.keys() {
        *lengths.entry(k.chars().next().unwrap()).or_insert(0) |= len_bit(k.chars().count());
    }
    Round { max_len: dict.keys().map(|k| k.chars().count()).max().unwrap_or(1), lengths, dict }
}

fn rounds() -> &'static [Round; 3] {
    static ROUNDS: OnceLock<[Round; 3]> = OnceLock::new();
    ROUNDS.get_or_init(|| {
        [
            load(&["UserPhrases.txt", "GPPhrases.txt", "STPhrases.txt", "STCharacters.txt"]),
            load(&["UserPhrases.txt", "TWPhrases.txt"]),
            load(&["TWVariants.txt"]),
        ]
    })
}

pub fn s2tw(text: &str) -> String {
    rounds().iter().fold(text.to_string(), |s, r| apply(&s, r))
}

fn apply(s: &str, r: &Round) -> String {
    // 每個字元的起始 byte 位置，多一個結尾，方便用字元數切 &str 查表而不用配置字串
    let bounds: Vec<usize> = s.char_indices().map(|(b, _)| b).chain([s.len()]).collect();
    let chars = bounds.len() - 1;
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars {
        let here = &s[bounds[i]..bounds[i + 1]];
        if let Some(&mask) = r.lengths.get(&here.chars().next().unwrap()) {
            let hit = (1..=r.max_len.min(chars - i))
                .rev()
                .filter(|&len| mask & len_bit(len) != 0)
                .find_map(|len| r.dict.get(&s[bounds[i]..bounds[i + len]]).map(|to| (len, to)));
            if let Some((len, to)) = hit {
                out += to;
                i += len;
                continue;
            }
        }
        out += here;
        i += 1;
    }
    out
}
