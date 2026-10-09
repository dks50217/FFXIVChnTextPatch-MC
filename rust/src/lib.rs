//! FFXIVChnTextPatch 的 Rust 移植版核心。對應 csharp/FFXIVChnTextPatch。
//! 兩個執行檔共用：命令列 ffxiv_chn_text_patch（src/main.rs）與操作畫面 FFXIVChnTextPatch（src/bin/gui.rs）。
pub mod bootstrap;
pub mod config;
pub mod crc;
pub mod drift;
pub mod exd;
pub mod exdnames;
pub mod hextags;
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

/// 印到 stderr，並附上本地時間寫進基準目錄的 debug.log（操作畫面沒有主控台，出問題時靠它查）。
pub fn log(msg: &str) {
    eprintln!("{msg}");
    use std::io::Write;
    let line = format!("[{}] {msg}\n", now());
    // 記錄失敗不影響主流程
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(p("debug.log")).and_then(|mut f| f.write_all(line.as_bytes()));
}

/// 本地時間 "yyyy-MM-dd HH:mm:ss"。std 沒有時區，Windows 直接問系統；其他平台（只有 CI）用 UTC。
pub fn now() -> String {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            millis: u16,
        }
        extern "system" {
            fn GetLocalTime(t: *mut SystemTime);
        }
        let mut t = SystemTime::default();
        unsafe { GetLocalTime(&mut t) };
        format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second)
    }
    #[cfg(not(windows))]
    {
        utc_string(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
    }
}

/// Unix 秒數 → UTC "yyyy-MM-dd HH:mm:ss"（公曆換算用 Howard Hinnant 的 civil_from_days）。
pub fn utc_string(secs: u64) -> String {
    let (days, rem) = (secs / 86400, secs % 86400);
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// 啟動外部程式（git、tar、curl、tasklist）一律走這裡。
/// 操作畫面沒有主控台，直接啟動主控台程式會每次閃出一個黑色視窗，所以那時加 CREATE_NO_WINDOW；
/// 命令列有主控台就不加，git 的下載進度才印得出來。
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        extern "system" {
            fn GetConsoleWindow() -> *mut std::ffi::c_void;
        }
        if unsafe { GetConsoleWindow() }.is_null() {
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
    }
    cmd
}

/// Windows 內建工具（System32）的完整路徑，避免 PATH 裡 Git 附的同名 GNU 版本先被找到（GNU tar 不支援 zip）。
pub fn system_tool(name: &str) -> PathBuf {
    if cfg!(windows) {
        Path::new(&std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32").join(format!("{name}.exe"))
    } else {
        PathBuf::from(name)
    }
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
