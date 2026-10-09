//! 唯讀對照工具（--hextags）：掃描 resource/rawexd 下所有 CSV 的 <hex:...> 標籤，依 SaintCoinach 的
//! SeString TagType 解成可讀的標籤名，方便翻譯者辨認哪些 blob 不能動。報告寫到 hextags-report.txt，不改任何檔案。
//! 對應 C# HexTagTool。
use crate::update::csv_files;
use crate::{log, p, progress};
use std::collections::HashMap;

/// SeString 標籤起始位元組 0x02 後的 TagType，取自 SaintCoinach/Text/TagType.cs。
fn tag_name(t: u8) -> Option<&'static str> {
    Some(match t {
        0x06 => "ResetTime",
        0x07 => "Time",
        0x08 => "If",
        0x09 => "Switch",
        0x0A => "Unknown0A",
        0x0C => "IfEquals",
        0x10 => "LineBreak",
        0x12 => "Gui",
        0x13 => "Color",
        0x14 => "Unknown14",
        0x16 => "SoftHyphen",
        0x17 => "Unknown17",
        0x19 => "Emphasis2",
        0x1A => "Emphasis",
        0x1D => "Indent",
        0x1E => "CommandIcon",
        0x1F => "Dash",
        0x20 => "Value",
        0x22 => "Format",
        0x24 => "TwoDigitValue",
        0x28 => "Sheet",
        0x29 => "Highlight",
        0x2B => "Clickable",
        0x2C => "Split",
        0x2D => "Unknown2D",
        0x2E => "Fixed",
        0x2F => "Unknown2F",
        0x30 => "SheetJa",
        0x31 => "SheetEn",
        0x32 => "SheetDe",
        0x33 => "SheetFr",
        0x40 => "InstanceContent",
        0x48 => "UIForeground",
        0x49 => "UIGlow",
        0x4A => "RubyCharaters",
        0x50 => "ZeroPaddedValue",
        0x60 => "Unknown60",
        _ => return None,
    })
}

/// 把一段 hex（如 "02100103"）解成可讀標籤名，如 "[LineBreak 01]"。只認 TagType，參數原樣以 hex 呈現。
// ponytail: 不解析 SeString 的整數/字串長度編碼——翻譯者只需要標籤名，參數維持不透明。
pub fn decode(hex: &str) -> String {
    let bytes: Option<Vec<u8>> = (hex.len() % 2 == 0)
        .then(|| (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok()).collect())
        .flatten();
    let Some(b) = bytes else { return "(格式錯誤)".into() };
    let upper = |b: &[u8]| b.iter().map(|x| format!("{x:02X}")).collect::<String>();
    if b == [0x03] {
        return "[end]".into();
    }
    if b.len() >= 2 && b[0] == 0x02 {
        if let Some(name) = tag_name(b[1]) {
            let end = if b[b.len() - 1] == 0x03 { b.len() - 1 } else { b.len() }; // 去掉結尾終止碼
            let params = upper(&b[2..end.max(2)]);
            return if params.is_empty() { format!("[{name}]") } else { format!("[{name} {params}]") };
        }
    }
    upper(&b) // 落單的參數片段
}

pub fn run() -> String {
    let rawexd = p("resource/rawexd");
    let files = csv_files(&rawexd);
    // 標籤 → 出現次數，照第一次出現的順序（同次數時的排序跟 C# 版一致）
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (i, path) in files.iter().enumerate() {
        progress((i + 1) as f32 / files.len() as f32, "正在掃描：", &path.file_name().unwrap_or_default().to_string_lossy());
        let text = String::from_utf8_lossy(&std::fs::read(path).unwrap_or_default()).into_owned();
        let mut rest = text.as_str();
        // 等同 regex <hex:([0-9A-Fa-f]+)>
        while let Some(at) = rest.find("<hex:") {
            let body = &rest[at + 5..];
            let len = body.bytes().take_while(u8::is_ascii_hexdigit).count();
            if len == 0 || body.as_bytes().get(len) != Some(&b'>') {
                rest = &rest[at + 1..];
                continue;
            }
            let tag = body[..len].to_ascii_uppercase();
            *counts.entry(tag.clone()).or_insert_with(|| {
                order.push(tag);
                0
            }) += 1;
            rest = &body[len + 1..];
        }
    }
    order.sort_by_key(|t| std::cmp::Reverse(counts[t])); // 穩定排序
    let mut out = format!("hex 標籤對照表  {}\n相異標籤數：{}\n（依出現次數排序；解碼依 SaintCoinach SeString TagType）\n\n", crate::now(), order.len());
    out += &format!("{:>10}  {:<24}  解讀\n", "出現次數", "hex 標籤");
    for tag in &order {
        out += &format!("{:>10}  {:<24}  {}\n", counts[tag], format!("<hex:{tag}>"), decode(tag));
    }
    let path = p("hextags-report.txt");
    if let Err(e) = std::fs::write(&path, out) {
        return format!("寫入 hextags-report.txt 失敗：{e}");
    }
    log(&format!("[HexTags] {} 種標籤，報告寫到 {}", order.len(), path.display()));
    format!("完成：{} 種標籤 → hextags-report.txt", order.len())
}
