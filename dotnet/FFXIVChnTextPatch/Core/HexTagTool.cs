using System.Text;
using System.Text.RegularExpressions;

namespace FFXIVChnTextPatch.Core;

/// <summary>
/// 唯讀對照工具（<c>--hextags</c>）：掃描 resource/rawexd 下所有 CSV 的 &lt;hex:...&gt; 標籤，
/// 依 SaintCoinach 的 SeString TagType 解成人類可讀的標籤名，方便翻譯者辨認哪些 blob 不能動。
/// 報告寫到 repo 根目錄 hextags-report.txt。不修改任何檔案。
/// </summary>
public static class HexTagTool
{
    // 對照表：SeString 標籤起始位元組 0x02 後的 TagType。取自 SaintCoinach/Text/TagType.cs。
    private static readonly Dictionary<byte, string> TagNames = new()
    {
        [0x06] = "ResetTime",  [0x07] = "Time",       [0x08] = "If",
        [0x09] = "Switch",     [0x0A] = "Unknown0A",  [0x0C] = "IfEquals",
        [0x10] = "LineBreak",  [0x12] = "Gui",        [0x13] = "Color",
        [0x14] = "Unknown14",  [0x16] = "SoftHyphen", [0x17] = "Unknown17",
        [0x19] = "Emphasis2",  [0x1A] = "Emphasis",   [0x1D] = "Indent",
        [0x1E] = "CommandIcon",[0x1F] = "Dash",       [0x20] = "Value",
        [0x22] = "Format",     [0x24] = "TwoDigitValue", [0x28] = "Sheet",
        [0x29] = "Highlight",  [0x2B] = "Clickable",  [0x2C] = "Split",
        [0x2D] = "Unknown2D",  [0x2E] = "Fixed",      [0x2F] = "Unknown2F",
        [0x30] = "SheetJa",    [0x31] = "SheetEn",    [0x32] = "SheetDe",
        [0x33] = "SheetFr",    [0x40] = "InstanceContent",
        [0x48] = "UIForeground", [0x49] = "UIGlow",   [0x4A] = "RubyCharaters",
        [0x50] = "ZeroPaddedValue", [0x60] = "Unknown60",
    };

    private static readonly Regex HexTag = new(@"<hex:([0-9A-Fa-f]+)>", RegexOptions.Compiled);

    /// <summary>
    /// 把一個 hex 字串（如 "02100103"）解成可讀標籤名，如 "[LineBreak 01]"。
    /// 一段 &lt;hex:&gt; 就是一個 SeString 標籤（0x02 類型 …參數… 0x03），或是被匯出切開的參數片段。
    /// 只認第一個位元組後的 TagType；其餘位元組是機器參數，原樣以 hex 呈現、翻譯者不用管。
    /// ponytail: 不解析 SeString 的整數/字串長度編碼——翻譯者只需要標籤名，參數維持不透明。
    /// </summary>
    public static string Decode(string hex)
    {
        byte[] b;
        try { b = HexUtils.HexStringToBytes(hex); }
        catch { return "(格式錯誤)"; }

        if (b.Length == 1 && b[0] == 0x03) return "[end]";
        if (b.Length >= 2 && b[0] == 0x02 && TagNames.TryGetValue(b[1], out var name))
        {
            int len = b.Length;
            if (b[^1] == 0x03) len--;                       // 去掉結尾終止碼
            string pars = Convert.ToHexString(b, 2, len - 2);
            return pars.Length > 0 ? $"[{name} {pars}]" : $"[{name}]";
        }
        return Convert.ToHexString(b);                       // 落單參數片段
    }

    public static Task<string> RunAsync(IProgress<PatchProgress>? progress = null) =>
        Task.Run(() => Run(progress));

    public static string Run(IProgress<PatchProgress>? progress = null)
    {
        string rawexd = AppEnv.P("resource", "rawexd");
        var counts = new Dictionary<string, int>();
        var csvFiles = Directory.Exists(rawexd)
            ? Directory.GetFiles(rawexd, "*.csv", SearchOption.AllDirectories)
            : Array.Empty<string>();
        int done = 0;
        foreach (var csv in csvFiles)
        {
            progress?.Report(new(++done / (double)csvFiles.Length, "正在掃描：", Path.GetFileName(csv)));
            foreach (Match m in HexTag.Matches(File.ReadAllText(csv, Encoding.UTF8)))
            {
                string tag = m.Groups[1].Value.ToUpperInvariant();
                counts[tag] = counts.GetValueOrDefault(tag) + 1;
            }
        }

        var report = new StringBuilder();
        report.AppendLine($"hex 標籤對照表  {DateTime.Now:yyyy-MM-dd HH:mm:ss}");
        report.AppendLine($"相異標籤數：{counts.Count}");
        report.AppendLine("（依出現次數排序；解碼依 SaintCoinach SeString TagType）");
        report.AppendLine();
        report.AppendLine($"{"出現次數",10}  {"hex 標籤",-24}  解讀");
        foreach (var (tag, n) in counts.OrderByDescending(p => p.Value))
            report.AppendLine($"{n,10}  {"<hex:" + tag + ">",-24}  {Decode(tag)}");

        string outPath = AppEnv.P("hextags-report.txt");
        File.WriteAllText(outPath, report.ToString(), Encoding.UTF8);
        AppEnv.Log($"[HexTags] {counts.Count} 種標籤，報告寫到 {outPath}");
        return $"完成：{counts.Count} 種標籤 → hextags-report.txt";
    }
}
