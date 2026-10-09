//! 首次執行缺翻譯檔時，從 GitHub Release 下載 rawexd-opencc.zip 解壓到 resource/（不需 git）。對應 C# ResourceBootstrap。
//! 契約：zip 根目錄直接是 rawexd/ 與 opencc/ 兩個資料夾。放在固定 tag rawexd-latest 的 Release，
//! 由 .github/workflows/rawexd-asset.yml 在 master 的翻譯檔有變動時自動重新打包覆蓋，跟遊戲版本的發行無關。
//! 要換位址就在 conf/global.properties 設 ResourceZipUrl。
//! 下載與解壓用 Windows 內建的 curl.exe、tar.exe，不另加 HTTP/zip 套件。
use crate::config::Config;
use crate::{command, log, p, patch, progress, system_tool, R};
use std::fs;
use std::io::Read;
use std::process::Stdio;
use std::time::Duration;

const DEFAULT_URL: &str = "https://github.com/dks50217/FFXIVChnTextPatch-MC/releases/download/rawexd-latest/rawexd-opencc.zip";
pub const REPO_URL: &str = "https://github.com/dks50217/FFXIVChnTextPatch-MC";

/// resource/rawexd 一個 CSV 都沒有就視為缺翻譯檔。
pub fn needs_csv() -> bool {
    !patch::has_csv_files(&p("resource/rawexd"))
}

/// resource/font 沒有任何檔就視為缺字體。
pub fn has_font() -> bool {
    fs::read_dir(p("resource/font")).is_ok_and(|mut it| it.next().is_some())
}

pub fn download(cfg: &Config) -> R<String> {
    let url = cfg.get_or("ResourceZipUrl", DEFAULT_URL);
    let zip = std::env::temp_dir().join("ffxiv-resource.zip");
    let result = (|| -> R<String> {
        progress(0.0, "正在下載翻譯檔……", "");
        let total = content_length(url);
        let _ = fs::remove_file(&zip);
        let mut child = command(system_tool("curl"))
            .args(["-fsSL", "--retry", "2", "-o"])
            .arg(&zip)
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("無法執行 curl：{e}"))?;
        // 邊下載邊看暫存檔大小更新進度；問不到總大小就只顯示已下載幾 MB
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            let done = fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
            match total {
                Some(t) => progress(done as f32 / t as f32 * 0.9, "正在下載翻譯檔……", &format!("{}MB / {}MB", done >> 20, t >> 20)),
                None => progress(0.0, "正在下載翻譯檔……", &format!("{}MB", done >> 20)),
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        if !status.success() {
            let mut err = String::new();
            child.stderr.take().map(|mut s| s.read_to_string(&mut err));
            return Err(format!("無法從 {url} 下載：{}", err.trim()).into());
        }

        progress(0.92, "正在解壓縮……", "");
        fs::create_dir_all(p("resource"))?;
        let status = command(system_tool("tar")).arg("-xf").arg(&zip).arg("-C").arg(p("resource")).status()?;
        if !status.success() {
            return Err(format!("解壓縮失敗（tar {status}）").into());
        }
        if needs_csv() {
            return Err("下載完成但仍找不到 CSV，請確認 zip 根目錄直接是 rawexd/ 資料夾".into());
        }
        log("翻譯檔下載並解壓完成。");
        Ok("翻譯檔下載完成，可以開始漢化了".into())
    })();
    let _ = fs::remove_file(&zip); // 暫存檔清不掉不影響結果
    result
}

/// 先問一次檔案大小（跟著轉址到最後一站），好算下載百分比。
fn content_length(url: &str) -> Option<u64> {
    let out = command(system_tool("curl")).args(["-sIL", url]).output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse().ok()))
        .last()
        .filter(|&n| n > 0)
}
