//! 漢化主流程：備份 → 字型替換 → CSV 文本替換，以及還原。對應 C# 的 PatchService。
//! 只支援 CSV 翻譯來源（FLanguage=CSV）。
use crate::config::Config;
use crate::crc::ffcrc_lower;
use crate::sqpack::{build_block, build_tex_block, read_index, Dats};
use crate::{exd, log, p, R};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const RESOURCE_NAMES: [&str; 6] = [
    "000000.win32.dat0", "000000.win32.index", "000000.win32.index2",
    "0a0000.win32.dat0", "0a0000.win32.index", "0a0000.win32.index2",
];

pub fn is_ffxiv_folder(path: &str) -> bool {
    !path.is_empty() && Path::new(path).join("game/ffxiv_dx11.exe").is_file()
}

fn sqpack_folder(game_path: &str) -> PathBuf {
    Path::new(game_path).join("game/sqpack/ffxiv")
}

fn is_game_running() -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq ffxiv_dx11.exe", "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("ffxiv_dx11.exe"))
        .unwrap_or(false)
}

pub fn game_version(cfg: &Config) -> String {
    let ver = Path::new(cfg.get_or("GamePath", "")).join("game/ffxivgame.ver");
    fs::read_to_string(ver).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// 六個資源檔的「大小:修改時間」指紋。時間用 .NET ticks，跟 C# 版寫進同一個 PatchedStamp 才比得起來。
fn file_stamp(cfg: &Config) -> String {
    let game = cfg.get_or("GamePath", "");
    if !is_ffxiv_folder(game) {
        return String::new();
    }
    RESOURCE_NAMES
        .iter()
        .map(|n| match fs::metadata(sqpack_folder(game).join(n)) {
            Ok(m) => {
                let since_epoch = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).unwrap_or_default();
                format!("{}:{}", m.len(), 621_355_968_000_000_000 + since_epoch.as_nanos() / 100)
            }
            Err(_) => "-".to_string(),
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn check_game(cfg: &Config, action: &str) -> R<PathBuf> {
    let game = cfg.get_or("GamePath", "");
    if !is_ffxiv_folder(game) {
        return Err("請選擇正確的遊戲根目錄（目錄內應有 game\\ffxiv_dx11.exe）".into());
    }
    if is_game_running() {
        return Err(format!("偵測到 FFXIV 正在執行中，請先關閉遊戲再進行{action}").into());
    }
    Ok(sqpack_folder(game))
}

pub fn patch(cfg: &mut Config) -> R<String> {
    let folder = check_game(cfg, "漢化")?;
    fs::create_dir_all(p("backup"))?;
    for n in RESOURCE_NAMES {
        if folder.join(n).is_file() {
            fs::copy(folder.join(n), p("backup").join(n))?;
        }
    }
    cfg.set("BackupVersion", &game_version(cfg));

    if cfg.get("ReplaFont") == Some("1") {
        replace_font(&folder.join("000000.win32.index"), &p("resource/font"))?;
    } else {
        log("Skip replacing font files.");
    }
    let mut summary = "漢化完畢".to_string();
    if cfg.get("ReplaText") == Some("1") {
        if cfg.get("FLanguage") != Some("CSV") || !has_csv_files(&p("resource/rawexd")) {
            return Err("找不到 CSV 翻譯檔（resource/rawexd），或 FLanguage 不是 CSV。此版本僅支援 CSV 模式。".into());
        }
        summary = replace_exdf(cfg, &folder.join("0a0000.win32.index"))?;
    } else {
        log("Skip replacing text.");
    }
    cfg.set("PatchedVersion", &game_version(cfg));
    cfg.set("PatchedStamp", &file_stamp(cfg));
    cfg.save()?;
    Ok(summary)
}

pub fn rollback(cfg: &mut Config) -> R<String> {
    let folder = check_game(cfg, "還原")?;
    // 備份只對備份當下的遊戲版本有效；版本不符時還原會把遊戲更新蓋掉
    let (backup, game) = (cfg.get_or("BackupVersion", "").to_string(), game_version(cfg));
    if !backup.is_empty() && !game.is_empty() && backup != game {
        return Err(format!(
            "備份是遊戲 {backup} 版的檔案，但目前遊戲已更新至 {game}。還原會把遊戲更新內容蓋掉，已取消。遊戲更新後漢化已自動失效，直接重新漢化即可。"
        )
        .into());
    }
    for n in RESOURCE_NAMES {
        if p("backup").join(n).is_file() {
            log(&format!("[Rollback] {n}"));
            fs::copy(p("backup").join(n), folder.join(n))?;
        }
    }
    cfg.set("PatchedVersion", "");
    cfg.set("PatchedStamp", "");
    cfg.save()?;
    Ok("還原完畢".into())
}

// ponytail: 只看第一層；rawexd 根目錄一定有 Addon.csv 這類檔案
fn has_csv_files(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|it| it.flatten().any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("csv"))))
        .unwrap_or(false)
}

/// 開 index 與 dat0：index 整份讀進記憶體改完再寫回，dat0 只追加。
struct Writer {
    index_path: PathBuf,
    index: Vec<u8>,
    dat: BufWriter<fs::File>,
    dat_len: u64,
}

impl Writer {
    fn open(index_path: &Path) -> R<Writer> {
        let dat_path = index_path.to_string_lossy().replace("index", "dat0");
        let dat = OpenOptions::new().append(true).open(dat_path)?;
        let dat_len = dat.metadata()?.len();
        Ok(Writer { index_path: index_path.into(), index: fs::read(index_path)?, dat: BufWriter::new(dat), dat_len })
    }

    /// 把 block 接到 dat0 尾端，index 該 entry 改指過去；回傳寫進 index 的原始 offset 值。
    fn append(&mut self, entry_pt: usize, block: &[u8]) -> R<u32> {
        let raw = (self.dat_len / 8) as u32;
        self.index[entry_pt + 8..entry_pt + 12].copy_from_slice(&raw.to_le_bytes());
        self.dat.write_all(block)?;
        self.dat_len += block.len() as u64;
        Ok(raw)
    }

    fn finish(mut self) -> R<()> {
        self.dat.flush()?;
        fs::write(&self.index_path, &self.index)?;
        Ok(())
    }
}

fn replace_font(index_path: &Path, font_dir: &Path) -> R<()> {
    let index = read_index(&index_path.to_string_lossy())?;
    let Ok(files) = fs::read_dir(font_dir) else { return Ok(()) };
    let mut w = Writer::open(index_path)?;
    let folder = index.get(&ffcrc_lower("common/font")).ok_or("index 找不到 common/font")?;
    for file in files.flatten() {
        let name = file.file_name().to_string_lossy().to_string();
        log(&format!("Replace : {name}"));
        let data = fs::read(file.path())?;
        let block = if name.to_ascii_lowercase().ends_with(".tex") { build_tex_block(&data) } else { build_block(&data) };
        let entry = folder.get(&ffcrc_lower(&name)).ok_or_else(|| format!("index 找不到字型 {name}"))?;
        w.append(entry.pt, &block)?;
    }
    w.finish()
}

fn replace_exdf(cfg: &Config, index_path: &Path) -> R<String> {
    let index_str = index_path.to_string_lossy().to_string();
    let slang = cfg.get_or("SLanguage", "JA").to_ascii_lowercase();
    let skip = cfg.get_or("SkipFiles", "").to_ascii_lowercase();
    let skip: Vec<&str> = skip.split('|').filter(|s| !s.is_empty()).collect();
    let (mut replaced, mut no_csv, mut failed) = (0, 0, 0);
    let mut writes: Vec<(u32, Vec<u8>)> = Vec::new(); // 讀回驗證用，只留第一筆與最後一筆

    let index = read_index(&index_str)?;
    let mut dats = Dats::new(&index_str);
    let file_list = init_file_list(&index, &mut dats)?;
    let mut w = Writer::open(index_path)?;

    for (n, sheet) in file_list.iter().enumerate() {
        if n % 500 == 0 {
            log(&format!("[{n}/{}] {sheet}", file_list.len()));
        }
        let lower = format!("exd/{}", sheet.to_ascii_lowercase()); // SkipFiles 格式是 exd/<小寫表名>
        if skip.iter().any(|k| lower == *k || lower.starts_with(&format!("{k}/"))) {
            log(&format!("{lower} in skipFiles. Skip this part."));
            continue;
        }
        let (dir, name) = sheet.rsplit_once('/').map_or(("exd".to_string(), sheet.as_str()), |(d, n)| (format!("exd/{d}"), n));
        let Some(folder) = index.get(&ffcrc_lower(&dir)) else { continue };
        let Some(exh_entry) = folder.get(&ffcrc_lower(&format!("{name}.exh"))) else { continue };
        let exh = match dats.extract(exh_entry.data_offset).and_then(|d| exd::parse_exh(&d)) {
            Ok(e) => e,
            Err(e) => { log(&format!("EXH failed: {sheet}: {e}")); failed += 1; continue; }
        };
        if exh.lang_count == 0 {
            continue;
        }
        let csv_path = p("resource/rawexd").join(format!("{sheet}.csv"));
        if !csv_path.is_file() {
            no_csv += 1;
            continue;
        }
        let (offset_map, csv_rows) = match load_csv(&csv_path) {
            Ok(x) => x,
            Err(e) => { log(&format!("CSV Exception: {sheet}: {e}")); failed += 1; continue; }
        };

        let mut wrote = false;
        for page in &exh.pages {
            let Some(exd_entry) = folder.get(&ffcrc_lower(&format!("{name}_{page}_{slang}.exd"))) else { continue };
            let Ok(mut rows) = dats.extract(exd_entry.data_offset).and_then(|d| exd::parse_exd(&d)) else { continue };
            for (row_id, raw) in rows.iter_mut() {
                if raw.is_empty() || raw.len() < exh.chunk_size {
                    log("Data size was insufficient, bypass handling.");
                    continue;
                }
                let mut chunk = raw[..exh.chunk_size].to_vec();
                let mut strings = Vec::new();
                for &(ty, offset) in &exh.datasets {
                    if ty != 0 {
                        continue; // 只處理字串欄位
                    }
                    let off = offset as usize;
                    chunk[off..off + 4].copy_from_slice(&(strings.len() as u32).to_be_bytes());
                    let translated = csv_rows
                        .get(row_id)
                        .zip(offset_map.get(&offset))
                        .and_then(|(cells, &col)| cells.get(col))
                        .filter(|s| !s.is_empty());
                    match translated {
                        Some(s) => append_csv_string(&mut strings, s)?,
                        None => strings.extend_from_slice(exd::get_string(raw, exh.chunk_size, off)),
                    }
                    strings.push(0);
                }
                // 補到 4 bytes 對齊（剛好對齊時再補 4）
                let len = chunk.len() + strings.len();
                chunk.extend_from_slice(&strings);
                chunk.resize(len + 4 - len % 4, 0);
                *raw = chunk;
            }
            let exd_file = exd::build_exd(&rows);
            let raw_offset = w.append(exd_entry.pt, &build_block(&exd_file))?;
            if writes.len() == 2 {
                writes.pop();
            }
            writes.push((raw_offset, exd_file));
            wrote = true;
        }
        replaced += wrote as u32;
    }
    w.finish()?;
    let summary = format!("漢化完畢：已替換 {replaced} 個資料表（無翻譯 CSV：{no_csv}，失敗：{failed}）");
    log(&summary);
    if replaced == 0 {
        return Err(format!("沒有替換任何文本（失敗：{failed}，無 CSV：{no_csv}）").into());
    }
    // 讀回驗證：重新 extract 第一筆與最後一筆寫入，確保資料真的落地
    for (offset, expected) in &writes {
        match dats.extract(*offset) {
            Ok(back) if back == *expected => {}
            Ok(_) => return Err("漢化寫入驗證失敗（讀回資料與寫入不符），請執行還原".into()),
            Err(e) => return Err(format!("漢化寫入驗證失敗（讀回時發生錯誤：{e}），請執行還原").into()),
        }
    }
    log("Read-back verification passed.");
    Ok(summary)
}

/// root.exl 列出所有資料表，回傳表名（不含 exd/ 與副檔名，例如 "quest/000/ClsArc011_00021"）。
fn init_file_list(index: &crate::sqpack::Index, dats: &mut Dats) -> R<Vec<String>> {
    let root = index
        .get(&ffcrc_lower("exd"))
        .and_then(|f| f.get(&ffcrc_lower("root.exl")))
        .ok_or("index 找不到 exd/root.exl")?;
    let text = String::from_utf8(dats.extract(root.data_offset)?)?;
    Ok(text.lines().map(|l| l.split(',').next().unwrap_or(l).to_string()).filter(|s| !s.is_empty()).collect())
}

/// CSV 內容轉 EXD 位元組：<hex:...> 標籤轉回二進位，其他照 UTF-8。
pub fn append_csv_string(out: &mut Vec<u8>, s: &str) -> R<()> {
    let mut rest = s;
    while let Some(start) = rest.find("<hex") {
        out.extend_from_slice(rest[..start].as_bytes());
        let Some(len) = rest[start..].find('>') else {
            rest = &rest[start..]; // 沒有收尾的 '>'：當一般文字
            break;
        };
        let hex = rest.get(start + 5..start + len).unwrap_or("");
        if hex.contains("<hex") {
            return Err(format!("TagInTagException!{s}").into());
        }
        let hex: String = hex.chars().filter(|&c| c != ' ').collect();
        if hex.len() % 2 != 0 {
            return Err(format!("hex 標籤長度不是偶數：{s}").into());
        }
        for i in (0..hex.len()).step_by(2) {
            out.push(u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| format!("hex 標籤格式錯誤：{s}"))?);
        }
        rest = &rest[start + len + 1..];
    }
    out.extend_from_slice(rest.as_bytes());
    Ok(())
}

/// 讀 SaintCoinach rawexd CSV。'#' 開頭是註解列；有效列 index 1 是 offset 列，資料從 index 3 開始。
/// 回傳 (欄位 offset → 欄號, 列號 → 該列各欄文字)。
fn load_csv(path: &Path) -> R<(HashMap<u16, usize>, HashMap<i32, Vec<String>>)> {
    let text = fs::read_to_string(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .comment(Some(b'#'))
        .from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    let rows: Vec<csv::StringRecord> = reader.records().collect::<Result<_, _>>()?;
    if rows.len() < 3 {
        return Err("CSV 列數不足".into());
    }
    let mut offsets = HashMap::new();
    for (col, v) in rows[1].iter().skip(1).enumerate() {
        offsets.insert(v.parse()?, col);
    }
    let mut data = HashMap::new();
    for row in &rows[3..] {
        data.insert(row[0].parse()?, row.iter().skip(1).map(str::to_string).collect());
    }
    Ok((offsets, data))
}
