using System.Text;

namespace FFXIVChnTextPatch.Core;

/// <summary>
/// rawexd CSV 逐儲存格合併：本地非空儲存格永遠保留，本地空格/缺列才補上游內容。
/// 輸出以上游的欄位配置為準（offset 列對齊，跨遊戲版本欄位增減也對得回正確位置），
/// 本地獨有的列附加在檔尾。
/// </summary>
public static class RawexdMerge
{
    /// <summary>有效列: [0]=key標頭 [1]=offset [2]=型別 [3..]=資料（'#' 開頭為註解，原樣保留）。</summary>
    public static (string Merged, int Filled, int NewRows, bool HeadersChanged) Merge(string localText, string upText, string nl)
    {
        var lo = Parse(localText);
        var up = Parse(upText);
        var loRows = lo.Where(r => r.Fields != null).Select(r => r.Fields!).ToList();
        var upRows = up.Where(r => r.Fields != null).Select(r => r.Fields!).ToList();
        if (loRows.Count < 3 || upRows.Count < 3)
            throw new InvalidDataException("CSV 缺少標頭列");

        // offset 值 → 欄索引，跨版本欄位對齊用
        var loOff = new Dictionary<string, int>();
        for (int j = 1; j < loRows[1].Count; j++) loOff[loRows[1][j]] = j;

        var loData = new Dictionary<string, List<string>>();
        for (int i = 3; i < loRows.Count; i++) loData[loRows[i][0]] = loRows[i];

        bool headersChanged = Enumerable.Range(0, 3).Any(i => !loRows[i].SequenceEqual(upRows[i]));
        int filled = 0, newRows = 0, upRowIdx = 0;
        var seen = new HashSet<string>();
        var sb = new StringBuilder();

        foreach (var (comment, fields) in up)
        {
            if (comment != null) { sb.Append(comment).Append(nl); continue; }
            if (upRowIdx++ < 3 || fields!.Count == 0) { WriteRow(sb, fields!, nl); continue; }

            string key = fields[0];
            seen.Add(key);
            if (loData.TryGetValue(key, out var lrow))
            {
                var merged = new List<string>(fields);
                // 有些列有多餘尾逗號（欄數比 offset 列多），超出的欄原樣放行
                for (int j = 1; j < merged.Count && j < upRows[1].Count; j++)
                {
                    if (loOff.TryGetValue(upRows[1][j], out int lj) && lj < lrow.Count && lrow[lj] != "")
                        merged[j] = lrow[lj];
                    else if (merged[j] != "")
                        filled++;
                }
                WriteRow(sb, merged, nl);
            }
            else
            {
                newRows++;
                WriteRow(sb, fields, nl);
            }
        }

        // 本地獨有的列: 依 offset 重排進上游欄位配置後附加在檔尾
        for (int i = 3; i < loRows.Count; i++)
        {
            if (seen.Contains(loRows[i][0])) continue;
            var row = new List<string> { loRows[i][0] };
            for (int j = 1; j < upRows[1].Count; j++)
                row.Add(loOff.TryGetValue(upRows[1][j], out int lj) && lj < loRows[i].Count ? loRows[i][lj] : "");
            WriteRow(sb, row, nl);
        }

        return (sb.ToString(), filled, newRows, headersChanged);
    }

    private static void WriteRow(StringBuilder sb, List<string> fields, string nl) =>
        sb.Append(string.Join(",", fields.Select(Encode))).Append(nl);

    private static string Encode(string f) =>
        f.Contains('"') || f.Contains(',') || f.Contains('\n') || f.Contains('\r') || f.StartsWith('#')
            ? "\"" + f.Replace("\"", "\"\"") + "\""
            : f;

    /// <summary>極簡 CSV parser: 支援引號欄位（含逗號/換行/雙引號跳脫），行首 '#' 為註解（原樣保留），空行略過。</summary>
    public static List<(string? Comment, List<string>? Fields)> Parse(string t)
    {
        var recs = new List<(string?, List<string>?)>();
        int i = 0, n = t.Length;
        while (i < n)
        {
            if (t[i] == '\r' || t[i] == '\n') { i++; continue; }
            if (t[i] == '#')
            {
                int e = i;
                while (e < n && t[e] != '\r' && t[e] != '\n') e++;
                recs.Add((t[i..e], null));
                i = e;
                continue;
            }
            var fields = new List<string>();
            while (true)
            {
                var sb = new StringBuilder();
                if (i < n && t[i] == '"')
                {
                    i++;
                    while (i < n)
                    {
                        if (t[i] == '"' && i + 1 < n && t[i + 1] == '"') { sb.Append('"'); i += 2; }
                        else if (t[i] == '"') { i++; break; }
                        else sb.Append(t[i++]);
                    }
                }
                while (i < n && t[i] != ',' && t[i] != '\r' && t[i] != '\n') sb.Append(t[i++]);
                fields.Add(sb.ToString());
                if (i < n && t[i] == ',') { i++; continue; }
                break;
            }
            recs.Add((null, fields));
        }
        return recs;
    }
}
