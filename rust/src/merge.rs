//! rawexd CSV 逐儲存格合併：本地非空儲存格永遠保留，本地空格/缺列才補上游內容。對應 C# RawexdMerge。
//! 輸出以上游的欄位配置為準（offset 列對齊，跨版本欄位增減也對得回去），本地獨有的列附加在檔尾。
use crate::R;
use std::collections::{HashMap, HashSet};

/// 一筆 CSV 記錄：'#' 開頭的註解列（原樣保留）或一般欄位列。
pub enum Rec {
    Comment(String),
    Fields(Vec<String>),
}

pub struct Merged {
    pub text: String,
    pub filled: usize,
    pub new_rows: usize,
    pub headers_changed: bool,
}

/// 只取欄位列。有效列 [0]=key 標頭 [1]=offset [2]=型別 [3..]=資料。
pub fn rows(recs: &[Rec]) -> Vec<&Vec<String>> {
    recs.iter().filter_map(|r| match r { Rec::Fields(f) => Some(f), _ => None }).collect()
}

pub fn merge(local: &str, upstream: &str, nl: &str) -> R<Merged> {
    let (lo_recs, up_recs) = (parse(local), parse(upstream));
    let (lo, up) = (rows(&lo_recs), rows(&up_recs));
    if lo.len() < 3 || up.len() < 3 {
        return Err("CSV 缺少標頭列".into());
    }
    // offset 值 → 本地欄索引，跨版本欄位對齊用
    let lo_off: HashMap<&str, usize> = lo[1].iter().enumerate().skip(1).map(|(j, o)| (o.as_str(), j)).collect();

    // 配對鍵：offset-0 欄若是穩定 id（CtsWks 的 TEXT_… 這種）就用它，否則退回位序 key（第 0 欄）。
    // 位序配對時上游在中間插一列，之後每列的譯文都會被貼到錯的列上（009 那批 CtsWks 漂移的成因）。
    let (lo_id, up_id) = (key_column(&lo), key_column(&up));
    let by_id = matches!((lo_id, up_id), (Some(l), Some(u)) if qualifies_as_id(&lo, &up, l, u));
    let key = |r: &[String], col: Option<usize>| -> String {
        match col.filter(|&c| by_id && c < r.len()) {
            Some(c) => r[c].clone(),
            None => r[0].clone(),
        }
    };
    let lo_data: HashMap<String, &Vec<String>> = lo[3..].iter().map(|r| (key(r, lo_id), *r)).collect();

    let headers_changed = (0..3).any(|i| lo[i] != up[i]);
    let (mut filled, mut new_rows, mut up_idx) = (0, 0, 0);
    let mut seen = HashSet::new();
    let mut out = String::with_capacity(upstream.len() + local.len() / 4);

    for rec in &up_recs {
        let fields = match rec {
            Rec::Comment(c) => {
                out += c;
                out += nl;
                continue;
            }
            Rec::Fields(f) => f,
        };
        up_idx += 1;
        if up_idx <= 3 {
            write_row(&mut out, fields, nl);
            continue;
        }
        let k = key(fields, up_id);
        match lo_data.get(&k) {
            Some(lrow) => {
                let mut merged = fields.clone();
                // 有些列有多餘尾逗號（欄數比 offset 列多），超出的欄原樣放行
                for j in 1..merged.len().min(up[1].len()) {
                    match lo_off.get(up[1][j].as_str()).and_then(|&lj| lrow.get(lj)).filter(|s| !s.is_empty()) {
                        Some(l) => merged[j] = l.clone(),
                        None if !merged[j].is_empty() => filled += 1,
                        None => {}
                    }
                }
                write_row(&mut out, &merged, nl);
            }
            None => {
                new_rows += 1;
                write_row(&mut out, fields, nl);
            }
        }
        seen.insert(k);
    }

    // 本地獨有的列：依 offset 重排進上游欄位配置後附加在檔尾
    for r in &lo[3..] {
        if seen.contains(&key(r, lo_id)) {
            continue;
        }
        let mut row = vec![r[0].clone()];
        row.extend(up[1][1..].iter().map(|o| lo_off.get(o.as_str()).and_then(|&lj| r.get(lj)).cloned().unwrap_or_default()));
        write_row(&mut out, &row, nl);
    }
    Ok(Merged { text: out, filled, new_rows, headers_changed })
}

/// 第一個 offset 值為 "0" 的資料欄（慣例上緊接 key）。
pub fn key_column(rows: &[&Vec<String>]) -> Option<usize> {
    rows[1].iter().skip(1).position(|o| o == "0").map(|j| j + 1)
}

/// offset-0 欄能否當配對鍵：上游該欄每列都非空、唯一、純 ASCII（真 id 是 TEXT_… 這種；
/// 若是被翻譯的中文欄，簡繁高度重疊會騙過重疊檢查），且本地/上游過半對得上。
pub fn qualifies_as_id(lo: &[&Vec<String>], up: &[&Vec<String>], lo_col: usize, up_col: usize) -> bool {
    let mut ids = HashSet::new();
    for r in &up[3..] {
        match r.get(up_col) {
            Some(v) if !v.is_empty() && !v.chars().any(|c| c > '~') && ids.insert(v.as_str()) => {}
            _ => return false,
        }
    }
    let overlap = lo[3..].iter().filter(|r| r.get(lo_col).is_some_and(|v| ids.contains(v.as_str()))).count();
    !ids.is_empty() && overlap * 2 >= ids.len().min(lo.len() - 3)
}

fn write_row(out: &mut String, fields: &[String], nl: &str) {
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        if f.contains(['"', ',', '\n', '\r']) || f.starts_with('#') {
            out.push('"');
            *out += &f.replace('"', "\"\"");
            out.push('"');
        } else {
            *out += f;
        }
    }
    *out += nl;
}

/// 極簡 CSV parser：引號欄位（含逗號/換行/雙引號跳脫），行首 '#' 為註解，空行略過。
/// 跟 C# 版同一套規則；引號欄位裡的空行會保留（TextFieldParser 會吃掉，這裡不會）。
pub fn parse(t: &str) -> Vec<Rec> {
    let b = t.as_bytes();
    let (n, mut i) = (b.len(), 0);
    let mut recs = Vec::new();
    // 分隔字元都是 ASCII，只在 ASCII 位置切，切出來的 bytes 一定是合法 UTF-8
    let text = |v: Vec<u8>| String::from_utf8(v).unwrap();
    while i < n {
        match b[i] {
            b'\r' | b'\n' => i += 1,
            b'#' => {
                let e = (i..n).find(|&e| b[e] == b'\r' || b[e] == b'\n').unwrap_or(n);
                recs.push(Rec::Comment(t[i..e].to_string()));
                i = e;
            }
            _ => {
                let mut fields = Vec::new();
                loop {
                    let mut f = Vec::new();
                    if i < n && b[i] == b'"' {
                        i += 1;
                        while i < n {
                            if b[i] == b'"' && b.get(i + 1) == Some(&b'"') {
                                f.push(b'"');
                                i += 2;
                            } else if b[i] == b'"' {
                                i += 1;
                                break;
                            } else {
                                f.push(b[i]);
                                i += 1;
                            }
                        }
                    }
                    while i < n && !matches!(b[i], b',' | b'\r' | b'\n') {
                        f.push(b[i]);
                        i += 1;
                    }
                    fields.push(text(f));
                    if i < n && b[i] == b',' {
                        i += 1;
                        continue;
                    }
                    break;
                }
                recs.push(Rec::Fields(fields));
            }
        }
    }
    recs
}
