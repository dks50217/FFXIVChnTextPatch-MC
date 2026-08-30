using System.Text;
using FFXIVChnTextPatch.Core;

namespace FFXIVChnTextPatch;

/// <summary>`FFXIVChnTextPatch.exe --selftest`：核心二進位邏輯的最小驗證，結果寫到 selftest.log。
/// 回傳失敗數，App 用它當 process exit code，CI 才能判定成敗。</summary>
public static class SelfTest
{
    public static int Run()
    {
        var log = new StringBuilder();
        int failed = 0;

        void Check(string name, bool ok)
        {
            log.AppendLine($"{(ok ? "PASS" : "FAIL")}  {name}");
            if (!ok) failed++;
        }

        // 1. FFCRC 與 Java 版輸出一致（比對值由原版 FFCRC.java 產生）
        var crcVectors = new (string Input, int Expected)[]
        {
            ("common/font", 1713442675),
            ("axis_12.fdt", -1447579230),
            ("exd", -476350055),
            ("root.exl", 1370848956),
            ("exd/item_0_ja.exd", -1235084413),
            ("exd/quest/000/clshrv001_00003.exh", 434756717),
        };
        foreach (var (input, expected) in crcVectors)
            Check($"FFCRC(\"{input}\") == {expected}",
                FFCRC.ComputeCRC(Encoding.UTF8.GetBytes(input)) == expected);

        // 2. zlib raw deflate round-trip
        var rnd = new Random(42);
        var data = new byte[40000];
        rnd.NextBytes(data);
        for (int i = 0; i < 20000; i++) data[i] = (byte)(i % 7); // 一半可壓縮
        var decompressed = SqPackZlib.Decompress(SqPackZlib.Compress(data), data.Length);
        Check("SqPackZlib round-trip", decompressed.AsSpan().SequenceEqual(data));

        // 3. EXDFBuilder → EXDFFile round-trip
        var entries = new Dictionary<int, byte[]>
        {
            [5] = Encoding.UTF8.GetBytes("hello\0pad."),
            [1] = new byte[] { 0, 0, 0, 4, 1, 2, 3, 4, 0, 0, 0, 0 },
            [42] = new byte[] { 9, 9, 9, 9 },
        };
        var exdBytes = new EXDFBuilder(entries).BuildExdf();
        var parsed = new EXDFFile(exdBytes).Entries;
        Check("EXDF build/parse round-trip",
            parsed.Count == entries.Count &&
            entries.All(kv => parsed.TryGetValue(kv.Key, out var v) && v.AsSpan().SequenceEqual(kv.Value)));

        // 4. BinaryBlockBuilder → SqPackDatFile round-trip（多區塊，>16000 bytes）
        var block = new BinaryBlockBuilder(data).BuildBlock();
        var tmp = Path.Combine(Path.GetTempPath(), "ffxivpatch-selftest.dat");
        File.WriteAllBytes(tmp, block);
        try
        {
            using (var dat = new SqPackDatFile(tmp))
            {
                var extracted = dat.ExtractFile(0);
                Check("BinaryBlock build/extract round-trip", extracted.AsSpan().SequenceEqual(data));
                Check("Block 128-byte alignment", block.Length % 128 == 0);
            }

            // PatchService 漢化中會以 ReadWrite 開著 dat 檔，ExtractFile 必須仍能讀（sharing violation 回歸測試）
            bool sharingOk;
            try
            {
                using var writeHandle = new FileStream(tmp, FileMode.Open, FileAccess.ReadWrite);
                using var dat = new SqPackDatFile(tmp);
                sharingOk = dat.ExtractFile(0).AsSpan().SequenceEqual(data);
            }
            catch (IOException) { sharingOk = false; }
            Check("ExtractFile with concurrent ReadWrite handle", sharingOk);
        }
        finally
        {
            File.Delete(tmp);
        }

        // 5. Config properties 跳脫 round-trip
        Check("Config path unescape",
            TestConfigUnescape(@"D\:\\FF14\\SquareEnix\\FINAL FANTASY XIV - A Realm Reborn",
                @"D:\FF14\SquareEnix\FINAL FANTASY XIV - A Realm Reborn"));

        // 6. exd-names.csv 載入與各種表名形式的查詢
        Check("ExdNames lookup (Item)", ExdNames.Describe("Item") == "道具");
        Check("ExdNames lookup (EXD/Item.EXH)", ExdNames.Describe("EXD/Item.EXH") == "道具");
        Check("ExdNames folder fallback (quest/000/x)", ExdNames.Describe("quest/000/ClsArc011_00021") == "任務對話");
        Check("ExdNames folder/sheet name collision (Quest vs quest/)", ExdNames.Describe("Quest") == "任務");
        Check("ExdNames unknown passthrough", ExdNames.Label("NoSuchSheet") == "NoSuchSheet");

        // 7. RawexdMerge 逐格合併規則
        string mLo = "key,0,1\n#,Name,Desc\noffset,0,4\nint32,str,str\n0,已翻,\n1,,\n3,本地獨有,x\n";
        string mUp = "key,0,1\n#,Name,Desc\noffset,0,4\nint32,str,str\n0,上游改進,上游補\n1,新翻,\n2,新列,y\n";
        var mr = RawexdMerge.Merge(mLo, mUp, "\n");
        Check("Merge 本地非空保留 + 空格補上游", mr.Merged.Contains("0,已翻,上游補") && mr.Merged.Contains("1,新翻,"));
        Check("Merge 上游新列", mr.Merged.Contains("2,新列,y") && mr.NewRows == 1);
        Check("Merge 本地獨有列附加檔尾", mr.Merged.TrimEnd().EndsWith("3,本地獨有,x"));
        Check("Merge 補格計數", mr is { Filled: 2, HeadersChanged: false });
        Check("Merge 註解列原樣保留", mr.Merged.Contains("#,Name,Desc"));

        string mQuoted = "key,0\n#,Name\noffset,0\nint32,str\n0,\"a,\"\"b\"\"\n換行\"\n";
        var mrQ = RawexdMerge.Merge(mQuoted, mQuoted, "\n");
        Check("Merge 引號欄位 round-trip",
            RawexdMerge.Parse(mrQ.Merged).Last(x => x.Fields != null).Fields![1] == "a,\"b\"\n換行");

        var mrAlign = RawexdMerge.Merge(
            "key,0,1\n#,A,B\noffset,0,4\nint32,str,str\n0,甲,乙\n",
            "key,0,1,2\n#,A,New,B\noffset,0,2,4\nint32,str,str,str\n0,x,新欄,y\n", "\n");
        Check("Merge 跨版本 offset 欄位對齊", mrAlign.Merged.Contains("0,甲,新欄,乙") && mrAlign.HeadersChanged);

        // 7b. 漂移偵測：上游該 key 全空、本地有翻譯、且該翻譯在上游別處出現 → 疑似錯位。
        //     排除「本地翻在上游前面」（fork 比上游完整、值不在上游）與本地獨有列。
        string dUp = "key,0\n#,Name\noffset,0\nint32,str\n0,\n1,foo\n2,\n";
        string dLo = "key,0\n#,Name\noffset,0\nint32,str\n0,baz\n1,foo\n2,foo\n9,本地獨有\n";
        var drift = LintTool.DetectDrift(dLo, dUp);
        //  key2=foo：上游同 key 空、foo 在上游 key1 有 → 錯位 ✓
        //  key0=baz：上游同 key 空，但 baz 不在上游任何處 → 翻在前面，不算 ✓
        //  key9    ：上游沒這個 key → 本地獨有列，不算 ✓
        Check("DetectDrift 內容搬到別處才算錯位", drift.SequenceEqual(new[] { 2 }));

        // 8. ZhConvert 簡轉繁（單字、詞級消歧義、台灣異體字，各走到不同字典）
        Check("ZhConvert 單字", ZhConvert.S2Tw("汉化") == "漢化");
        Check("ZhConvert 詞級消歧義 (头发)", ZhConvert.S2Tw("头发") == "頭髮");
        Check("ZhConvert 台灣異體字 (麪→麵)", ZhConvert.S2Tw("麪") == "麵");
        Check("ZhConvert 台灣用語 (服务器→伺服器)", ZhConvert.S2Tw("服务器") == "伺服器");
        Check("ZhConvert 台灣用語一對多取第一 (菜单→選單)", ZhConvert.S2Tw("菜单") == "選單");
        Check("ZhConvert GP 詞彙表 (激活→啟動)", ZhConvert.S2Tw("激活") == "啟動");
        Check("ZhConvert GP 詞彙表 (几率→機率)", ZhConvert.S2Tw("几率") == "機率");
        Check("ZhConvert GP 引號規則 (‘→『)", ZhConvert.S2Tw("‘") == "『");
        Check("ZhConvert GP 英文名保護 (L’Heritier 不變)", ZhConvert.S2Tw("L’Heritier") == "L’Heritier");
        Check("ZhConvert ASCII/CSV 結構字元不動", ZhConvert.S2Tw("0,\"a\",汉") == "0,\"a\",漢");

        log.AppendLine(failed == 0 ? "ALL PASSED" : $"{failed} FAILED");
        File.WriteAllText(Path.Combine(AppEnv.BaseDir, "selftest.log"), log.ToString());
        return failed;
    }

    private static bool TestConfigUnescape(string escaped, string expected)
    {
        var tmp = Path.Combine(Path.GetTempPath(), "ffxivpatch-selftest.properties");
        File.WriteAllText(tmp, "#comment\nGamePath=" + escaped + "\n");
        try
        {
            Config.Load(tmp);
            return Config.Get("GamePath") == expected;
        }
        finally
        {
            File.Delete(tmp);
            Config.Load(AppEnv.P("conf", "global.properties")); // 還原正式設定
        }
    }
}
