using System.Diagnostics;
using System.Text;
using Microsoft.VisualBasic.FileIO;

namespace FFXIVChnTextPatch.Core;

public record PatchProgress(double Percent, string Action, string Detail);

/// <summary>
/// 漢化主流程。移植自 ReplaceThread/ReplaceEXDF/ReplaceFont/RollbackThread。
/// ponytail: 只支援 CSV 翻譯來源（FLanguage=CSV，即目前設定與建議模式）；
/// 舊的「CN 客戶端檔案」模式（EXDFUtil/JianFan/transtable/TransMode）未移植，需要時再從 Java 版補。
/// </summary>
public class PatchService
{
    private static readonly string[] ResourceNames =
    {
        "000000.win32.dat0", "000000.win32.index", "000000.win32.index2",
        "0a0000.win32.dat0", "0a0000.win32.index", "0a0000.win32.index2"
    };

    public static bool IsFFXIVFolder(string? path) =>
        !string.IsNullOrEmpty(path) && File.Exists(Path.Combine(path, "game", "ffxiv_dx11.exe"));

    private static string SqpackFolder(string gamePath) =>
        Path.Combine(gamePath, "game", "sqpack", "ffxiv");

    private static bool IsGameRunning() =>
        Process.GetProcessesByName("ffxiv_dx11").Length > 0;

    /// <summary>讀 game/ffxivgame.ver 的遊戲版本，讀不到回傳 null。</summary>
    public static string? GameVersion()
    {
        var gamePath = Config.Get("GamePath");
        if (string.IsNullOrEmpty(gamePath)) return null;
        var verFile = Path.Combine(gamePath, "game", "ffxivgame.ver");
        try { return File.Exists(verFile) ? File.ReadAllText(verFile).Trim() : null; }
        catch { return null; }
    }

    /// <summary>主畫面顯示的漢化狀態（依據上次漢化時記下的遊戲版本）。</summary>
    public static (string Text, bool Warn) PatchStatus()
    {
        string? patched = Config.Get("PatchedVersion");
        if (string.IsNullOrEmpty(patched)) return ("目前狀態：未漢化", false);
        string? game = GameVersion();
        if (game != null && game != patched)
            return ($"⚠ 遊戲已從 {patched} 更新至 {game}，漢化已被覆蓋，請重新漢化", true);
        return ($"目前狀態：已漢化（遊戲版本 {patched}）", false);
    }

    /// <summary>執行漢化（含備份）。Ok=true 時 Message 是成功摘要，否則是錯誤訊息。</summary>
    public async Task<(bool Ok, string Message)> PatchAsync(IProgress<PatchProgress> progress)
    {
        var gamePath = Config.Get("GamePath");
        if (!IsFFXIVFolder(gamePath))
            return (false, "請選擇正確的遊戲根目錄（目錄內應有 game\\ffxiv_dx11.exe）");
        if (IsGameRunning())
            return (false, "偵測到 FFXIV 正在執行中，請先關閉遊戲再進行漢化");
        string resourceFolder = SqpackFolder(gamePath!);
        try
        {
            string summary = "漢化完畢";
            await Task.Run(() =>
            {
                Backup(resourceFolder);
                if (Config.Get("ReplaFont") == "1")
                    ReplaceFont(Path.Combine(resourceFolder, "000000.win32.index"), AppEnv.P("resource", "font"), progress);
                else
                    AppEnv.Log("Skip replacing font files.");

                if (Config.Get("ReplaText") == "1")
                {
                    if (Config.Get("FLanguage") == "CSV" && HasCsvFiles(AppEnv.P("resource", "rawexd")))
                    {
                        AppEnv.Log("Start patching with CSV files.");
                        summary = ReplaceExdf(Path.Combine(resourceFolder, "0a0000.win32.index"), progress);
                    }
                    else
                    {
                        throw new InvalidOperationException(
                            "找不到 CSV 翻譯檔（resource/rawexd），或 FLanguage 不是 CSV。此版本僅支援 CSV 模式。");
                    }
                }
                else
                {
                    AppEnv.Log("Skip replacing text.");
                }
            });
            AppEnv.Log("Patch finished. " + summary);
            Config.Set("PatchedVersion", GameVersion() ?? "");
            Config.Save();
            return (true, summary);
        }
        catch (Exception ex)
        {
            AppEnv.Log("Patch failed! " + ex);
            return (false, "漢化失敗：" + ex.Message);
        }
    }

    /// <summary>從 backup/ 還原六個資源檔。</summary>
    public async Task<(bool Ok, string Message)> RollbackAsync(IProgress<PatchProgress> progress)
    {
        var gamePath = Config.Get("GamePath");
        if (!IsFFXIVFolder(gamePath))
            return (false, "請選擇正確的遊戲根目錄（目錄內應有 game\\ffxiv_dx11.exe）");
        if (IsGameRunning())
            return (false, "偵測到 FFXIV 正在執行中，請先關閉遊戲再進行還原");
        string resourceFolder = SqpackFolder(gamePath!);
        try
        {
            await Task.Run(() =>
            {
                AppEnv.Log("[Rollback] Start rollback.");
                int fileCount = 0;
                foreach (var name in ResourceNames)
                {
                    progress.Report(new(++fileCount / (double)ResourceNames.Length, "正在還原……", name));
                    var backupFile = AppEnv.P("backup", name);
                    if (File.Exists(backupFile))
                    {
                        AppEnv.Log($"[Rollback] {name}, {new FileInfo(backupFile).Length / 1024}KB");
                        File.Copy(backupFile, Path.Combine(resourceFolder, name), overwrite: true);
                    }
                }
                AppEnv.Log("[Rollback] Rollback completed.");
            });
            Config.Set("PatchedVersion", "");
            Config.Save();
            return (true, "還原完畢");
        }
        catch (Exception ex)
        {
            AppEnv.Log("Rollback failed! " + ex);
            return (false, "還原失敗：" + ex.Message);
        }
    }

    private static void Backup(string resourceFolder)
    {
        Directory.CreateDirectory(AppEnv.P("backup"));
        foreach (var name in ResourceNames)
        {
            var src = Path.Combine(resourceFolder, name);
            if (File.Exists(src))
                File.Copy(src, AppEnv.P("backup", name), overwrite: true);
        }
    }

    public static bool HasCsvFiles(string directory) =>
        Directory.Exists(directory) &&
        Directory.EnumerateFiles(directory, "*.csv", System.IO.SearchOption.AllDirectories).Any();

    // ===== 字體替換（ReplaceFont.java） =====

    private static void ReplaceFont(string pathToIndex, string fontFolder, IProgress<PatchProgress> progress)
    {
        AppEnv.Log("[ReplaceFont] Loading Index File...");
        var index = new SqPackIndex(pathToIndex).ResolveIndex();

        using var indexFs = new FileStream(pathToIndex, FileMode.Open, FileAccess.ReadWrite);
        using var indexWriter = new BinaryWriter(indexFs);
        using var datFs = new FileStream(pathToIndex.Replace("index", "dat0"), FileMode.Open, FileAccess.ReadWrite);
        long datLength = datFs.Length;
        datFs.Seek(0, SeekOrigin.End);

        if (!Directory.Exists(fontFolder)) return;
        var files = Directory.GetFiles(fontFolder);
        int fileCount = 0;
        foreach (var file in files)
        {
            progress.Report(new(++fileCount / (double)files.Length, "正在替換字體：", Path.GetFileName(file)));
            AppEnv.Log("Replace : " + Path.GetFileName(file));
            var data = File.ReadAllBytes(file);
            byte[] block = file.EndsWith(".tex", StringComparison.OrdinalIgnoreCase)
                ? new TexBlockBuilder(data).BuildBlock()
                : new BinaryBlockBuilder(data).BuildBlock();
            int folderCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes("common/font"));
            int fileCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes(Path.GetFileName(file).ToLowerInvariant()));
            var indexEntry = index[folderCrc].Files[fileCrc];
            indexFs.Seek(indexEntry.Pt + 8, SeekOrigin.Begin);
            indexWriter.Write((int)(datLength / 8));
            datLength += block.Length;
            datFs.Write(block);
        }
    }

    // ===== 文本替換（ReplaceEXDF.java，CSV 模式） =====

    /// <summary>回傳成功摘要；一個檔案都沒替換到時視為失敗直接丟例外。</summary>
    private static string ReplaceExdf(string pathToIndex, IProgress<PatchProgress> progress)
    {
        int replaced = 0, noCsv = 0, failed = 0;
        // 讀回驗證用：記下第一筆與最後一筆寫入（offset 為 index 內儲存的原始值）
        (int Offset, byte[] Expected)? firstWrite = null, lastWrite = null;
        string slang = Config.Get("SLanguage") ?? "JA";
        var skipFiles = (Config.Get("SkipFiles") ?? "").ToLowerInvariant().Split('|').ToList();

        AppEnv.Log("[ReplaceEXDF] Initializing File List...");
        var fileList = InitFileList(pathToIndex);
        AppEnv.Log("[ReplaceEXDF] Loading Index File...");
        var indexSE = new SqPackIndex(pathToIndex).ResolveIndex();
        AppEnv.Log("[ReplaceEXDF] Loading Index Complete");

        using var indexFs = new FileStream(pathToIndex, FileMode.Open, FileAccess.ReadWrite);
        using var indexWriter = new BinaryWriter(indexFs);
        using var datFs = new FileStream(pathToIndex.Replace("index", "dat0"), FileMode.Open, FileAccess.ReadWrite);
        long datLength = datFs.Length;
        datFs.Seek(0, SeekOrigin.End);

        int fileCount = 0;
        foreach (var replaceFile in fileList)
        {
            progress.Report(new(++fileCount / (double)fileList.Count, "正在替換文本：", replaceFile));
            if (!replaceFile.ToUpperInvariant().EndsWith(".EXH")) continue;

            string replaceFileName = replaceFile[..replaceFile.LastIndexOf('.')].ToLowerInvariant();
            if (skipFiles.Contains(replaceFileName) ||
                skipFiles.Any(k => k.Length > 0 && replaceFileName.StartsWith(k + "/")))
            {
                AppEnv.Log(replaceFileName + " in skipFiles. Skip this part.");
                continue;
            }

            string filePath = replaceFile[..replaceFile.LastIndexOf('/')];
            string fileName = replaceFile[(replaceFile.LastIndexOf('/') + 1)..];
            int filePathCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes(filePath.ToLowerInvariant()));
            int exhFileCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes(fileName.ToLowerInvariant()));

            if (!indexSE.TryGetValue(filePathCrc, out var folder)) continue;
            if (!folder.Files.TryGetValue(exhFileCrc, out var exhIndexFile)) continue;

            byte[] exhData;
            try { exhData = ExtractFile(pathToIndex, exhIndexFile.DataOffset); }
            catch (Exception ex) { AppEnv.Log($"EXH extract failed: {replaceFile}: {ex.Message}"); failed++; continue; }
            var exhSE = new EXHFFile(exhData);
            if (exhSE.Langs.Length == 0) continue;

            // 載入對應 CSV
            string csvPath = AppEnv.P("resource", "rawexd",
                replaceFile[4..replaceFile.IndexOf('.')].Replace('/', Path.DirectorySeparatorChar) + ".csv");
            if (!File.Exists(csvPath))
            {
                AppEnv.Log("File not exists: " + csvPath);
                noCsv++;
                continue;
            }
            Dictionary<int, int> offsetMap;
            Dictionary<int, string[]> csvDataMap;
            try { (offsetMap, csvDataMap) = LoadCsv(csvPath); }
            catch (Exception ex) { AppEnv.Log("CSV Exception: " + ex.Message); failed++; continue; }

            bool wroteAnyPage = false;
            foreach (var page in exhSE.Pages)
            {
                int exdFileCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes(
                    fileName.Replace(".EXH", $"_{page.PageNum}_{slang}.EXD").ToLowerInvariant()));
                if (!folder.Files.TryGetValue(exdFileCrc, out var exdIndexFile)) continue;

                byte[] exdData;
                try { exdData = ExtractFile(pathToIndex, exdIndexFile.DataOffset); }
                catch { continue; }

                var exd = new EXDFFile(exdData);
                var entries = exd.Entries;

                foreach (var entryIndex in entries.Keys.ToList())
                {
                    var entryJA = new EXDFEntry(entries[entryIndex], exhSE.DatasetChunkSize);
                    if (entryJA.Data.Length == 0)
                    {
                        AppEnv.Log("Data size was insufficient, bypass handling.");
                        continue;
                    }
                    var chunk = new LERandomBytes(new byte[entryJA.Chunk.Length], bigEndian: true, increment: false);
                    chunk.Write(entryJA.Chunk);
                    var newFFXIVString = new MemoryStream();
                    foreach (var dataset in exhSE.Datasets)
                    {
                        if (dataset.Type != 0) continue; // 只處理字串欄位
                        byte[] jaBytes = entryJA.GetString(dataset.Offset);
                        // 更新 chunk 內的字串 offset 指標
                        chunk.Seek(dataset.Offset);
                        chunk.WriteInt((int)newFFXIVString.Length);
                        // 套用 CSV 翻譯；找不到就保留原文
                        string[]? rowStrings = csvDataMap.GetValueOrDefault(entryIndex);
                        if (rowStrings != null && offsetMap.TryGetValue(dataset.Offset, out int column)
                            && !string.IsNullOrEmpty(rowStrings[column]))
                            AppendCsvString(newFFXIVString, rowStrings[column]);
                        else
                            newFFXIVString.Write(jaBytes);
                        newFFXIVString.WriteByte(0);
                    }
                    // 打包整個 Entry，補到 4 bytes 對齊（剛好對齊時再補 4）
                    var chunkBytes = chunk.GetWork();
                    int bodyLength = chunkBytes.Length + (int)newFFXIVString.Length;
                    int paddingSize = 4 - bodyLength % 4;
                    if (paddingSize == 0) paddingSize = 4;
                    var entryBody = new byte[bodyLength + paddingSize];
                    chunkBytes.CopyTo(entryBody, 0);
                    newFFXIVString.ToArray().CopyTo(entryBody, chunkBytes.Length);
                    entries[entryIndex] = entryBody;
                }

                byte[] exdfFile = new EXDFBuilder(entries).BuildExdf();
                byte[] exdfBlock = new BinaryBlockBuilder(exdfFile).BuildBlock();
                int rawOffset = (int)(datLength / 8);
                indexFs.Seek(exdIndexFile.Pt + 8, SeekOrigin.Begin);
                indexWriter.Write(rawOffset);
                datLength += exdfBlock.Length;
                datFs.Write(exdfBlock);
                wroteAnyPage = true;
                firstWrite ??= (rawOffset, exdfFile);
                lastWrite = (rawOffset, exdfFile);
            }
            if (wroteAnyPage) replaced++;
        }
        string summary = $"漢化完畢：已替換 {replaced} 個資料表（無翻譯 CSV：{noCsv}，失敗：{failed}）";
        AppEnv.Log("Replace Completed. " + summary);
        if (replaced == 0)
            throw new InvalidOperationException(
                $"沒有替換任何文本（失敗：{failed}，無 CSV：{noCsv}），詳見 debug.log");

        // 讀回驗證：抽第一筆與最後一筆寫入，重新 extract 比對，確保資料真的落地
        datFs.Flush();
        foreach (var (offset, expected) in new[] { firstWrite!.Value, lastWrite!.Value }.DistinctBy(s => s.Offset))
        {
            byte[] readBack;
            try { readBack = ExtractFile(pathToIndex, offset); }
            catch (Exception ex)
            {
                throw new InvalidOperationException("漢化寫入驗證失敗（讀回時發生錯誤：" + ex.Message + "），請執行還原");
            }
            if (!readBack.AsSpan().SequenceEqual(expected))
                throw new InvalidOperationException("漢化寫入驗證失敗（讀回資料與寫入不符），請執行還原");
        }
        AppEnv.Log("Read-back verification passed.");
        return summary;
    }

    /// <summary>CSV 內容轉為 EXD 位元組：&lt;hex:...&gt; 標籤轉回 binary，其他照 UTF-8 寫入。</summary>
    private static void AppendCsvString(MemoryStream output, string readString)
    {
        var pending = new StringBuilder();
        bool isHexString = false;
        for (int i = 0; i < readString.Length; i++)
        {
            char c = readString[i];
            switch (c)
            {
                case '<':
                    if (i + 3 < readString.Length && readString[i + 1] == 'h'
                        && readString[i + 2] == 'e' && readString[i + 3] == 'x')
                    {
                        if (isHexString)
                            throw new Exception("TagInTagException!" + readString);
                        isHexString = true;
                    }
                    if (pending.Length > 0)
                    {
                        output.Write(Encoding.UTF8.GetBytes(pending.ToString()));
                        pending.Clear();
                    }
                    pending.Append(c);
                    break;
                case '>':
                    pending.Append(c);
                    if (isHexString)
                    {
                        // "<hex:XXXX>" → 去掉前 5 字元與最後的 '>'
                        output.Write(HexUtils.HexStringToBytes(pending.ToString(5, pending.Length - 6)));
                        pending.Clear();
                        isHexString = false;
                    }
                    break;
                default:
                    pending.Append(c);
                    break;
            }
        }
        if (pending.Length > 0)
            output.Write(Encoding.UTF8.GetBytes(pending.ToString()));
    }

    /// <summary>讀 SaintCoinach rawexd CSV。'#' 開頭是註解列（univocity 預設行為）；
    /// 有效列第 2 列（index 1）是 offset 列，資料從第 4 列（index 3）開始。</summary>
    private static (Dictionary<int, int> OffsetMap, Dictionary<int, string[]> DataMap) LoadCsv(string csvPath)
    {
        var rows = new List<string[]>();
        using (var parser = new TextFieldParser(csvPath, Encoding.UTF8))
        {
            parser.TextFieldType = FieldType.Delimited;
            parser.SetDelimiters(",");
            parser.HasFieldsEnclosedInQuotes = true;
            parser.TrimWhiteSpace = false;
            parser.CommentTokens = new[] { "#" };
            while (!parser.EndOfData)
                rows.Add(parser.ReadFields()!);
        }
        var offsetMap = new Dictionary<int, int>();
        for (int i = 1; i < rows[1].Length; i++)
            offsetMap[int.Parse(rows[1][i])] = i - 1;
        var dataMap = new Dictionary<int, string[]>();
        for (int i = 3; i < rows.Count; i++)
            dataMap[int.Parse(rows[i][0])] = rows[i][1..];
        return (offsetMap, dataMap);
    }

    internal static List<string> InitFileList(string pathToIndex)
    {
        var index = new SqPackIndex(pathToIndex).ResolveIndex();
        int filePathCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes("exd"));
        int rootFileCrc = FFCRC.ComputeCRC(Encoding.UTF8.GetBytes("root.exl"));
        var rootIndexFile = index[filePathCrc].Files[rootFileCrc];
        byte[] rootFile = ExtractFile(pathToIndex, rootIndexFile.DataOffset);
        var fileList = new List<string>();
        using var reader = new StreamReader(new MemoryStream(rootFile));
        string? line;
        while ((line = reader.ReadLine()) != null)
            fileList.Add("EXD/" + (line.Contains(',') ? line.Split(',')[0] : line) + ".EXH");
        return fileList;
    }

    internal static byte[] ExtractFile(string pathToIndex, long dataOffset)
    {
        int datNum = (int)((dataOffset & 0xF) / 2);
        dataOffset -= dataOffset & 0xF;
        string pathToOpen = pathToIndex.Replace("index2", "dat" + datNum).Replace("index", "dat" + datNum);
        using var datFile = new SqPackDatFile(pathToOpen);
        return datFile.ExtractFile(dataOffset * 8);
    }
}
