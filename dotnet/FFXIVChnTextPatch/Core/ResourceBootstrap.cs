using System.IO.Compression;
using System.Net.Http;

namespace FFXIVChnTextPatch.Core;

/// <summary>
/// exe-only 首次執行時，從 GitHub Release 下載已打包好的 rawexd + opencc（不需 git）。
/// 字體檔太大（~248MB）不含在內，缺字體時只提示使用者自行到 repo 下載。
/// 契約：release 資產 zip 的「根目錄」要直接是 rawexd/ 與 opencc/ 兩個資料夾，
/// 會被解壓到 &lt;base&gt;/resource/ 底下。要換位址就設 ResourceZipUrl。
/// </summary>
public static class ResourceBootstrap
{
    private const string DefaultUrl =
        "https://github.com/dks50217/FFXIVChnTextPatch-MC/releases/latest/download/rawexd-opencc.zip";

    public const string RepoUrl = "https://github.com/dks50217/FFXIVChnTextPatch-MC";

    /// <summary>resource/rawexd 一個 CSV 都沒有就視為缺翻譯檔。</summary>
    public static bool NeedsCsv() => !PatchService.HasCsvFiles(AppEnv.P("resource", "rawexd"));

    /// <summary>resource/font 沒有任何檔就視為缺字體。</summary>
    public static bool HasFont()
    {
        string dir = AppEnv.P("resource", "font");
        return Directory.Exists(dir) && Directory.EnumerateFiles(dir).Any();
    }

    public static Task<(bool Ok, string Message)> DownloadAsync(IProgress<PatchProgress> progress) =>
        Task.Run(() => DownloadCore(progress));

    private static async Task<(bool Ok, string Message)> DownloadCore(IProgress<PatchProgress> progress)
    {
        string url = Config.Get("ResourceZipUrl") ?? DefaultUrl;
        string tmpZip = Path.Combine(Path.GetTempPath(), "ffxiv-resource.zip");
        try
        {
            using (var http = new HttpClient { Timeout = TimeSpan.FromMinutes(15) })
            using (var resp = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead))
            {
                resp.EnsureSuccessStatusCode();
                long? total = resp.Content.Headers.ContentLength;
                await using var netStream = await resp.Content.ReadAsStreamAsync();
                await using var fileStream = File.Create(tmpZip);
                var buf = new byte[81920];
                long done = 0;
                int n;
                while ((n = await netStream.ReadAsync(buf)) > 0)
                {
                    await fileStream.WriteAsync(buf.AsMemory(0, n));
                    done += n;
                    double pct = total is > 0 ? (double)done / total.Value : 0;
                    string detail = total is > 0
                        ? $"{done / 1048576}MB / {total.Value / 1048576}MB"
                        : $"{done / 1048576}MB";
                    progress.Report(new(pct * 0.9, "正在下載翻譯檔……", detail));
                }
            }

            progress.Report(new(0.92, "正在解壓縮……", ""));
            string resDir = AppEnv.P("resource");
            Directory.CreateDirectory(resDir);
            ZipFile.ExtractToDirectory(tmpZip, resDir, overwriteFiles: true);
            progress.Report(new(1, "完成", ""));

            if (NeedsCsv())
                return (false, "下載完成但仍找不到 CSV，請確認 zip 根目錄直接是 rawexd/ 資料夾");
            AppEnv.Log("翻譯檔下載並解壓完成。");
            return (true, "翻譯檔下載完成，可以開始漢化了");
        }
        catch (Exception ex)
        {
            AppEnv.Log("下載翻譯檔失敗: " + ex);
            return (false, "下載失敗：" + ex.Message);
        }
        finally
        {
            try { File.Delete(tmpZip); } catch { /* 暫存檔清不掉不影響結果 */ }
        }
    }
}
