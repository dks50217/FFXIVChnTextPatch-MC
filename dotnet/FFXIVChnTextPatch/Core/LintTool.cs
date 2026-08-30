using System.Text;
using Microsoft.VisualBasic.FileIO;

namespace FFXIVChnTextPatch.Core;

public record LintResult(int ErrorCount, int MissingSheetCount, string Summary);

/// <summary>
/// 翻譯 CSV 維護工具：檢查 resource/rawexd 下所有 CSV 會不會讓漢化中斷或整檔被跳過，
/// 統計翻譯覆蓋率，並（在遊戲路徑有效時）列出遊戲裡有字串欄位但缺少 CSV 的表。
/// 報告寫到 repo 根目錄 lint-report.txt。
/// </summary>
public static class LintTool
{
    public static Task<LintResult> RunAsync(IProgress<PatchProgress>? progress = null) =>
        Task.Run(() => Run(progress));

    public static LintResult Run(IProgress<PatchProgress>? progress = null)
    {
        var errors = new List<string>();
        var sayTodoZh = new List<string>();
        var coverage = new List<(string Name, int Translated, int Total)>();

        // ── 1. 檢查所有 CSV ──
        string rawexd = AppEnv.P("resource", "rawexd");
        var csvFiles = Directory.Exists(rawexd)
            ? Directory.GetFiles(rawexd, "*.csv", System.IO.SearchOption.AllDirectories)
            : Array.Empty<string>();
        int count = 0;
        foreach (var csvFile in csvFiles)
        {
            string name = Path.GetRelativePath(rawexd, csvFile).Replace('\\', '/');
            progress?.Report(new(++count / (double)csvFiles.Length * 0.7, "正在檢查：", name));
            LintCsv(csvFile, name, errors, sayTodoZh, coverage);
        }

        // ── 2. 缺少 CSV 的表（需要有效的遊戲路徑）──
        var missingSheets = new List<string>();
        string? missingNote = null;
        var gamePath = Config.Get("GamePath");
        if (PatchService.IsFFXIVFolder(gamePath))
        {
            try
            {
                missingSheets = FindMissingSheets(gamePath!, rawexd, progress);
            }
            catch (Exception ex)
            {
                missingNote = "讀取遊戲檔案失敗，略過缺表檢查：" + ex.Message;
                AppEnv.Log("[Lint] " + missingNote);
            }
        }
        else
        {
            missingNote = "遊戲路徑未設定或無效，略過缺表檢查。";
        }

        // ── 3. 輸出報告 ──
        int totalTranslated = coverage.Sum(c => c.Translated);
        int totalCells = coverage.Sum(c => c.Total);
        double totalRatio = totalCells == 0 ? 0 : totalTranslated * 100.0 / totalCells;

        var report = new StringBuilder();
        report.AppendLine($"翻譯 CSV 檢查報告  {DateTime.Now:yyyy-MM-dd HH:mm:ss}");
        report.AppendLine($"CSV 檔數：{csvFiles.Length}");
        report.AppendLine();

        report.AppendLine($"■ 錯誤（{errors.Count}）—— 會中斷漢化流程或讓整檔被跳過");
        if (errors.Count == 0) report.AppendLine("（無）");
        foreach (var error in errors) report.AppendLine(error);
        report.AppendLine();

        report.AppendLine($"■ 說話任務仍是中文的 SAYTODO（{sayTodoZh.Count}）—— 國際服不建議打中文，需改成英文");
        if (sayTodoZh.Count == 0) report.AppendLine("（無）");
        foreach (var s in sayTodoZh) report.AppendLine(s);
        report.AppendLine();

        report.AppendLine($"■ 遊戲中有字串欄位但缺少 CSV 的表（{missingSheets.Count}）—— 這些表會維持原文");
        if (missingNote != null) report.AppendLine(missingNote);
        else if (missingSheets.Count == 0) report.AppendLine("（無）");
        foreach (var sheet in missingSheets) report.AppendLine(sheet);
        report.AppendLine();

        report.AppendLine($"■ 覆蓋率（非空欄位 / 總欄位），總計 {totalTranslated}/{totalCells}（{totalRatio:0.0}%）");
        foreach (var (name, translated, total) in coverage.OrderBy(c => c.Total == 0 ? 1.0 : c.Translated / (double)c.Total))
        {
            double ratio = total == 0 ? 100 : translated * 100.0 / total;
            report.AppendLine($"{ratio,6:0.0}%  {translated}/{total}  {ExdNames.Label(name)}");
        }

        string reportPath = Path.Combine(AppEnv.BaseDir, "lint-report.txt");
        File.WriteAllText(reportPath, report.ToString(), new UTF8Encoding(false));

        string summary = $"檢查完成：{errors.Count} 個錯誤、缺 {missingSheets.Count} 張表、覆蓋率 {totalRatio:0.0}%（詳見 lint-report.txt）";
        return new LintResult(errors.Count, missingSheets.Count, summary);
    }

    /// <summary>
    /// 漂移偵測：拿對齊正確的上游比對本地，回傳疑似「錯位」的 key。三個條件都成立才算：
    ///   1. 上游同 key 整列全空、本地該 key 有翻譯；
    ///   2. 本地那句話在上游別處出現過（代表條目被搬走，而非 fork 翻在上游前面）；
    ///   3. 該 key 上下最近的「兩邊都有值」錨點沒對齊（真的整塊位移，而非孤立補白）。
    ///
    /// 第 3 條專門排掉「常見/重複字串，上游留白、本地補上」的合理情況（如 NPC 反覆喊同一句、
    /// 一排都叫「攻擊」）：那種上下鄰列 local 跟 upstream 仍對得齊。upstreamText 必須是**已簡轉繁**
    /// 的版本，字串才對得起來。純字串比對，不需遊戲或網路。
    /// </summary>
    public static List<int> DetectDrift(string localText, string upstreamText)
    {
        // key → 代表值（該列第一個非空儲存格；整列全空則 null）
        static SortedDictionary<int, string?> Rep(string text)
        {
            var m = new SortedDictionary<int, string?>();
            foreach (var r in RawexdMerge.Parse(text).Where(r => r.Fields != null).Select(r => r.Fields!).Skip(3))
                if (int.TryParse(r[0], out int key))
                    m[key] = r.Skip(1).FirstOrDefault(c => !string.IsNullOrEmpty(c));
            return m;
        }

        var up = Rep(upstreamText);
        var lo = Rep(localText);
        var upVals = new HashSet<string>(up.Values.Where(v => v != null)!);

        // 錨點：兩邊該 key 都有值；aligned = 兩邊值相同（沒位移）。up 已排序，故 anchorKeys 遞增。
        var anchorKeys = new List<int>();
        var aligned = new Dictionary<int, bool>();
        foreach (var (k, uv) in up)
            if (uv != null && lo.TryGetValue(k, out var lv) && lv != null)
            {
                anchorKeys.Add(k);
                aligned[k] = lv == uv;
            }

        // 上下最近的錨點是否都對齊 → 是的話代表這裡沒位移，只是孤立補白
        bool NeighborsAligned(int key)
        {
            int i = anchorKeys.BinarySearch(key);
            if (i < 0) i = ~i;                       // 插入點 = 第一個 > key 的錨點
            bool prevOk = i - 1 >= 0 && aligned[anchorKeys[i - 1]];
            bool nextOk = i < anchorKeys.Count && aligned[anchorKeys[i]];
            return prevOk && nextOk;
        }

        var suspects = new List<int>();
        foreach (var (key, lv) in lo)
            if (lv != null
                && up.TryGetValue(key, out var uv) && uv == null   // 上游同 key 全空
                && upVals.Contains(lv)                             // 本地值在上游別處出現
                && !NeighborsAligned(key))                         // 上下未對齊 → 真的位移
                suspects.Add(key);
        suspects.Sort();
        return suspects;
    }

    private static void LintCsv(string path, string name, List<string> errors,
        List<string> sayTodoZh, List<(string, int, int)> coverage)
    {
        var rows = new List<string[]>();
        var lineNumbers = new List<long>();
        try
        {
            using var parser = new TextFieldParser(path, Encoding.UTF8);
            parser.TextFieldType = FieldType.Delimited;
            parser.SetDelimiters(",");
            parser.HasFieldsEnclosedInQuotes = true;
            parser.TrimWhiteSpace = false;
            parser.CommentTokens = new[] { "#" };
            while (!parser.EndOfData)
            {
                lineNumbers.Add(parser.LineNumber);
                rows.Add(parser.ReadFields()!);
            }
        }
        catch (MalformedLineException ex)
        {
            errors.Add($"{name}: CSV 格式錯誤（第 {ex.LineNumber} 行）→ 整檔會被跳過");
            return;
        }

        if (rows.Count < 2)
        {
            errors.Add($"{name}: 列數不足（{rows.Count}）→ 整檔會被跳過");
            return;
        }
        // 第 2 列（index 1）是 offset 列，欄位必須是整數
        for (int i = 1; i < rows[1].Length; i++)
        {
            if (!int.TryParse(rows[1][i], out _))
            {
                errors.Add($"{name}: offset 列第 {i + 1} 欄不是整數（\"{rows[1][i]}\"）→ 整檔會被跳過");
                return;
            }
        }
        if (rows.Count < 4)
            return; // 只有表頭沒有資料列（空表）：漢化流程視為無翻譯，不是錯誤

        int expectedWidth = rows[1].Length;
        int translated = 0, total = 0;
        for (int r = 3; r < rows.Count; r++)
        {
            var row = rows[r];
            long line = lineNumbers[r];
            if (!int.TryParse(row[0], out _))
            {
                errors.Add($"{name} 第 {line} 行: key \"{row[0]}\" 不是整數 → 整檔會被跳過");
                return;
            }
            if (row.Length < expectedWidth)
                errors.Add($"{name} 第 {line} 行: 欄位數 {row.Length} 少於 offset 列的 {expectedWidth} → 漢化可能中斷");
            // 說話任務：SAYTODO 是玩家實際要輸入的字，國際服不建議打中文，仍含中文就提醒改英文
            if (row.Any(cell => cell.Contains("_SAYTODO_", StringComparison.OrdinalIgnoreCase)))
            {
                var zh = row.FirstOrDefault(cell => !cell.Contains("SAYTODO", StringComparison.OrdinalIgnoreCase) && HasCjk(cell));
                if (zh != null)
                    sayTodoZh.Add($"{name} 第 {line} 行: 「{Truncate(zh)}」");
            }
            for (int c = 1; c < row.Length; c++)
            {
                total++;
                if (string.IsNullOrEmpty(row[c])) continue;
                translated++;
                CheckHexTags(row[c], $"{name} 第 {line} 行第 {c + 1} 欄", errors);
            }
        }
        coverage.Add((name, translated, total));
    }

    /// <summary>依 PatchService.AppendCsvString 的實際行為，找出會讓它丟例外或寫出壞資料的 hex 標籤。</summary>
    private static void CheckHexTags(string cell, string location, List<string> errors)
    {
        bool isHex = false;
        int tagStart = -1;
        for (int i = 0; i < cell.Length; i++)
        {
            char c = cell[i];
            if (c == '<')
            {
                if (i + 3 < cell.Length && cell[i + 1] == 'h' && cell[i + 2] == 'e' && cell[i + 3] == 'x')
                {
                    if (isHex)
                    {
                        errors.Add($"{location}: 巢狀 hex 標籤（TagInTag）→ 會中斷漢化");
                        return;
                    }
                    isHex = true;
                    tagStart = i;
                }
            }
            else if (c == '>' && isHex)
            {
                string tag = cell[tagStart..(i + 1)];
                if (tag.Length < 6 || tag[4] != ':')
                {
                    errors.Add($"{location}: hex 標籤格式錯誤（缺少冒號）\"{Truncate(tag)}\" → 會中斷漢化");
                }
                else
                {
                    string body = tag[5..^1];
                    if (body.Length % 2 != 0 || !body.All(Uri.IsHexDigit))
                        errors.Add($"{location}: hex 內容無效 \"{Truncate(tag)}\" → 會中斷漢化");
                }
                isHex = false;
            }
        }
        if (isHex)
            errors.Add($"{location}: hex 標籤沒有關閉的 '>' → 標籤會被當成一般文字寫入（遊戲顯示錯誤）");
    }

    private static string Truncate(string s) => s.Length <= 40 ? s : s[..40] + "…";

    private static bool HasCjk(string s)
    {
        foreach (char c in s)
            if (c >= 0x4E00 && c <= 0x9FFF) return true;
        return false;
    }

    private static List<string> FindMissingSheets(string gamePath, string rawexd, IProgress<PatchProgress>? progress)
    {
        string pathToIndex = Path.Combine(gamePath, "game", "sqpack", "ffxiv", "0a0000.win32.index");
        var fileList = PatchService.InitFileList(pathToIndex);
        var index = new SqPackIndex(pathToIndex).ResolveIndex();
        var missing = new List<string>();
        int count = 0;
        foreach (var replaceFile in fileList) // "EXD/Xxx.EXH"
        {
            progress?.Report(new(0.7 + ++count / (double)fileList.Count * 0.3, "正在比對遊戲資料表：", replaceFile));
            if (!replaceFile.ToUpperInvariant().EndsWith(".EXH")) continue;
            string sheetName = replaceFile[4..replaceFile.IndexOf('.')];
            if (File.Exists(Path.Combine(rawexd, sheetName.Replace('/', Path.DirectorySeparatorChar) + ".csv")))
                continue;

            string filePath = replaceFile[..replaceFile.LastIndexOf('/')];
            string fileName = replaceFile[(replaceFile.LastIndexOf('/') + 1)..];
            int filePathCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes(filePath.ToLowerInvariant()));
            int exhFileCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes(fileName.ToLowerInvariant()));
            if (!index.TryGetValue(filePathCrc, out var folder)) continue;
            if (!folder.Files.TryGetValue(exhFileCrc, out var exhIndexFile)) continue;
            EXHFFile exh;
            try { exh = new EXHFFile(PatchService.ExtractFile(pathToIndex, exhIndexFile.DataOffset)); }
            catch { continue; }
            if (exh.Langs.Length == 0) continue; // 無語言版本的表，漢化流程本來就跳過
            int stringColumns = exh.Datasets.Count(d => d.Type == 0);
            if (stringColumns > 0)
                missing.Add($"{ExdNames.Label(sheetName)}，{stringColumns} 個字串欄位");
        }
        missing.Sort(StringComparer.OrdinalIgnoreCase);
        return missing;
    }
}
