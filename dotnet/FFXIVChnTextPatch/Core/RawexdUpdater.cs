using System.Diagnostics;
using System.IO.Compression;
using System.Text.RegularExpressions;

namespace FFXIVChnTextPatch.Core;

/// <summary>
/// 一鍵更新 rawexd 翻譯：git sparse clone 下載上游 repo 的 resource/rawexd（約 78MB，
/// 整包 zip 近 900MB 所以不可行）→ ZhConvert 簡轉繁 → RawexdMerge 逐格合併（本地翻譯優先）。
/// 合併前先把整個 resource/rawexd 備份成 backup/rawexd-before-update.zip（只保留最新一份）。
/// </summary>
public static class RawexdUpdater
{
    private const string DefaultRepo = "https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma";

    public static Task<(bool Ok, string Message)> UpdateAsync(IProgress<PatchProgress> progress) =>
        Task.Run(() => UpdateCore(progress)); // 跟 PatchAsync 一樣把重活丟離 UI 執行緒

    private static async Task<(bool Ok, string Message)> UpdateCore(IProgress<PatchProgress> progress)
    {
        string repo = Config.Get("UpstreamRepo") ?? DefaultRepo;
        string localDir = AppEnv.P("resource", "rawexd");
        string tmp = Path.Combine(Path.GetTempPath(), "ffxiv-rawexd-upstream");
        try
        {
            // 1. 下載（sparse clone 只抓 resource/rawexd 的內容）
            DeleteDir(tmp);
            await RunGit($"clone --depth 1 --filter=blob:none --sparse --progress {repo} \"{tmp}\"",
                progress, 0.02, 0.15, "正在下載上游翻譯……");
            await RunGit($"-C \"{tmp}\" sparse-checkout set resource/rawexd",
                progress, 0.15, 0.45, "正在下載上游翻譯……");
            string upDir = Path.Combine(tmp, "resource", "rawexd");
            if (!Directory.Exists(upDir))
                return (false, "上游 repo 裡找不到 resource/rawexd");

            // 2. 備份本地翻譯
            progress.Report(new(0.45, "正在備份本地翻譯……", ""));
            Directory.CreateDirectory(AppEnv.P("backup"));
            string backupZip = AppEnv.P("backup", "rawexd-before-update.zip");
            File.Delete(backupZip);
            ZipFile.CreateFromDirectory(localDir, backupZip);

            // 3. 逐檔簡轉繁 + 合併
            var files = Directory.GetFiles(upDir, "*.csv", SearchOption.AllDirectories);
            int filled = 0, newRows = 0, changed = 0, newFiles = 0, failed = 0;
            for (int i = 0; i < files.Length; i++)
            {
                string rel = Path.GetRelativePath(upDir, files[i]);
                progress.Report(new(0.5 + 0.5 * i / files.Length, "正在合併翻譯……", rel));
                try
                {
                    string upText = ZhConvert.S2Tw(File.ReadAllText(files[i]));
                    string localPath = Path.Combine(localDir, rel);
                    if (!File.Exists(localPath))
                    {
                        Directory.CreateDirectory(Path.GetDirectoryName(localPath)!);
                        File.WriteAllText(localPath, upText, Utf8(files[i]));
                        newFiles++;
                        continue;
                    }
                    string localText = ReadLocal(localPath, out bool wasBig5);
                    var r = RawexdMerge.Merge(localText, upText, localText.Contains("\r\n") ? "\r\n" : "\n");
                    if (r.Filled > 0 || r.NewRows > 0 || r.HeadersChanged || wasBig5)
                    {
                        File.WriteAllText(localPath, r.Merged, Utf8(localPath));
                        changed++;
                        filled += r.Filled;
                        newRows += r.NewRows;
                    }
                }
                catch (Exception ex)
                {
                    failed++;
                    AppEnv.Log($"合併失敗 {rel}: {ex.Message}");
                }
            }

            string msg = $"更新完成：{changed} 檔更新（補 {filled} 格、新增 {newRows} 列）、{newFiles} 個新檔" +
                         (failed > 0 ? $"、{failed} 檔失敗（見 debug.log）" : "") +
                         "。原檔已備份至 backup/rawexd-before-update.zip";
            AppEnv.Log(msg);
            return (failed == 0, msg);
        }
        catch (Exception ex)
        {
            AppEnv.Log("更新翻譯失敗: " + ex);
            return (false, "更新失敗：" + ex.Message);
        }
        finally
        {
            try { DeleteDir(tmp); } catch { /* 暫存目錄清不掉不影響結果 */ }
        }
    }

    /// <summary>跑 git 並把 stderr 的進度（"Receiving objects: 42%" 之類）映射到 from..to 區間。</summary>
    private static async Task RunGit(string args, IProgress<PatchProgress> progress, double from, double to, string action)
    {
        progress.Report(new(from, action, ""));
        var psi = new ProcessStartInfo("git", args)
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardError = true,
            RedirectStandardOutput = true,
        };
        Process proc;
        try { proc = Process.Start(psi)!; }
        catch (Exception ex) { throw new InvalidOperationException("無法執行 git，請先安裝 Git for Windows。" + ex.Message); }

        string lastLine = "";
        using (proc)
        {
            // git 進度用 \r 更新同一行，逐字元讀才拿得到
            var buf = new System.Text.StringBuilder();
            var reader = proc.StandardError;
            int c;
            while ((c = reader.Read()) >= 0)
            {
                if (c is '\r' or '\n')
                {
                    if (buf.Length == 0) continue;
                    lastLine = buf.ToString();
                    buf.Clear();
                    var m = Regex.Match(lastLine, @"(\d+)%");
                    double pct = m.Success ? int.Parse(m.Groups[1].Value) / 100.0 : 0;
                    progress.Report(new(from + (to - from) * pct, action, lastLine));
                }
                else buf.Append((char)c);
            }
            await proc.WaitForExitAsync();
            if (proc.ExitCode != 0)
                throw new InvalidOperationException($"git {args.Split(' ')[0]} 失敗：{lastLine}");
        }
    }

    private static readonly System.Text.Encoding Big5 = CreateBig5();

    private static System.Text.Encoding CreateBig5()
    {
        System.Text.Encoding.RegisterProvider(System.Text.CodePagesEncodingProvider.Instance);
        return System.Text.Encoding.GetEncoding(950);
    }

    /// <summary>讀本地 CSV：先嚴格 UTF-8，解不開就當舊 ConvertZZ 產出的 Big5
    /// （這些檔漢化主流程一直讀成亂碼；wasBig5 讓呼叫端強制重寫成 UTF-8 修復）。</summary>
    private static string ReadLocal(string path, out bool wasBig5)
    {
        byte[] bytes = File.ReadAllBytes(path);
        int start = bytes is [0xEF, 0xBB, 0xBF, ..] ? 3 : 0;
        try
        {
            string text = new System.Text.UTF8Encoding(false, true).GetString(bytes.AsSpan(start));
            wasBig5 = false;
            return text;
        }
        catch (System.Text.DecoderFallbackException)
        {
            wasBig5 = true;
            return Big5.GetString(bytes);
        }
    }

    private static System.Text.UTF8Encoding Utf8(string path)
    {
        using var s = File.OpenRead(path);
        Span<byte> b = stackalloc byte[3];
        return new(s.Read(b) == 3 && b[0] == 0xEF && b[1] == 0xBB && b[2] == 0xBF);
    }

    /// <summary>git 物件檔是唯讀的，Directory.Delete 直接刪會炸，先清屬性。</summary>
    private static void DeleteDir(string dir)
    {
        if (!Directory.Exists(dir)) return;
        foreach (var f in Directory.GetFiles(dir, "*", SearchOption.AllDirectories))
            File.SetAttributes(f, FileAttributes.Normal);
        Directory.Delete(dir, true);
    }
}
