//! 命令列版。操作畫面在 src/bin/gui.rs。
use ffxiv_chn_text_patch::{config, hextags, lint, p, patch, selftest, sheetsig, update, zhconvert, R};
use std::time::Instant;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let mut cfg = config::Config::load(&p("conf/global.properties"));
    let start = Instant::now();
    let code = match arg.as_str() {
        "--selftest" => selftest::run(),
        "--patch" => report(patch::patch(&mut cfg)),
        "--rollback" => report(patch::rollback(&mut cfg)),
        "--update" => report(update::update(&cfg)),
        "--lint" => {
            // exit code = 會中斷漢化的錯誤數
            let (errors, summary) = lint::run(&cfg);
            println!("{summary}");
            errors as i32
        }
        // exit code = 不符的格數；給 base ref 只查有變動的格子（CI/PR），不給就掃全部（本機 triage）
        "--sheetsig" => sheetsig::check(&cfg, std::env::args().nth(2).filter(|a| !a.starts_with("--")).as_deref()) as i32,
        // exit code = 有問題的檔數；clone/上游失敗回 0，CI 當 warn-only，別讓網路問題誤報成漂移
        "--driftcheck" => update::drift_check(&cfg).map_or_else(|e| { eprintln!("漂移檢查失敗：{e}"); 0 }, |n| n as i32),
        // 本機用：從 SaintCoinach 日文匯出（<版本>/rawexd）產生 Sheet 參照檔；失敗回 2
        "--gensheetsig" => match std::env::args().nth(2).filter(|a| !a.starts_with("--")) {
            Some(dir) => if report(sheetsig::generate(std::path::Path::new(&dir))) == 0 { 0 } else { 2 },
            None => {
                eprintln!("用法：--gensheetsig <SaintCoinach 日文匯出的 rawexd 目錄>");
                2
            }
        },
        "--hextags" => {
            println!("{}", hextags::run());
            0
        }
        "--s2tw" => report(std::env::args().nth(2).ok_or("用法：--s2tw <檔案>".into()).and_then(|f| Ok(zhconvert::s2tw(&std::fs::read_to_string(f)?)))),
        _ => {
            eprintln!("用法：ffxiv_chn_text_patch --patch | --rollback | --update | --driftcheck | --lint | --sheetsig [base-ref] | --hextags | --gensheetsig <dir> | --selftest | --s2tw <檔案>");
            eprintln!("操作畫面請開 FFXIVChnTextPatch.exe。");
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
