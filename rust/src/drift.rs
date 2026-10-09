//! rawexd 漂移偵測（純字串比對，不需遊戲或網路）。對應 C# LintTool 的 DetectDrift / DetectIdDrift / DuplicateKeys。
use crate::merge::{key_column, parse, qualifies_as_id, rows};
use std::collections::{BTreeMap, HashMap, HashSet};

/// 拿對齊正確的上游比對本地，回傳疑似「錯位」的 key。三個條件都成立才算：
///   1. 上游同 key 整列全空、本地該 key 有翻譯；
///   2. 本地那句話在上游別處出現過（條目被搬走，而非 fork 翻在上游前面）；
///   3. 該 key 上下最近的「兩邊都有值」錨點沒對齊（真的整塊位移，而非孤立補白）。
/// upstream 必須是已簡轉繁的版本，字串才對得起來。
pub fn detect_drift(local: &str, upstream: &str) -> Vec<i32> {
    // key → 代表值（該列第一個非空儲存格；整列全空則 None）
    fn rep(text: &str) -> BTreeMap<i32, Option<String>> {
        rows(&parse(text))
            .iter()
            .skip(3)
            .filter_map(|r| Some((r[0].parse().ok()?, r[1..].iter().find(|c| !c.is_empty()).cloned())))
            .collect()
    }
    let (up, lo) = (rep(upstream), rep(local));
    let up_vals: HashSet<&str> = up.values().flatten().map(String::as_str).collect();

    // 錨點：兩邊該 key 都有值；bool = 兩邊值相同（沒位移）。依 key 遞增。
    let anchors: Vec<(i32, bool)> = up
        .iter()
        .filter_map(|(k, uv)| Some((*k, lo.get(k)?.as_ref()? == uv.as_ref()?)))
        .collect();
    // 上下最近的錨點都對齊 → 這裡沒位移，只是孤立補白
    let neighbors_aligned = |key: i32| {
        let i = anchors.partition_point(|&(k, _)| k < key);
        i > 0 && anchors[i - 1].1 && i < anchors.len() && anchors[i].1
    };
    lo.iter()
        .filter(|(k, lv)| {
            lv.as_ref().is_some_and(|v| up_vals.contains(v.as_str()))
                && matches!(up.get(k), Some(None))
                && !neighbors_aligned(**k)
        })
        .map(|(k, _)| *k)
        .collect()
}

/// 有 TEXT-id 欄的表的精確漂移檢查（上游為準）：本地某 RowId 的 TEXT-id ≠ 上游 → 該列譯文會顯示在錯的遊戲列上。
/// 無 id 欄的表回空（交給 detect_drift）。本地獨有列（key 不在上游）不算。
pub fn detect_id_drift(local: &str, upstream: &str) -> Vec<i32> {
    let (lo_recs, up_recs) = (parse(local), parse(upstream));
    let (lo, up) = (rows(&lo_recs), rows(&up_recs));
    if lo.len() < 4 || up.len() < 4 {
        return Vec::new();
    }
    let (Some(lo_col), Some(up_col)) = (key_column(&lo), key_column(&up)) else { return Vec::new() };
    if !qualifies_as_id(&lo, &up, lo_col, up_col) {
        return Vec::new();
    }
    let up_by_key: HashMap<i32, &str> =
        up[3..].iter().filter_map(|r| Some((r[0].parse().ok()?, r.get(up_col)?.as_str()))).collect();
    let mut bad: Vec<i32> = lo[3..]
        .iter()
        .filter_map(|r| {
            let k = r[0].parse().ok()?;
            (*up_by_key.get(&k)? != r.get(lo_col)?.as_str()).then_some(k)
        })
        .collect();
    bad.sort();
    bad
}

/// 同一個 RowId 出現多次的 key。來源是 merge 的檔尾附加（上游刪/改名了該列的 TEXT-id）；
/// 套用時後面那列會蓋掉前面那列。
pub fn duplicate_keys(csv: &str) -> Vec<i32> {
    let recs = parse(csv);
    let mut seen = HashSet::new();
    let mut dup = Vec::new();
    for r in rows(&recs).iter().skip(3) {
        if !seen.insert(r[0].as_str()) {
            if let Ok(k) = r[0].parse() {
                if !dup.contains(&k) {
                    dup.push(k);
                }
            }
        }
    }
    dup.sort();
    dup
}
