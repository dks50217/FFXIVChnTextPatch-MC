//! FFXIVChnTextPatch 的 Rust 移植版核心。對應 csharp/FFXIVChnTextPatch。
//! 兩個執行檔共用：命令列 ffxiv_chn_text_patch（src/main.rs）與操作畫面 FFXIVChnTextPatch（src/bin/gui.rs）。
pub mod config;
pub mod crc;
pub mod drift;
pub mod exd;
pub mod exdnames;
pub mod lint;
pub mod merge;
pub mod patch;
pub mod selftest;
pub mod sheetsig;
pub mod sqpack;
pub mod update;
pub mod zhconvert;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub type R<T> = Result<T, Box<dyn std::error::Error>>;

/// 基準目錄（conf/resource/backup 所在處）：從執行檔位置向上找 conf/global.properties，找不到用目前目錄。
pub fn base_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let cwd = std::env::current_dir().unwrap_or_default();
        let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        exe.into_iter()
            .chain([cwd.clone()])
            .find_map(|start| start.ancestors().find(|d| d.join("conf/global.properties").is_file()).map(Path::to_path_buf))
            .unwrap_or(cwd)
    })
}

pub fn p(rel: &str) -> PathBuf {
    base_dir().join(rel)
}

// ponytail: 只印到 stderr，不寫 debug.log；操作畫面看結果訊息與報告檔即可
pub fn log(msg: &str) {
    eprintln!("{msg}");
}

/// 目前工作的進度，給操作畫面的進度條讀；命令列不看它。
#[derive(Clone, Default)]
pub struct Progress {
    pub percent: f32,
    pub action: String,
    pub detail: String,
}

static PROGRESS: Mutex<Progress> = Mutex::new(Progress { percent: 0.0, action: String::new(), detail: String::new() });

pub fn progress(percent: f32, action: &str, detail: &str) {
    *PROGRESS.lock().unwrap() = Progress { percent, action: action.to_string(), detail: detail.to_string() };
}

pub fn current_progress() -> Progress {
    PROGRESS.lock().unwrap().clone()
}
