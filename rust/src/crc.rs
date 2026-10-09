//! FFXIV 檔名雜湊。C# 版那四張表是標準 CRC-32（0xEDB88320）的 slicing-by-4，
//! 差別只在結尾不做 XOR，所以 = !crc32。selftest 的向量驗證與 Java/C# 逐位元一致。

const TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

pub fn ffcrc(bytes: &[u8]) -> u32 {
    bytes.iter().fold(u32::MAX, |c, &b| TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8))
}

/// 雜湊小寫後的路徑/檔名（索引裡存的都是小寫）。
pub fn ffcrc_lower(s: &str) -> u32 {
    ffcrc(s.to_ascii_lowercase().as_bytes())
}
