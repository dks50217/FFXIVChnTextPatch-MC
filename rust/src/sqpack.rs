//! SqPack：.index 解析（CRC → offset）、.dat 解出（只支援 type 2）、type 2 / type 4 區塊重建。
//! 全部 little-endian。
use crate::R;
use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

pub fn u32le(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}

fn u16le(b: &[u8], p: usize) -> usize {
    u16::from_le_bytes(b[p..p + 2].try_into().unwrap()) as usize
}

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put16(v: &mut Vec<u8>, x: usize) {
    v.extend_from_slice(&(x as u16).to_le_bytes());
}

pub struct IndexFile {
    /// 這筆 entry 在 .index 裡的位置；data offset 存在 pt + 8
    pub pt: usize,
    pub data_offset: u32,
}

/// 資料夾 CRC → (檔名 CRC → entry)
pub type Index = HashMap<u32, HashMap<u32, IndexFile>>;

pub fn read_index(path: &str) -> R<Index> {
    let b = std::fs::read(path)?;
    if !b.starts_with(b"SqPack") {
        return Err("Not a SqPack file".into());
    }
    if u32le(&b, 0x14) != 2 {
        return Err("Not a index".into());
    }
    // 4 個 segment：1 是檔案、4 是資料夾
    let mut p = u32le(&b, 0xC) as usize + 4;
    let mut segs = [(0usize, 0usize); 4];
    for (i, seg) in segs.iter_mut().enumerate() {
        *seg = (u32le(&b, p + 4) as usize, u32le(&b, p + 8) as usize);
        p += 72 + if i == 0 { 4 } else { 0 };
    }
    let read_files = |offset: usize, count: usize, index2: bool| {
        let step = if index2 { 8 } else { 16 };
        (0..count)
            .map(|i| {
                let pt = offset + i * step;
                let data_offset = u32le(&b, pt + if index2 { 4 } else { 8 });
                (u32le(&b, pt), IndexFile { pt, data_offset })
            })
            .collect::<HashMap<_, _>>()
    };
    let mut index = Index::new();
    let (folder_off, folder_size) = segs[3];
    if folder_off != 0 {
        for i in 0..folder_size / 16 {
            let f = folder_off + i * 16;
            index.insert(u32le(&b, f), read_files(u32le(&b, f + 4) as usize, u32le(&b, f + 8) as usize / 16, false));
        }
    } else {
        let index2 = path.contains("index2");
        let count = if index2 { 2 } else { 1 } * segs[0].1 / 16;
        index.insert(0, read_files(segs[0].0, count, index2));
    }
    Ok(index)
}

/// 依 index 的 data offset 讀 .dat 檔，開過的 dat 保持開著（C# 版每次重開）。
pub struct Dats {
    index_path: String,
    files: HashMap<u32, File>,
}

impl Dats {
    pub fn new(index_path: &str) -> Dats {
        Dats { index_path: index_path.to_string(), files: HashMap::new() }
    }

    pub fn extract(&mut self, data_offset: u32) -> R<Vec<u8>> {
        let num = (data_offset & 0xF) / 2;
        if !self.files.contains_key(&num) {
            let dat = format!("dat{num}");
            let path = self.index_path.replace("index2", &dat).replace("index", &dat);
            // Rust 在 Windows 預設 share read|write，漢化中另一個 handle 追加寫入不會 sharing violation
            self.files.insert(num, File::open(path)?);
        }
        extract_at(self.files.get_mut(&num).unwrap(), (data_offset & !0xF) as u64 * 8)
    }
}

pub fn extract_at(f: &mut File, off: u64) -> R<Vec<u8>> {
    let mut h = [0u8; 24];
    f.seek(SeekFrom::Start(off))?;
    f.read_exact(&mut h)?;
    let (header_len, content_type, file_size, blocks) = (u32le(&h, 0), u32le(&h, 4), u32le(&h, 8), u32le(&h, 20));
    if content_type != 2 {
        // ponytail: 漢化只會解 EXH/EXD/root.exl（type 2）；type 3/4 需要時再從 git 歷史的 SqPackDatFile.java 移植
        return Err(format!("SqPack content type {content_type} not supported (offset {off:X})").into());
    }
    let mut table = vec![0u8; blocks as usize * 8];
    f.read_exact(&mut table)?;
    let mut out = Vec::with_capacity(file_size as usize);
    for i in 0..blocks as usize {
        let mut bh = [0u8; 16];
        f.seek(SeekFrom::Start(off + header_len as u64 + u32le(&table, i * 8) as u64))?;
        f.read_exact(&mut bh)?;
        let (compressed, size) = (u32le(&bh, 8) as usize, u32le(&bh, 12) as usize);
        let start = out.len();
        out.resize(start + size, 0);
        if compressed == 32000 || size == 1 {
            f.read_exact(&mut out[start..])?; // 未壓縮區塊
        } else {
            let mut c = vec![0u8; compressed];
            f.read_exact(&mut c)?;
            DeflateDecoder::new(&c[..]).read_exact(&mut out[start..])?;
        }
    }
    out.resize(file_size as usize, 0);
    Ok(out)
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut e = DeflateEncoder::new(Vec::new(), Compression::best());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

pub fn decompress(data: &[u8], size: usize) -> std::io::Result<Vec<u8>> {
    let mut out = vec![0u8; size];
    DeflateDecoder::new(data).read_exact(&mut out)?;
    Ok(out)
}

/// 補到 128 的倍數；剛好對齊時也再補一整個 128（與 Java/C# 版一致）。
fn pad128(n: usize) -> usize {
    if n < 128 { 128 } else { n + 128 - n % 128 }
}

/// 壓一個 part 接到 body 後面（16 bytes 區塊頭 + deflate + padding），回傳佔用長度。
fn append_part(body: &mut Vec<u8>, part: &[u8]) -> usize {
    let c = compress(part);
    let start = body.len();
    let size = pad128(c.len() + 16);
    for x in [16, 0, c.len() as u32, part.len() as u32] {
        put32(body, x);
    }
    body.extend_from_slice(&c);
    body.resize(start + size, 0);
    size
}

/// 切成最多 16000 bytes 的 deflate 區塊，組成 SqPack type 2 檔案。
pub fn build_block(data: &[u8]) -> Vec<u8> {
    let parts: Vec<&[u8]> = data.chunks(16000).collect();
    let header_len = pad128(24 + parts.len() * 8);
    let mut h = Vec::with_capacity(header_len);
    let mut body = Vec::new();
    for x in [header_len as u32, 2, data.len() as u32, 0, 0, parts.len() as u32] {
        put32(&mut h, x);
    }
    for part in &parts {
        let offset = body.len() as u32;
        let size = append_part(&mut body, part);
        put32(&mut h, offset);
        put16(&mut h, size);
        put16(&mut h, part.len());
    }
    let units = (body.len() as u32 / 128).to_le_bytes();
    h[12..16].copy_from_slice(&units);
    h[16..20].copy_from_slice(&units);
    h.resize(header_len, 0);
    h.extend_from_slice(&body);
    h
}

/// 重建 TEX 字型的 SqPack type 4 檔案。
// ponytail: 跟 Java/C# 版一樣假設只有一層 mip（字型 tex 都是），多層 mip 會讀超出資料而 panic。
pub fn build_tex_block(data: &[u8]) -> Vec<u8> {
    let mip_offset = u32le(data, 28) as usize;
    let tex_header = &data[..mip_offset];
    let (tex_type, width, height, mip_count) = (u16le(data, 4), u16le(data, 8), u16le(data, 10), u16le(data, 14));
    let part_count = data.len().div_ceil(16000);
    let header_len = pad128(24 + mip_count * 20 + part_count * 2);
    let mip_size = width * height * if tex_type == 5184 { 2 } else { 1 };

    let mut h = Vec::with_capacity(header_len);
    let mut body = Vec::new();
    for x in [header_len as u32, 4, data.len() as u32, 0, 0, mip_count as u32] {
        put32(&mut h, x);
    }
    let mut pos = mip_offset;
    for _ in 0..mip_count {
        put32(&mut h, mip_offset as u32);
        let length_pos = h.len();
        for x in [0, mip_size as u32, 0, part_count as u32] {
            put32(&mut h, x);
        }
        let mut mip_len = 0;
        for k in 1..=part_count {
            let size = if k == part_count { data.len() - pos } else { 16000 };
            let s = append_part(&mut body, &data[pos..pos + size]);
            pos += size;
            put16(&mut h, s);
            mip_len += s;
        }
        let units = (mip_len as u32 / 128).to_le_bytes();
        h[12..16].copy_from_slice(&units);
        h[16..20].copy_from_slice(&units);
        h[length_pos..length_pos + 4].copy_from_slice(&((mip_len + tex_header.len()) as u32).to_le_bytes());
    }
    h.resize(header_len, 0);
    h.extend_from_slice(tex_header);
    h.extend_from_slice(&body);
    let total = h.len();
    h.resize(total + 128 - total % 128, 0);
    h
}
