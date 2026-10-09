//! Sheet 標籤參照檢查（--sheetsig）：譯文的 Sheet 標籤參數（去哪張表、讀第幾欄）必須跟日文原文一致，
//! 照陸版欄號翻過來會讓該介面閃退（2026-09 莫古莫古指南書，Addon row 15919）。
//! 只比 Sheet：換行、顏色、If 分支長度本來就會隨譯文變動。參照檔 resource/ja-sheetsig.txt.gz
//! 由 C# 版 --gensheetsig 從 SaintCoinach 日文匯出產生，這裡只讀，CI 不需要遊戲。對應 C# SheetSig.Check。
use crate::merge::{parse, rows};
use crate::update::csv_files;
use crate::{base_dir, log, p, patch::game_version, config::Config};
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::io::Read;

const REF_NAME: &str = "ja-sheetsig.txt.gz";

/// Sheet 系列的 TagType（見 C# HexTagTool 對照表）。
const SHEET_TAGS: [&str; 5] = ["28", "30", "31", "32", "33"];

/// 取出一格裡的 Sheet 標籤與其參數片段，以 | 串成簽章。
/// 匯出會把一個標籤切成數段 <hex:…>：02 開頭是標籤起頭，其餘是參數片段。
pub fn of(cell: &str) -> String {
    let mut parts = Vec::new();
    let mut collecting = false;
    let mut rest = cell;
    // 等同 regex <hex:([0-9A-Fa-f]*)>
    while let Some(at) = rest.find("<hex:") {
        let body = &rest[at + 5..];
        let len = body.bytes().take_while(u8::is_ascii_hexdigit).count();
        if body.as_bytes().get(len) != Some(&b'>') {
            rest = &rest[at + 1..];
            continue;
        }
        let h = body[..len].to_ascii_uppercase();
        if h.starts_with("02") && h.len() >= 4 {
            collecting = SHEET_TAGS.contains(&&h[2..4]);
            if collecting {
                parts.push(h);
            }
        } else if collecting {
            parts.push(h);
        }
        rest = &body[len + 1..];
    }
    parts.join("|")
}

/// rawexd CSV → 非空儲存格 ((rowId, 欄index), 內容)，照第一次出現的順序（同 key 後蓋前，跟 C# Dictionary 一樣）。
fn cells(csv: &str) -> Vec<((i32, usize), String)> {
    let mut out: Vec<((i32, usize), String)> = Vec::new();
    let mut at: HashMap<(i32, usize), usize> = HashMap::new();
    let recs = parse(csv.trim_start_matches('\u{feff}'));
    for f in rows(&recs).iter().skip(3) {
        let Ok(row) = f[0].trim().parse::<i32>() else { continue };
        for (c, cell) in f.iter().enumerate().skip(1).filter(|(_, s)| !s.is_empty()) {
            let key = (row, c - 1);
            match at.get(&key) {
                Some(&i) => out[i].1 = cell.clone(),
                None => {
                    at.insert(key, out.len());
                    out.push((key, cell.clone()));
                }
            }
        }
    }
    out
}

/// 從 SaintCoinach 日文匯出（<版本>/rawexd）產生 resource/ja-sheetsig.txt.gz。需要本機有遊戲，每個遊戲版本重產一次。
pub fn generate(ja_dir: &std::path::Path) -> crate::R<String> {
    use std::io::Write;
    if !ja_dir.is_dir() {
        return Err(format!("找不到日文匯出目錄：{}", ja_dir.display()).into());
    }
    // SaintCoinach 輸出在 <版本>/rawexd 底下
    let ja_dir = ja_dir.canonicalize()?;
    let version = ja_dir.parent().and_then(|d| d.file_name()).map_or("unknown".into(), |n| n.to_string_lossy().to_string());
    let mut out = format!("# ja-sheetsig v1  game={version}  generated={}\n", &crate::now()[..10]);
    out += "# <csv>,<row>,<col>,<Sheet 標籤與參數片段，以 | 分隔>\n";
    let files = csv_files(&ja_dir);
    let mut count = 0;
    for (i, path) in files.iter().enumerate() {
        let rel = path.strip_prefix(&ja_dir)?.to_string_lossy().replace('\\', "/");
        crate::progress((i + 1) as f32 / files.len() as f32, "正在產生參照檔：", &rel);
        let Ok(bytes) = std::fs::read(path) else {
            log(&format!("[SheetSig] 讀取失敗 {rel}"));
            continue;
        };
        let mut sigs = cells(&String::from_utf8_lossy(&bytes));
        sigs.sort_by_key(|(key, _)| *key);
        for ((row, col), cell) in sigs {
            let sig = of(&cell);
            if !sig.is_empty() {
                out += &format!("{rel},{row},{col},{sig}\n");
                count += 1;
            }
        }
    }
    let path = p("resource").join(REF_NAME);
    let mut gz = flate2::write::GzEncoder::new(std::fs::File::create(&path)?, flate2::Compression::best());
    gz.write_all(out.as_bytes())?;
    gz.finish()?;
    let kb = std::fs::metadata(&path)?.len() / 1024;
    log(&format!("[SheetSig] {count} 格、{kb}KB → {}", path.display()));
    Ok(format!("完成：{count} 格含 Sheet 標籤，{kb}KB → resource/{REF_NAME}（遊戲版本 {version}）"))
}

/// 讀參照檔：(遊戲版本, "<csv>,<row>,<col>" → 簽章)。
fn load_ref() -> Option<(String, HashMap<String, String>)> {
    let mut text = String::new();
    GzDecoder::new(std::fs::File::open(p("resource").join(REF_NAME)).ok()?).read_to_string(&mut text).ok()?;
    let mut version = "unknown".to_string();
    let mut sigs = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') {
            if let Some(v) = line.split("game=").nth(1).and_then(|v| v.split_whitespace().next()) {
                version = v.to_string();
            }
            continue;
        }
        // <csv>,<row>,<col>,<sig>；簽章不含逗號
        let mut commas = line.match_indices(',').map(|(i, _)| i);
        if let (Some(_), Some(_), Some(c)) = (commas.next(), commas.next(), commas.next()) {
            sigs.insert(line[..c].to_string(), line[c + 1..].to_string());
        }
    }
    Some((version, sigs))
}

/// base_ref 有給就只檢查相對它（的 merge-base）有變動的格子，沒給就掃全部。回傳不符的格數。
pub fn check(cfg: &Config, base_ref: Option<&str>) -> usize {
    let mut report = format!("Sheet 標籤參照檢查  {}\n", crate::now());
    let Some((ref_version, sigs)) = load_ref() else {
        report += &format!("找不到 resource/{REF_NAME}，略過檢查。\n");
        report += "請在本機用 C# 版 --gensheetsig <SaintCoinach 日文匯出的 rawexd 目錄> 產生後 commit。\n";
        write(&report);
        return 0;
    };
    report += &format!("參照檔遊戲版本：{ref_version}（{} 格）\n", sigs.len());
    let game = game_version(cfg);
    if !game.is_empty() && game != ref_version {
        report += &format!("⚠ 參照檔是 {ref_version} 產的，本機遊戲是 {game}，結果可能不準，建議重新產生參照檔。\n");
    }

    let rawexd = p("resource/rawexd");
    let mut base = None;
    let targets: Vec<String> = match base_ref {
        Some(base_ref) => {
            // 對 merge-base，避免 base 前進後把別人的改動也算進來；比工作區而非 HEAD，commit 前就能跑
            let merge_base = git(&["merge-base", base_ref, "HEAD"]).map(|s| s.trim().to_string()).unwrap_or(base_ref.to_string());
            let Some(changed) = git(&["diff", "--name-only", &merge_base, "--", "resource/rawexd"]) else {
                report += &format!("⚠ git diff 失敗（base ref: {base_ref}），略過檢查。\n");
                write(&report);
                return 0;
            };
            let t: Vec<String> = changed.lines().map(str::trim).filter(|l| l.ends_with(".csv")).map(String::from).collect();
            report += &format!("比對範圍：相對 {merge_base} 有變動的 {} 個 CSV\n", t.len());
            base = Some(merge_base);
            t
        }
        None => {
            let t: Vec<String> = csv_files(&rawexd)
                .iter()
                .map(|f| format!("resource/rawexd/{}", f.strip_prefix(&rawexd).unwrap().to_string_lossy().replace('\\', "/")))
                .collect();
            report += &format!("比對範圍：全部 {} 個 CSV\n", t.len());
            t
        }
    };

    let mut hits = Vec::new();
    let mut checked = 0;
    for repo_path in &targets {
        let rel = repo_path.trim_start_matches("resource/rawexd/");
        let Ok(bytes) = std::fs::read(rawexd.join(rel)) else { continue };
        let now = cells(&String::from_utf8_lossy(&bytes));
        // diff 模式只看值真的變了的格子；base 沒有這個檔就全算新的
        let before: Option<HashMap<(i32, usize), String>> =
            base.as_ref().map(|b| git(&["show", &format!("{b}:{repo_path}")]).map(|t| cells(&t).into_iter().collect()).unwrap_or_default());
        for ((row, col), cell) in &now {
            if before.as_ref().is_some_and(|b| b.get(&(*row, *col)) == Some(cell)) {
                continue;
            }
            let sig = of(cell);
            if sig.is_empty() {
                continue;
            }
            checked += 1;
            if let Some(ja) = sigs.get(&format!("{rel},{row},{col}")).filter(|ja| **ja != sig) {
                hits.push(format!("{rel}  row {row} 欄{col}\n    日文原文：{ja}\n    本地譯文：{sig}"));
            }
        }
    }
    report += &format!("檢查了 {checked} 格含 Sheet 標籤的譯文\n\n");
    report += &format!("■ Sheet 參照與日文原文不符（{}）—— 可能讓該介面閃退，請逐格確認\n", hits.len());
    if hits.is_empty() {
        report += "（無）\n";
    }
    for h in &hits {
        report += h;
        report.push('\n');
    }
    write(&report);
    log(&format!("[SheetSig] {} 處不符", hits.len()));
    hits.len()
}

fn write(report: &str) {
    if let Err(e) = std::fs::write(p("sheetsig-report.txt"), report) {
        log(&format!("寫入 sheetsig-report.txt 失敗：{e}"));
    }
}

/// 在 repo 根目錄跑 git 拿 stdout，失敗回 None。
fn git(args: &[&str]) -> Option<String> {
    let out = crate::command("git").args(args).current_dir(base_dir()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}
