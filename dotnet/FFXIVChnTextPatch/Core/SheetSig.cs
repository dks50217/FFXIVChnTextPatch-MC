using System.Diagnostics;
using System.IO.Compression;
using System.Text;
using System.Text.RegularExpressions;

namespace FFXIVChnTextPatch.Core;

/// <summary>
/// Sheet 標籤參照檢查：譯文的 Sheet 標籤參數（去哪張表、讀第幾欄）必須跟日文原文一致，
/// 照陸版欄號翻過來會讓該介面閃退（2026-09 莫古莫古指南書，Addon row 15919）。
/// 只比 Sheet：換行、顏色、If 分支長度本來就會隨譯文變動。
/// <see cref="Generate"/> 從 SaintCoinach 日文匯出產生 resource/ja-sheetsig.txt.gz（需本機有遊戲），
/// <see cref="Check"/> 只讀那個檔，CI 不需要遊戲。
/// </summary>
public static class SheetSig
{
    private const string RefName = "ja-sheetsig.txt.gz";
    private static readonly Regex Chunk = new(@"<hex:([0-9A-Fa-f]*)>", RegexOptions.Compiled);

    /// <summary>Sheet 系列的 TagType（見 HexTagTool 對照表）。</summary>
    private static readonly HashSet<string> SheetTags = new() { "28", "30", "31", "32", "33" };

    /// <summary>
    /// 取出一格裡的 Sheet 標籤與其參數片段，串成簽章。
    /// 匯出會把一個標籤切成數段：02 開頭是標籤起頭，其餘是參數片段。
    /// </summary>
    public static string Of(string cell)
    {
        var parts = new List<string>();
        bool collecting = false;
        foreach (Match m in Chunk.Matches(cell))
        {
            string h = m.Groups[1].Value.ToUpperInvariant();
            if (h.StartsWith("02") && h.Length >= 4)
            {
                collecting = SheetTags.Contains(h[2..4]);
                if (collecting) parts.Add(h);
            }
            else if (collecting) parts.Add(h);
        }
        return string.Join("|", parts);
    }

    /// <summary>rawexd CSV 文字 → (rowId, 欄index) → 儲存格；資料列從第 4 個有效列開始。</summary>
    private static Dictionary<(int Row, int Col), string> Cells(string csvText)
    {
        var map = new Dictionary<(int, int), string>();
        var recs = RawexdMerge.Parse(csvText).Where(r => r.Fields != null).ToList();
        for (int i = 3; i < recs.Count; i++)
        {
            var f = recs[i].Fields!;
            if (f.Count == 0 || !int.TryParse(f[0], out int row)) continue;
            for (int c = 1; c < f.Count; c++)
                if (f[c].Length > 0) map[(row, c - 1)] = f[c];
        }
        return map;
    }

    // ── 產生參照檔 ──

    public static Task<string> GenerateAsync(string jaDir, IProgress<PatchProgress>? progress = null) =>
        Task.Run(() => Generate(jaDir, progress));

    public static string Generate(string jaDir, IProgress<PatchProgress>? progress = null)
    {
        if (!Directory.Exists(jaDir))
            return $"找不到日文匯出目錄：{jaDir}";

        // SaintCoinach 輸出在 <版本>/rawexd 底下
        string version = new DirectoryInfo(jaDir).Parent?.Name ?? "unknown";

        var sb = new StringBuilder();
        sb.Append("# ja-sheetsig v1  game=").Append(version)
          .Append("  generated=").Append(DateTime.Now.ToString("yyyy-MM-dd")).Append('\n');
        sb.Append("# <csv>,<row>,<col>,<Sheet 標籤與參數片段，以 | 分隔>\n");

        var files = Directory.GetFiles(jaDir, "*.csv", SearchOption.AllDirectories);
        int cells = 0;
        for (int i = 0; i < files.Length; i++)
        {
            string rel = Path.GetRelativePath(jaDir, files[i]).Replace('\\', '/');
            progress?.Report(new((i + 1) / (double)files.Length, "正在產生參照檔：", rel));
            Dictionary<(int, int), string> map;
            try { map = Cells(File.ReadAllText(files[i], Encoding.UTF8)); }
            catch (Exception ex) { AppEnv.Log($"[SheetSig] 讀取失敗 {rel}: {ex.Message}"); continue; }

            foreach (var ((row, col), cell) in map.OrderBy(k => k.Key.Item1).ThenBy(k => k.Key.Item2))
            {
                string sig = Of(cell);
                if (sig.Length == 0) continue;
                sb.Append(rel).Append(',').Append(row).Append(',').Append(col).Append(',').Append(sig).Append('\n');
                cells++;
            }
        }

        string outPath = AppEnv.P("resource", RefName);
        using (var fs = File.Create(outPath))
        using (var gz = new GZipStream(fs, CompressionLevel.SmallestSize))
            gz.Write(Encoding.UTF8.GetBytes(sb.ToString()));

        long kb = new FileInfo(outPath).Length / 1024;
        AppEnv.Log($"[SheetSig] {cells} 格、{kb}KB → {outPath}");
        return $"完成：{cells} 格含 Sheet 標籤，{kb}KB → resource/{RefName}（遊戲版本 {version}）";
    }

    // ── 檢查 ──

    private static (string Version, Dictionary<string, string> Sigs)? LoadRef()
    {
        string path = AppEnv.P("resource", RefName);
        if (!File.Exists(path)) return null;
        using var fs = File.OpenRead(path);
        using var gz = new GZipStream(fs, CompressionMode.Decompress);
        using var sr = new StreamReader(gz, Encoding.UTF8);
        string version = "unknown";
        var sigs = new Dictionary<string, string>();
        while (sr.ReadLine() is { } line)
        {
            if (line.StartsWith('#'))
            {
                var m = Regex.Match(line, @"game=(\S+)");
                if (m.Success) version = m.Groups[1].Value;
                continue;
            }
            // <csv>,<row>,<col>,<sig>；簽章不含逗號
            int a = line.IndexOf(','), b = line.IndexOf(',', a + 1), c = line.IndexOf(',', b + 1);
            if (a < 0 || b < 0 || c < 0) continue;
            sigs[line[..c]] = line[(c + 1)..];
        }
        return (version, sigs);
    }

    public static Task<int> CheckAsync(string? baseRef = null, IProgress<PatchProgress>? progress = null) =>
        Task.Run(() => Check(baseRef, progress));

    /// <summary>baseRef 有給就只檢查相對該 ref 有變動的格子，沒給就掃全部。回傳被標記的格數。</summary>
    public static int Check(string? baseRef = null, IProgress<PatchProgress>? progress = null)
    {
        var report = new StringBuilder();
        report.AppendLine($"Sheet 標籤參照檢查  {DateTime.Now:yyyy-MM-dd HH:mm:ss}");

        var loaded = LoadRef();
        if (loaded == null)
        {
            report.AppendLine($"找不到 resource/{RefName}，略過檢查。");
            report.AppendLine("請在本機用 --gensheetsig <SaintCoinach 日文匯出的 rawexd 目錄> 產生後 commit。");
            Write(report);
            return 0;
        }
        var (refVersion, sigs) = loaded.Value;
        report.AppendLine($"參照檔遊戲版本：{refVersion}（{sigs.Count} 格）");

        string? gameVersion = PatchService.GameVersion();
        if (gameVersion != null && gameVersion != refVersion)
            report.AppendLine($"⚠ 參照檔是 {refVersion} 產的，本機遊戲是 {gameVersion}，"
                            + "結果可能不準，建議重新產生參照檔。");

        string rawexd = AppEnv.P("resource", "rawexd");
        List<string> targets;
        if (baseRef != null)
        {
            // 對 merge-base，避免 base 前進後把別人的改動也算進來；比工作區而非 HEAD，commit 前就能跑
            string mergeBase = Git($"merge-base {baseRef} HEAD")?.Trim() ?? baseRef;
            var changed = Git($"diff --name-only {mergeBase} -- resource/rawexd");
            if (changed == null)
            {
                report.AppendLine($"⚠ git diff 失敗（base ref: {baseRef}），略過檢查。");
                Write(report);
                return 0;
            }
            baseRef = mergeBase;
            targets = changed.Split('\n', StringSplitOptions.RemoveEmptyEntries)
                             .Select(p => p.Trim()).Where(p => p.EndsWith(".csv")).ToList();
            report.AppendLine($"比對範圍：相對 {baseRef} 有變動的 {targets.Count} 個 CSV");
        }
        else
        {
            targets = Directory.Exists(rawexd)
                ? Directory.GetFiles(rawexd, "*.csv", SearchOption.AllDirectories)
                    .Select(p => "resource/rawexd/" + Path.GetRelativePath(rawexd, p).Replace('\\', '/')).ToList()
                : new List<string>();
            report.AppendLine($"比對範圍：全部 {targets.Count} 個 CSV");
        }

        var hits = new List<string>();
        int checkedCells = 0;
        for (int i = 0; i < targets.Count; i++)
        {
            string repoPath = targets[i];
            string rel = repoPath["resource/rawexd/".Length..];
            progress?.Report(new((i + 1) / (double)Math.Max(targets.Count, 1), "正在檢查 Sheet 參照：", rel));

            string localPath = Path.Combine(rawexd, rel.Replace('/', Path.DirectorySeparatorChar));
            if (!File.Exists(localPath)) continue;

            Dictionary<(int, int), string> now;
            try { now = Cells(File.ReadAllText(localPath, Encoding.UTF8)); }
            catch (Exception ex) { report.AppendLine($"!! {rel}：{ex.Message}"); continue; }

            // diff 模式只看值真的變了的格子
            Dictionary<(int, int), string>? before = null;
            if (baseRef != null)
            {
                string? oldText = Git($"show {baseRef}:{repoPath}");
                before = oldText == null ? new() : Cells(oldText);
            }

            foreach (var ((row, col), cell) in now)
            {
                if (before != null && before.TryGetValue((row, col), out var old) && old == cell) continue;
                string sig = Of(cell);
                if (sig.Length == 0) continue;
                checkedCells++;
                if (!sigs.TryGetValue($"{rel},{row},{col}", out var jaSig)) continue;
                if (sig != jaSig)
                    hits.Add($"{rel}  row {row} 欄{col}\n"
                           + $"    日文原文：{jaSig}\n"
                           + $"    本地譯文：{sig}");
            }
        }

        report.AppendLine($"檢查了 {checkedCells} 格含 Sheet 標籤的譯文");
        report.AppendLine();
        report.AppendLine($"■ Sheet 參照與日文原文不符（{hits.Count}）—— 可能讓該介面閃退，請逐格確認");
        if (hits.Count == 0) report.AppendLine("（無）");
        foreach (var h in hits) report.AppendLine(h);
        Write(report);
        AppEnv.Log($"[SheetSig] {hits.Count} 處不符");
        return hits.Count;
    }

    private static void Write(StringBuilder report) =>
        File.WriteAllText(AppEnv.P("sheetsig-report.txt"), report.ToString(), Encoding.UTF8);

    /// <summary>跑 git 拿 stdout，失敗回 null。</summary>
    private static string? Git(string args)
    {
        try
        {
            var psi = new ProcessStartInfo("git", args)
            {
                WorkingDirectory = AppEnv.BaseDir,
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                StandardOutputEncoding = Encoding.UTF8,
            };
            using var p = Process.Start(psi)!;
            string output = p.StandardOutput.ReadToEnd();
            p.WaitForExit();
            return p.ExitCode == 0 ? output : null;
        }
        catch (Exception ex)
        {
            AppEnv.Log("[SheetSig] git 執行失敗：" + ex.Message);
            return null;
        }
    }
}
