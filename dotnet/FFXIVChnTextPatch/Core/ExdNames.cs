namespace FFXIVChnTextPatch.Core;

/// <summary>
/// EXD 表名 → 遊戲內文本位置說明。資料在 conf/exd-names.csv（來源：Souma 版 wiki，轉繁體），
/// 查無說明時各方法會退回原名，缺檔也不影響功能。
/// </summary>
public static class ExdNames
{
    private static readonly Lazy<Dictionary<string, string>> Map = new(Load);

    private static Dictionary<string, string> Load()
    {
        var map = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        string path = AppEnv.P("conf", "exd-names.csv");
        if (!File.Exists(path)) return map;
        foreach (var line in File.ReadLines(path))
        {
            if (line.StartsWith('#')) continue;
            int comma = line.IndexOf(',');
            if (comma <= 0) continue;
            string name = line[..comma].Trim();
            string desc = line[(comma + 1)..].Trim();
            if (name.Length > 0 && desc.Length > 0) map[name] = desc;
        }
        return map;
    }

    /// <summary>查表名說明。接受 "Item"、"Item.csv"、"EXD/Item.EXH"、"quest/000/xxx"、"quest/"（資料夾）
    /// 等形式；子目錄檔案退回第一段資料夾的說明；查無回傳 null。
    /// 資料夾與資料表可能撞名（quest/ 與 Quest），故資料夾 key 以 / 結尾。</summary>
    public static string? Describe(string sheetName)
    {
        string s = sheetName.Replace('\\', '/').TrimStart('/');
        if (s.StartsWith("exd/", StringComparison.OrdinalIgnoreCase)) s = s[4..];
        int dot = s.LastIndexOf('.');
        if (dot > 0) s = s[..dot];
        if (Map.Value.TryGetValue(s, out var desc)) return desc;
        int slash = s.IndexOf('/');
        if (slash > 0 && Map.Value.TryGetValue(s[..(slash + 1)], out desc)) return desc;
        return null;
    }

    /// <summary>"Item" → "Item（道具）"；查無說明時原樣回傳。</summary>
    public static string Label(string name)
    {
        var desc = Describe(name);
        return desc == null ? name : $"{name}（{desc}）";
    }
}
