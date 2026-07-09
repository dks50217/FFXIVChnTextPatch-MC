using System.Text;
using FFXIVChnTextPatch.Core;

namespace FFXIVChnTextPatch;

/// <summary>`FFXIVChnTextPatch.exe --selftest`：核心二進位邏輯的最小驗證，結果寫到 selftest.log。</summary>
public static class SelfTest
{
    public static void Run()
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

        log.AppendLine(failed == 0 ? "ALL PASSED" : $"{failed} FAILED");
        File.WriteAllText(Path.Combine(AppEnv.BaseDir, "selftest.log"), log.ToString());
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
