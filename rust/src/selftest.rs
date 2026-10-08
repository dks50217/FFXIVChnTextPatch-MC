//! `--selftest`：已移植部分的二進位邏輯驗證，結果印到 stdout，回傳失敗數當 exit code。
//! 對應 C# SelfTest.cs 第 1-5 項；SheetSig/ExdNames/Merge/Lint/ZhConvert 移植後再補。
use crate::config::Config;
use crate::crc::ffcrc;
use crate::{exd, patch, sqpack};
use std::collections::BTreeMap;

pub fn run() -> i32 {
    let mut failed = 0;
    let mut check = |name: &str, ok: bool| {
        println!("{}  {name}", if ok { "PASS" } else { "FAIL" });
        failed += !ok as i32;
    };

    // 1. FFCRC 與 Java 版輸出一致
    for (input, expected) in [
        ("common/font", 1713442675),
        ("axis_12.fdt", -1447579230),
        ("exd", -476350055),
        ("root.exl", 1370848956),
        ("exd/item_0_ja.exd", -1235084413),
        ("exd/quest/000/clshrv001_00003.exh", 434756717),
    ] {
        check(&format!("FFCRC(\"{input}\") == {expected}"), ffcrc(input.as_bytes()) as i32 == expected);
    }

    // 2. deflate round-trip（一半可壓縮、一半偽亂數）
    let mut seed = 42u32;
    let data: Vec<u8> = (0..40000u32)
        .map(|i| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            if i < 20000 { (i % 7) as u8 } else { (seed >> 24) as u8 }
        })
        .collect();
    check("deflate round-trip", sqpack::decompress(&sqpack::compress(&data), data.len()).ok() == Some(data.clone()));

    // 3. EXDF build → parse round-trip
    let rows: BTreeMap<i32, Vec<u8>> =
        [(5, b"hello\0pad.".to_vec()), (1, vec![0, 0, 0, 4, 1, 2, 3, 4, 0, 0, 0, 0]), (42, vec![9; 4])].into();
    check("EXDF build/parse round-trip", exd::parse_exd(&exd::build_exd(&rows)).ok() == Some(rows));

    // 4. type 2 區塊 build → extract round-trip（多區塊），且漢化中另有寫入 handle 時仍讀得到
    let block = sqpack::build_block(&data);
    check("Block 128-byte alignment", block.len() % 128 == 0);
    let tmp = std::env::temp_dir().join("ffxivpatch-rs-selftest.dat");
    let extracted = std::fs::write(&tmp, &block).ok().and_then(|_| {
        let _writer = std::fs::OpenOptions::new().append(true).open(&tmp).ok()?;
        sqpack::extract_at(&mut std::fs::File::open(&tmp).ok()?, 0).ok()
    });
    check("Block build/extract round-trip（同時開著寫入 handle）", extracted == Some(data));
    let _ = std::fs::remove_file(&tmp);

    // 5. Config 跳脫：遊戲路徑解得開，含 : 與 | 的 PatchedStamp 存得回來
    let tmp = std::env::temp_dir().join("ffxivpatch-rs-selftest.properties");
    let _ = std::fs::write(&tmp, "#c\nGamePath=D\\:\\\\FF14\\\\FINAL FANTASY XIV\n");
    check("Config path unescape", Config::load(&tmp).get("GamePath") == Some(r"D:\FF14\FINAL FANTASY XIV"));
    let stamp = "123456:638900000000000000|789:638900000000000001";
    let mut cfg = Config::load(&tmp);
    cfg.set("PatchedStamp", stamp);
    let saved = cfg.save().is_ok();
    check("Config 存檔 round-trip（含 : 與 |）", saved && Config::load(&tmp).get("PatchedStamp") == Some(stamp));
    let _ = std::fs::remove_file(&tmp);

    // 6. CSV 字串 → EXD 位元組：<hex:> 轉二進位、其他照 UTF-8、巢狀標籤報錯
    let mut out = Vec::new();
    let ok = patch::append_csv_string(&mut out, "a<hex:0210 01 03>中>").is_ok();
    check("CSV <hex:> 轉二進位", ok && out == [b"a".as_slice(), &[2, 0x10, 1, 3], "中>".as_bytes()].concat());
    check("CSV 巢狀 <hex 報錯", patch::append_csv_string(&mut Vec::new(), "<hex:02<hex:03>").is_err());

    println!("{}", if failed == 0 { "ALL PASSED".to_string() } else { format!("{failed} FAILED") });
    failed
}
