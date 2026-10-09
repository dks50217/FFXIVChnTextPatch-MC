//! 一鍵更新 rawexd 翻譯：git sparse clone 上游的 resource/rawexd → 簡轉繁 → 逐格合併（本地翻譯優先）。
//! 合併前把整個 resource/rawexd 備份成 backup/rawexd-before-update.zip（只留最新一份）。對應 C# RawexdUpdater。
use crate::config::Config;
use crate::{drift, log, merge, p, progress, zhconvert, R};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_REPO: &str = "https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma";
const BOM: &str = "\u{feff}";

pub fn update(cfg: &Config) -> R<String> {
    let repo = cfg.get_or("UpstreamRepo", DEFAULT_REPO);
    let tmp = std::env::temp_dir().join("ffxiv-rawexd-upstream");
    let result = update_from(repo, &tmp);
    let _ = delete_dir(&tmp); // 暫存目錄清不掉不影響結果
    result
}

/// CI/CLI 漂移檢查（--driftcheck）：clone 上游 → 簡轉繁 → 逐檔跟本地比對 → 寫 rawexd-drift.txt，不改任何翻譯檔。
/// 回傳有問題的檔數（錯位 + 重複 RowId，0 = 乾淨）；clone/上游失敗回錯誤，呼叫端當 warn-only。
pub fn drift_check(cfg: &Config) -> R<usize> {
    let tmp = std::env::temp_dir().join("ffxiv-rawexd-driftcheck");
    let result = (|| -> R<usize> {
        let up_dir = clone_upstream(cfg.get_or("UpstreamRepo", DEFAULT_REPO), &tmp)?;
        let local_dir = p("resource/rawexd");
        log("正在比對漂移……");
        let mut drifted = Vec::new();
        for up_path in csv_files(&up_dir) {
            let rel = up_path.strip_prefix(&up_dir)?.to_string_lossy().replace('\\', "/");
            let local_path = local_dir.join(&rel);
            if !local_path.is_file() {
                continue; // 上游新檔、本地還沒有，不算漂移
            }
            let keys = read_utf8(&local_path).and_then(|(lo, _)| Ok(suspects(&lo, &zhconvert::s2tw(&read_utf8(&up_path)?.0))));
            match keys {
                Ok(keys) if !keys.is_empty() => drifted.push((rel, keys)),
                Ok(_) => {}
                Err(e) => log(&format!("漂移檢查失敗 {rel}: {e}")),
            }
        }
        let dupes = scan_duplicate_keys(&local_dir);
        write_drift_report(&drifted, &dupes)?;
        let keys: usize = drifted.iter().map(|(_, k)| k.len()).sum();
        log(&if drifted.is_empty() && dupes.is_empty() {
            "漂移檢查：乾淨，無疑似錯位".to_string()
        } else {
            format!("漂移檢查：{} 檔疑似錯位（{keys} 個 key）、{} 檔重複 RowId，見 rawexd-drift.txt", drifted.len(), dupes.len())
        });
        Ok(drifted.len() + dupes.len())
    })();
    let _ = delete_dir(&tmp);
    result
}

/// sparse clone 只抓 resource/rawexd（約 100MB；整包 zip 近 900MB 不可行），回傳上游 rawexd 目錄。
fn clone_upstream(repo: &str, tmp: &Path) -> R<PathBuf> {
    delete_dir(tmp)?;
    progress(0.02, "正在下載上游翻譯……", "");
    log("正在下載上游翻譯……");
    git(&["clone", "--depth", "1", "--filter=blob:none", "--sparse", "--progress", repo, &tmp.to_string_lossy()])?;
    git(&["-C", &tmp.to_string_lossy(), "sparse-checkout", "set", "resource/rawexd"])?;
    let up_dir = tmp.join("resource/rawexd");
    if !up_dir.is_dir() {
        return Err("上游 repo 裡找不到 resource/rawexd".into());
    }
    Ok(up_dir)
}

/// 疑似錯位的 key：內容啟發式 ∪ TEXT-id 精確比對。upstream 須已簡轉繁。
fn suspects(local: &str, upstream: &str) -> Vec<i32> {
    let mut keys = drift::detect_drift(local, upstream);
    keys.extend(drift::detect_id_drift(local, upstream));
    keys.sort();
    keys.dedup();
    keys
}

fn update_from(repo: &str, tmp: &Path) -> R<String> {
    let local_dir = p("resource/rawexd");
    // 1. 下載
    let up_dir = clone_upstream(repo, tmp)?;

    // 2. 備份本地翻譯
    progress(0.45, "正在備份本地翻譯……", "");
    log("正在備份本地翻譯……");
    backup_zip(&local_dir, &p("backup/rawexd-before-update.zip"))?;

    // 3. 逐檔簡轉繁 + 合併
    log("正在合併翻譯……");
    let (mut filled, mut new_rows, mut changed, mut new_files, mut failed) = (0, 0, 0, 0, 0);
    let mut drifted: Vec<(String, Vec<i32>)> = Vec::new();
    let files = csv_files(&up_dir);
    for (i, up_path) in files.iter().enumerate() {
        let rel = up_path.strip_prefix(&up_dir)?.to_string_lossy().replace('\\', "/");
        progress(0.5 + 0.5 * i as f32 / files.len() as f32, "正在合併翻譯：", &rel);
        let local_path = local_dir.join(&rel);
        let r = (|| -> R<()> {
            let (up_text, up_bom) = read_utf8(&up_path)?;
            let up_text = zhconvert::s2tw(&up_text);
            if !local_path.is_file() {
                fs::create_dir_all(local_path.parent().unwrap())?;
                write_csv(&local_path, &up_text, up_bom)?;
                new_files += 1;
                return Ok(());
            }
            let (local_text, local_bom) = read_utf8(&local_path)?;
            // 上游剛下載、轉好就在手上，順手做漂移偵測
            let keys = suspects(&local_text, &up_text);
            if !keys.is_empty() {
                drifted.push((rel.clone(), keys));
            }
            let nl = if local_text.contains("\r\n") { "\r\n" } else { "\n" };
            let m = merge::merge(&local_text, &up_text, nl)?;
            if m.filled > 0 || m.new_rows > 0 || m.headers_changed {
                write_csv(&local_path, &m.text, local_bom)?;
                changed += 1;
                filled += m.filled;
                new_rows += m.new_rows;
            }
            Ok(())
        })();
        if let Err(e) = r {
            failed += 1;
            log(&format!("合併失敗 {rel}: {e}"));
        }
    }

    let dupes = scan_duplicate_keys(&local_dir);
    write_drift_report(&drifted, &dupes)?;
    let drift_keys: usize = drifted.iter().map(|(_, k)| k.len()).sum();
    let mut msg = format!("更新完成：{changed} 檔更新（補 {filled} 格、新增 {new_rows} 列）、{new_files} 個新檔");
    if failed > 0 {
        msg += &format!("、{failed} 檔失敗（見上方訊息）");
    }
    if !drifted.is_empty() {
        msg += &format!("、{} 檔疑似錯位（{drift_keys} 個 key，見 rawexd-drift.txt）", drifted.len());
    }
    if !dupes.is_empty() {
        msg += &format!("、{} 檔有重複 RowId（見 rawexd-drift.txt）", dupes.len());
    }
    msg += "。原檔已備份至 backup/rawexd-before-update.zip";
    if failed > 0 { Err(msg.into()) } else { Ok(msg) }
}

/// 跑 git，進度直接印在主控台（stderr 繼承）。
fn git(args: &[&str]) -> R<()> {
    let status = crate::command("git")
        .args(args)
        .status()
        .map_err(|e| format!("無法執行 git，請先安裝 Git for Windows。{e}"))?;
    if !status.success() {
        return Err(format!("git {} 失敗（{status}）", args[0]).into());
    }
    Ok(())
}

/// 用 Windows 內建的 tar.exe（bsdtar，-a 依副檔名產 zip）打包；不另加 zip 套件。
fn backup_zip(dir: &Path, zip: &Path) -> R<()> {
    fs::create_dir_all(zip.parent().unwrap())?;
    let _ = fs::remove_file(zip);
    let status = crate::command(crate::system_tool("tar")).arg("-a").arg("-cf").arg(zip).arg("-C").arg(dir).arg(".").status()?;
    if !status.success() {
        return Err(format!("備份 resource/rawexd 失敗（tar {status}）").into());
    }
    Ok(())
}

/// 讀 UTF-8 CSV，回傳 (去掉 BOM 的內容, 原本有沒有 BOM)。
// ponytail: C# 版遇到非 UTF-8 會當舊 ConvertZZ 的 Big5 檔讀進來再改寫成 UTF-8；那批檔在 C# 跑過
// --update 後都已修好，這裡改成直接報錯（該檔算失敗），真的再遇到才加 encoding_rs。
fn read_utf8(path: &Path) -> R<(String, bool)> {
    let text = String::from_utf8(fs::read(path)?).map_err(|_| "不是 UTF-8（可能是舊的 Big5 檔，請先用 C# 版 --update 修復）")?;
    Ok(match text.strip_prefix(BOM) {
        Some(rest) => (rest.to_string(), true),
        None => (text, false),
    })
}

fn write_csv(path: &Path, text: &str, bom: bool) -> std::io::Result<()> {
    fs::write(path, if bom { format!("{BOM}{text}") } else { text.to_string() })
}

/// 遞迴列出 CSV，順序跟 .NET Directory.GetFiles(AllDirectories) 一樣（同層照 NTFS 順序、子目錄排隊後處理），
/// 報告裡同分的項目才會跟 C# 版排得一樣。
pub fn csv_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut queue = std::collections::VecDeque::from([dir.to_path_buf()]);
    while let Some(d) = queue.pop_front() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let path = e.path();
            if path.is_dir() {
                queue.push_back(path);
            } else if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("csv")) {
                out.push(path);
            }
        }
    }
    out
}

/// 掃本地 rawexd 找重複 RowId（不需上游）。
fn scan_duplicate_keys(dir: &Path) -> Vec<(String, Vec<i32>)> {
    csv_files(dir)
        .into_iter()
        .filter_map(|path| {
            let rel = path.strip_prefix(dir).ok()?.to_string_lossy().replace('\\', "/");
            match read_utf8(&path) {
                Ok((text, _)) => Some((rel, drift::duplicate_keys(&text))).filter(|(_, d)| !d.is_empty()),
                Err(e) => {
                    log(&format!("重複 RowId 檢查失敗 {rel}: {e}"));
                    None
                }
            }
        })
        .collect()
}

/// 寫 rawexd-drift.txt；乾淨時刪掉舊報告，免得殘留誤導。
fn write_drift_report(drifted: &[(String, Vec<i32>)], dupes: &[(String, Vec<i32>)]) -> std::io::Result<()> {
    let path = p("rawexd-drift.txt");
    if drifted.is_empty() && dupes.is_empty() {
        let _ = fs::remove_file(path);
        return Ok(());
    }
    let section = |out: &mut String, items: &[(String, Vec<i32>)]| {
        let mut items: Vec<_> = items.iter().collect();
        items.sort_by_key(|(_, keys)| std::cmp::Reverse(keys.len()));
        for (rel, keys) in items {
            let keys: Vec<String> = keys.iter().map(i32::to_string).collect();
            *out += &format!("{rel}（{}）：{}\n", keys.len(), keys.join(", "));
        }
    };
    let mut out = format!("rawexd 檢查報告  {}\n", crate::now());
    if !drifted.is_empty() {
        out += "\n【疑似錯位 key】上游該列全空、本地卻有翻譯。\n遊戲改版重新編號後，舊翻譯被釘在錯 key 的徵狀；請對照上游確認後再修。\n";
        section(&mut out, drifted);
    }
    if !dupes.is_empty() {
        out += "\n【重複 RowId】同一個 key 出現多次，套用時後面那列會蓋掉前面那列。\n多半是 merge 檔尾附加造成的（上游刪/改名了該列的 TEXT-id）；留一列、把譯文併回去。\n";
        section(&mut out, dupes);
    }
    fs::write(path, out)
}

/// git 物件檔是唯讀的，先清屬性再刪。
fn delete_dir(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)?.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let mut perm = e.metadata()?.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perm.set_readonly(false);
                fs::set_permissions(&path, perm)?;
            }
        }
    }
    fs::remove_dir_all(dir)
}
