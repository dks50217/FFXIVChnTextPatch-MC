//! `--selftest`：已移植部分的二進位邏輯驗證，結果印到 stdout，回傳失敗數當 exit code。
//! 對應 C# SelfTest.cs 第 1-5 項；SheetSig/ExdNames/Merge/Lint/ZhConvert 移植後再補。
use crate::config::Config;
use crate::crc::ffcrc;
use crate::{drift, exd, merge, patch, sqpack, zhconvert};
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

    // 7. ZhConvert 簡轉繁（單字、詞級消歧義、台灣異體字、GP 詞彙表，各走到不同字典）
    for (input, expected, name) in [
        ("汉化", "漢化", "單字"),
        ("头发", "頭髮", "詞級消歧義"),
        ("麪", "麵", "台灣異體字"),
        ("服务器", "伺服器", "台灣用語"),
        ("菜单", "選單", "台灣用語一對多取第一"),
        ("激活", "啟動", "GP 詞彙表"),
        ("几率", "機率", "GP 詞彙表"),
        ("‘", "『", "GP 引號規則"),
        ("L’Heritier", "L’Heritier", "GP 英文名保護"),
        ("0,\"a\",汉", "0,\"a\",漢", "ASCII/CSV 結構字元不動"),
    ] {
        check(&format!("ZhConvert {name} ({input}→{expected})"), zhconvert::s2tw(input) == expected);
    }

    // 8. RawexdMerge 逐格合併規則（與 C# SelfTest 第 7 項同一組案例）
    let m = |lo: &str, up: &str| merge::merge(lo, up, "\n").unwrap();
    let r = m(
        "key,0,1\n#,Name,Desc\noffset,0,4\nint32,str,str\n0,已翻,\n1,,\n3,本地獨有,x\n",
        "key,0,1\n#,Name,Desc\noffset,0,4\nint32,str,str\n0,上游改進,上游補\n1,新翻,\n2,新列,y\n",
    );
    check("Merge 本地非空保留 + 空格補上游", r.text.contains("0,已翻,上游補") && r.text.contains("1,新翻,"));
    check("Merge 上游新列", r.text.contains("2,新列,y") && r.new_rows == 1);
    check("Merge 本地獨有列附加檔尾", r.text.trim_end().ends_with("3,本地獨有,x"));
    check("Merge 補格計數", r.filled == 2 && !r.headers_changed);
    check("Merge 註解列原樣保留", r.text.contains("#,Name,Desc"));

    let quoted = "key,0\n#,Name\noffset,0\nint32,str\n0,\"a,\"\"b\"\"\n\n換行\"\n";
    let recs = merge::parse(&m(quoted, quoted).text);
    check("Merge 引號欄位 round-trip（含欄內空行）", merge::rows(&recs).last().map(|f| f[1].as_str()) == Some("a,\"b\"\n\n換行"));

    let r = m(
        "key,0,1\n#,A,B\noffset,0,4\nint32,str,str\n0,甲,乙\n",
        "key,0,1,2\n#,A,New,B\noffset,0,2,4\nint32,str,str,str\n0,x,新欄,y\n",
    );
    check("Merge 跨版本 offset 欄位對齊", r.text.contains("0,甲,新欄,乙") && r.headers_changed);

    // 上游中間插列時按 offset-0 的 TEXT-id 配對，整條不位移（CtsWks 類漂移的根因防護）
    let r = m(
        "key,0,1\n#,,\noffset,0,4\nInt32,String,String\n0,ID_A,甲\n1,ID_B,乙\n2,ID_C,丙\n",
        "key,0,1\n#,,\noffset,0,4\nInt32,String,String\n0,ID_A,a\n1,ID_NEW,n\n2,ID_B,b\n3,ID_C,c\n",
    );
    check(
        "Merge 按 TEXT-id 配對（上游中間插列不位移）",
        ["0,ID_A,甲", "1,ID_NEW,n", "2,ID_B,乙", "3,ID_C,丙"].iter().all(|s| r.text.contains(s)) && r.new_rows == 1,
    );
    let r = m("key,0\n#,Name\noffset,0\nInt32,String\n0,\n1,本地\n", "key,0\n#,Name\noffset,0\nInt32,String\n0,\n1,上游\n");
    check("Merge 無 id 欄退回位序", r.text.contains("1,本地"));
    // offset-0 本身是被翻譯的中文欄 → 不可當 id，否則差一字的譯文會被當本地獨有、重複附在檔尾
    let r = m("key,0\n#,Name\noffset,0\nInt32,String\n1,甲\n2,管弦樂琴\n", "key,0\n#,Name\noffset,0\nInt32,String\n1,甲\n2,管絃樂琴\n");
    check("Merge 中文欄不當 id（不重複附加列）", r.text.contains("2,管弦樂琴") && !r.text.contains("管絃"));

    // 9. 漂移偵測（與 C# SelfTest 7b-7d 同一組案例）
    //  key2=乙：上游空、乙在上游別處有，但上下(1,3)兩邊對齊 → 合理補白，不算
    //  key10=壬：上游空、壬在上游 key11 有，且 key11 上下不對齊(壬≠癸) → 真的位移
    check(
        "DetectDrift 上下對齊排除、位移才算",
        drift::detect_drift(
            "key,0\n#,Name\noffset,0\nint32,str\n0,甲\n1,乙\n2,乙\n3,丙\n10,壬\n11,癸\n",
            "key,0\n#,Name\noffset,0\nint32,str\n0,甲\n1,乙\n2,\n3,丙\n10,\n11,壬\n",
        ) == [10],
    );
    let id_up = "key,0,1\n#,,\noffset,0,4\nInt32,String,String\n0,ID_A,a\n1,ID_NEW,n\n2,ID_B,b\n3,ID_C,c\n";
    let id_lo = "key,0,1\n#,,\noffset,0,4\nInt32,String,String\n0,ID_A,甲\n1,ID_B,乙\n2,ID_C,丙\n";
    check("DetectIdDrift 抓 TEXT-id 對不上的 key", drift::detect_id_drift(id_lo, id_up) == [1, 2]);
    check("DetectIdDrift 對齊時不誤報", drift::detect_id_drift(id_up, id_up).is_empty());
    check(
        "DetectIdDrift 無 id 欄回空",
        drift::detect_id_drift("key,0\n#,Name\noffset,0\nInt32,String\n0,\n1,本地\n", "key,0\n#,Name\noffset,0\nInt32,String\n0,\n1,上游\n")
            .is_empty(),
    );
    check(
        "DuplicateKeys 抓撞號的 key",
        drift::duplicate_keys("key,0,1\n#,,\noffset,0,4\nInt32,String,String\n0,ID_A,甲\n1,ID_C,丙\n1,ID_B,乙\n") == [1],
    );
    check("DuplicateKeys 乾淨時回空", drift::duplicate_keys(id_up).is_empty());

    println!("{}", if failed == 0 { "ALL PASSED".to_string() } else { format!("{failed} FAILED") });
    failed
}
