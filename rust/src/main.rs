//! FFXIVChnTextPatch 的 Rust 移植版（CLI，尚無介面）。對應 csharp/FFXIVChnTextPatch。
mod config;
mod crc;
mod drift;
mod exd;
mod exdnames;
mod lint;
mod merge;
mod patch;
mod selftest;
mod sheetsig;
mod sqpack;
mod update;
mod zhconvert;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

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

// ponytail: CLI 直接印到 stderr，不寫 debug.log；之後有介面再接 log 檔。
pub fn log(msg: &str) {
    eprintln!("{msg}");
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let mut cfg = config::Config::load(&p("conf/global.properties"));
    let start = Instant::now();
    let code = match arg.as_str() {
        "--selftest" => selftest::run(),
        "--patch" => report(patch::patch(&mut cfg)),
        "--rollback" => report(patch::rollback(&mut cfg)),
        "--update" => report(update::update(&cfg)),
        "--lint" => lint::run(&cfg) as i32, // exit code = 會中斷漢化的錯誤數
        // exit code = 不符的格數；給 base ref 只查有變動的格子（CI/PR），不給就掃全部（本機 triage）
        "--sheetsig" => sheetsig::check(&cfg, std::env::args().nth(2).filter(|a| !a.starts_with("--")).as_deref()) as i32,
        // exit code = 有問題的檔數；clone/上游失敗回 0，CI 當 warn-only，別讓網路問題誤報成漂移
        "--driftcheck" => update::drift_check(&cfg).map_or_else(|e| { eprintln!("漂移檢查失敗：{e}"); 0 }, |n| n as i32),
        "--s2tw" => report(std::env::args().nth(2).ok_or("用法：--s2tw <檔案>".into()).and_then(|f| Ok(zhconvert::s2tw(&std::fs::read_to_string(f)?)))),
        _ => {
            eprintln!("用法：ffxiv_chn_text_patch --patch | --rollback | --update | --driftcheck | --lint | --sheetsig [base-ref] | --selftest | --s2tw <檔案>");
            2
        }
    };
    eprintln!("耗時 {:.2?}", start.elapsed());
    // exit code 只有 8 位元（Linux/Git Bash 取 mod 256），計數超過 255 就封頂，免得剛好 256 個錯誤被讀成 0 = 通過
    std::process::exit(code.clamp(0, 255));
}

fn report(r: R<String>) -> i32 {
    match r {
        Ok(msg) => { println!("{msg}"); 0 }
        Err(e) => { eprintln!("失敗：{e}"); 1 }
    }
}
