//! 翻譯 CSV 檢查（--lint）：resource/rawexd 下的 CSV 會不會讓漢化中斷或整檔被跳過、翻譯覆蓋率、
//! 說話任務 SAYTODO 是否仍是中文，以及（遊戲路徑有效時）遊戲裡有字串欄位卻沒有 CSV 的表。
//! 報告寫到 lint-report.txt，回傳錯誤數當 exit code。對應 C# LintTool.Run。
//!
//! 規則照 C# 版（C# 版的 TextFieldParser 比 Rust 打補丁用的 csv crate 嚴格：引號沒關好會讓 C# 整檔跳過），
//! C# 版還在發行，CI 要擋的是兩邊都安全的 CSV。
use crate::config::Config;
use crate::crc::ffcrc_lower;
use crate::sqpack::{read_index, Dats};
use crate::update::csv_files;
use crate::{exd, exdnames, log, p, patch, R};
use std::path::Path;

pub fn run(cfg: &Config) -> usize {
    let rawexd = p("resource/rawexd");
    let mut errors = Vec::new();
    let mut say_todo_zh = Vec::new();
    let mut coverage: Vec<(String, usize, usize)> = Vec::new();

    // 1. 檢查所有 CSV
    let files = csv_files(&rawexd);
    for path in &files {
        let name = path.strip_prefix(&rawexd).unwrap().to_string_lossy().replace('\\', "/");
        let text = String::from_utf8_lossy(&std::fs::read(path).unwrap_or_default()).into_owned();
        lint_csv(text.trim_start_matches('\u{feff}'), &name, &mut errors, &mut say_todo_zh, &mut coverage);
    }

    // 2. 缺少 CSV 的表（需要有效的遊戲路徑）
    let game = cfg.get_or("GamePath", "");
    let (missing, missing_note) = if patch::is_ffxiv_folder(game) {
        match find_missing_sheets(game, &rawexd) {
            Ok(m) => (m, None),
            Err(e) => {
                let note = format!("讀取遊戲檔案失敗，略過缺表檢查：{e}");
                log(&format!("[Lint] {note}"));
                (Vec::new(), Some(note))
            }
        }
    } else {
        (Vec::new(), Some("遊戲路徑未設定或無效，略過缺表檢查。".to_string()))
    };

    // 3. 輸出報告
    let translated: usize = coverage.iter().map(|c| c.1).sum();
    let cells: usize = coverage.iter().map(|c| c.2).sum();
    let ratio = if cells == 0 { 0.0 } else { translated as f64 * 100.0 / cells as f64 };
    let none = |v: &[String]| if v.is_empty() { "（無）\n".to_string() } else { v.iter().map(|s| format!("{s}\n")).collect() };

    // ponytail: 不寫產生時間（std 沒有本地時間），看檔案修改時間即可
    let mut out = format!("翻譯 CSV 檢查報告\nCSV 檔數：{}\n\n", files.len());
    out += &format!("■ 錯誤（{}）—— 會中斷漢化流程或讓整檔被跳過\n{}\n", errors.len(), none(&errors));
    out += &format!("■ 說話任務仍是中文的 SAYTODO（{}）—— 國際服不建議打中文，需改成英文\n{}\n", say_todo_zh.len(), none(&say_todo_zh));
    out += &format!("■ 遊戲中有字串欄位但缺少 CSV 的表（{}）—— 這些表會維持原文\n", missing.len());
    out += &match &missing_note {
        Some(note) => format!("{note}\n{}", missing.iter().map(|s| format!("{s}\n")).collect::<String>()),
        None => none(&missing),
    };
    out += &format!("\n■ 覆蓋率（非空欄位 / 總欄位），總計 {translated}/{cells}（{}%）\n", f1(ratio));
    let share = |c: &(String, usize, usize)| if c.2 == 0 { 1.0 } else { c.1 as f64 / c.2 as f64 };
    coverage.sort_by(|a, b| share(a).total_cmp(&share(b))); // 穩定排序，同比例維持檔案順序
    for (name, t, total) in &coverage {
        let r = if *total == 0 { 100.0 } else { *t as f64 * 100.0 / *total as f64 };
        out += &format!("{:>6}%  {t}/{total}  {}\n", f1(r), exdnames::label(name));
    }
    if let Err(e) = std::fs::write(p("lint-report.txt"), out) {
        log(&format!("寫入 lint-report.txt 失敗：{e}"));
    }
    println!("檢查完成：{} 個錯誤、缺 {} 張表、覆蓋率 {}%（詳見 lint-report.txt）", errors.len(), missing.len(), f1(ratio));
    errors.len()
}

/// 一位小數，四捨五入（.NET "0.0" 的 midpoint 是遠離零，Rust 的 {:.1} 是四捨六入五成雙）。
fn f1(x: f64) -> String {
    format!("{:.1}", (x * 10.0).round() / 10.0)
}

fn lint_csv(text: &str, name: &str, errors: &mut Vec<String>, say_todo_zh: &mut Vec<String>, coverage: &mut Vec<(String, usize, usize)>) {
    let rows = match read_records(text) {
        Ok(rows) => rows,
        Err(line) => {
            errors.push(format!("{name}: CSV 格式錯誤（第 {line} 行）→ 整檔會被跳過"));
            return;
        }
    };
    if rows.len() < 2 {
        errors.push(format!("{name}: 列數不足（{}）→ 整檔會被跳過", rows.len()));
        return;
    }
    // 有效列第 2 列（index 1）是 offset 列，欄位必須是整數
    if let Some((i, v)) = rows[1].1.iter().enumerate().skip(1).find(|(_, v)| !is_int(v)) {
        errors.push(format!("{name}: offset 列第 {} 欄不是整數（\"{v}\"）→ 整檔會被跳過", i + 1));
        return;
    }
    if rows.len() < 4 {
        return; // 只有表頭沒有資料列（空表）：漢化流程視為無翻譯，不是錯誤
    }
    let width = rows[1].1.len();
    let (mut translated, mut total) = (0, 0);
    for (line, row) in &rows[3..] {
        if !is_int(&row[0]) {
            errors.push(format!("{name} 第 {line} 行: key \"{}\" 不是整數 → 整檔會被跳過", row[0]));
            return;
        }
        if row.len() < width {
            errors.push(format!("{name} 第 {line} 行: 欄位數 {} 少於 offset 列的 {width} → 漢化可能中斷", row.len()));
        }
        // 說話任務：SAYTODO 是玩家實際要輸入的字，國際服不建議打中文
        if row.iter().any(|c| c.to_ascii_uppercase().contains("_SAYTODO_")) {
            if let Some(zh) = row.iter().find(|c| !c.to_ascii_uppercase().contains("SAYTODO") && has_cjk(c)) {
                say_todo_zh.push(format!("{name} 第 {line} 行: 「{}」", truncate(zh)));
            }
        }
        for (c, cell) in row.iter().enumerate().skip(1) {
            total += 1;
            if !cell.is_empty() {
                translated += 1;
                check_hex_tags(cell, &format!("{name} 第 {line} 行第 {} 欄", c + 1), errors);
            }
        }
    }
    coverage.push((name.to_string(), translated, total));
}

/// 跟 .NET int.TryParse 一樣容許前後空白。
fn is_int(s: &str) -> bool {
    s.trim().parse::<i32>().is_ok()
}

/// 依打補丁時 <hex:> 的實際處理，找出會讓它出錯或寫出壞資料的標籤。
fn check_hex_tags(cell: &str, location: &str, errors: &mut Vec<String>) {
    let mut tag_start: Option<usize> = None;
    for (i, c) in cell.char_indices() {
        if c == '<' && cell[i + 1..].starts_with("hex") {
            if tag_start.is_some() {
                errors.push(format!("{location}: 巢狀 hex 標籤（TagInTag）→ 會中斷漢化"));
                return;
            }
            tag_start = Some(i);
        } else if c == '>' {
            if let Some(start) = tag_start.take() {
                let tag = &cell[start..=i];
                if tag.len() < 6 || tag.as_bytes()[4] != b':' {
                    errors.push(format!("{location}: hex 標籤格式錯誤（缺少冒號）\"{}\" → 會中斷漢化", truncate(tag)));
                } else {
                    let body = &tag[5..tag.len() - 1];
                    if body.len() % 2 != 0 || !body.chars().all(|c| c.is_ascii_hexdigit()) {
                        errors.push(format!("{location}: hex 內容無效 \"{}\" → 會中斷漢化", truncate(tag)));
                    }
                }
            }
        }
    }
    if tag_start.is_some() {
        errors.push(format!("{location}: hex 標籤沒有關閉的 '>' → 標籤會被當成一般文字寫入（遊戲顯示錯誤）"));
    }
}

fn truncate(s: &str) -> String {
    match s.char_indices().nth(40) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

/// 讀 CSV 記錄並附上行號，規則仿 C# 的 TextFieldParser：'#' 開頭的行是註解、空行略過、引號欄位可跨行。
/// 行號也照它的算法：記錄前若有註解/空行，報的是那些行的第一行（rawexd 只有第 2 行是註解，資料列不受影響）。
/// 引號欄位的結尾引號後面只能接空白再接逗號或行尾，否則、或檔案結束時引號還沒關，回傳 Err(該記錄的行號)。
fn read_records(t: &str) -> Result<Vec<(usize, Vec<String>)>, usize> {
    let b = t.as_bytes();
    let n = b.len();
    let (mut i, mut line) = (0, 1);
    let mut recs = Vec::new();
    // 吃掉一個換行（\r\n、\n 或 \r），回傳是否吃到
    let eol = |i: &mut usize, line: &mut usize| -> bool {
        match b.get(*i) {
            Some(b'\r') => {
                *i += if b.get(*i + 1) == Some(&b'\n') { 2 } else { 1 };
            }
            Some(b'\n') => *i += 1,
            _ => return false,
        }
        *line += 1;
        true
    };
    while i < n {
        let reported = line;
        // 略過註解與空白行
        loop {
            let end = (i..n).find(|&e| b[e] == b'\r' || b[e] == b'\n').unwrap_or(n);
            if i < n && (b[i] == b'#' || b[i..end].iter().all(|&c| c == b' ' || c == b'\t')) {
                i = end;
                if !eol(&mut i, &mut line) {
                    break;
                }
            } else {
                break;
            }
        }
        if i >= n {
            break;
        }
        let start_line = line;
        let mut fields = Vec::new();
        loop {
            let mut f = Vec::new();
            let lead = (i..n).find(|&e| b[e] != b' ' && b[e] != b'\t').unwrap_or(n);
            if lead < n && b[lead] == b'"' {
                i = lead + 1;
                loop {
                    if i >= n {
                        return Err(start_line); // 引號沒關
                    }
                    if b[i] == b'"' && b.get(i + 1) == Some(&b'"') {
                        f.push(b'"');
                        i += 2;
                    } else if b[i] == b'"' {
                        i += 1;
                        while i < n && (b[i] == b' ' || b[i] == b'\t') {
                            i += 1;
                        }
                        if i < n && !matches!(b[i], b',' | b'\r' | b'\n') {
                            return Err(start_line); // 結尾引號後面還有東西
                        }
                        break;
                    } else {
                        if matches!(b[i], b'\r' | b'\n') {
                            line += 1;
                            if b[i] == b'\r' && b.get(i + 1) == Some(&b'\n') {
                                f.push(b'\r');
                                i += 1;
                            }
                        }
                        f.push(b[i]);
                        i += 1;
                    }
                }
            } else {
                while i < n && !matches!(b[i], b',' | b'\r' | b'\n') {
                    f.push(b[i]);
                    i += 1;
                }
            }
            fields.push(String::from_utf8_lossy(&f).into_owned());
            if i < n && b[i] == b',' {
                i += 1;
                continue;
            }
            break;
        }
        eol(&mut i, &mut line);
        recs.push((reported, fields));
    }
    Ok(recs)
}

/// 遊戲裡有字串欄位、但 resource/rawexd 沒有對應 CSV 的表。
fn find_missing_sheets(game: &str, rawexd: &Path) -> R<Vec<String>> {
    let index_path = Path::new(game).join("game/sqpack/ffxiv/0a0000.win32.index").to_string_lossy().to_string();
    let index = read_index(&index_path)?;
    let mut dats = Dats::new(&index_path);
    let mut missing = Vec::new();
    for sheet in patch::init_file_list(&index, &mut dats)? {
        if rawexd.join(format!("{sheet}.csv")).is_file() {
            continue;
        }
        let (dir, name) = sheet.rsplit_once('/').map_or(("exd".to_string(), sheet.as_str()), |(d, n)| (format!("exd/{d}"), n));
        let Some(entry) = index.get(&ffcrc_lower(&dir)).and_then(|f| f.get(&ffcrc_lower(&format!("{name}.exh")))) else { continue };
        let Ok(exh) = dats.extract(entry.data_offset).and_then(|d| exd::parse_exh(&d)) else { continue };
        if exh.lang_count == 0 {
            continue; // 無語言版本的表，漢化流程本來就跳過
        }
        let strings = exh.datasets.iter().filter(|d| d.0 == 0).count();
        if strings > 0 {
            missing.push(format!("{}，{strings} 個字串欄位", exdnames::label(&sheet)));
        }
    }
    missing.sort_by_key(|s| s.to_uppercase()); // C# 的 OrdinalIgnoreCase
    Ok(missing)
}
