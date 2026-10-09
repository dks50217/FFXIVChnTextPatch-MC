//! EXH/EXD 遊戲資料表。注意：內容是 big-endian（與 SqPack index/dat 相反）。
use crate::R;
use std::collections::BTreeMap;

fn be32(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(p..p + 4)?.try_into().ok()?))
}

fn be16(b: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(p..p + 2)?.try_into().ok()?))
}

pub struct Exh {
    pub chunk_size: usize,
    /// (欄位型別, chunk 內 offset)；型別 0 = 字串
    pub datasets: Vec<(u16, u16)>,
    pub pages: Vec<u32>,
    pub lang_count: usize,
}

pub fn parse_exh(d: &[u8]) -> R<Exh> {
    if be32(d, 0) != Some(0x4558_4846) || be16(d, 4) != Some(3) {
        return Err("Not a EXHF".into());
    }
    let n = |p| be16(d, p).unwrap_or(0) as usize;
    let (datasets, pages) = (n(8), n(10));
    let pages_at = 32 + datasets * 4;
    // 資料不足時跟 Java/C# 版一樣靜默容忍，只取讀得到的部分
    Ok(Exh {
        chunk_size: n(6),
        datasets: (0..datasets).map_while(|i| Some((be16(d, 32 + i * 4)?, be16(d, 34 + i * 4)?))).collect(),
        pages: (0..pages).map_while(|i| be32(d, pages_at + i * 8)).collect(),
        lang_count: n(12),
    })
}

/// 解 EXD：列號 → 該列原始資料。
pub fn parse_exd(d: &[u8]) -> R<BTreeMap<i32, Vec<u8>>> {
    if be32(d, 0) != Some(0x4558_4446) || be16(d, 4) != Some(2) {
        return Err("Not a EXDF".into());
    }
    let bad = || "EXDF truncated";
    let table = be32(d, 8).ok_or_else(bad)? as usize;
    let mut rows = BTreeMap::new();
    for i in 0..table / 8 {
        let p = 32 + i * 8;
        let (index, offset) = (be32(d, p).ok_or_else(bad)?, be32(d, p + 4).ok_or_else(bad)? as usize);
        let size = be32(d, offset).ok_or_else(bad)? as usize;
        rows.insert(index as i32, d.get(offset + 6..offset + 6 + size).ok_or_else(bad)?.to_vec()); // +4 size +2 flags
    }
    Ok(rows)
}

pub fn build_exd(rows: &BTreeMap<i32, Vec<u8>>) -> Vec<u8> {
    let mut table = Vec::with_capacity(rows.len() * 8);
    let mut body = Vec::new();
    let mut offset = 32 + rows.len() * 8;
    for (&index, data) in rows {
        table.extend_from_slice(&index.to_be_bytes());
        table.extend_from_slice(&(offset as u32).to_be_bytes());
        body.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(data);
        offset += data.len() + 6;
    }
    let mut out = b"EXDF\0\x02\0\0".to_vec();
    out.extend_from_slice(&(table.len() as u32).to_be_bytes());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.resize(32, 0);
    out.extend_from_slice(&table);
    out.extend_from_slice(&body);
    out
}

/// 讀出某列 chunk 內 offset 欄位指到的 null 結尾字串（不含結尾 0）。
pub fn get_string(row: &[u8], chunk_size: usize, offset: usize) -> &[u8] {
    let Some(rel) = be32(row, offset) else { return &[] };
    match row.get(chunk_size + rel as usize..) {
        Some(rest) => &rest[..rest.iter().position(|&b| b == 0).unwrap_or(rest.len())],
        None => &[],
    }
}
