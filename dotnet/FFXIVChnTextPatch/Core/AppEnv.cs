namespace FFXIVChnTextPatch.Core;

/// <summary>基準目錄（conf/resource/backup 所在處）與 debug.log。從執行檔位置向上找 conf/global.properties，找不到就用目前目錄。</summary>
public static class AppEnv
{
    public static string BaseDir { get; } = FindBaseDir();

    private static string FindBaseDir()
    {
        foreach (var start in new[] { AppContext.BaseDirectory, Environment.CurrentDirectory })
        {
            for (var d = new DirectoryInfo(start); d != null; d = d.Parent)
                if (File.Exists(Path.Combine(d.FullName, "conf", "global.properties")))
                    return d.FullName;
        }
        return Environment.CurrentDirectory;
    }

    public static string P(params string[] parts) =>
        Path.Combine(new[] { BaseDir }.Concat(parts).ToArray());

    private static readonly object LogLock = new();

    public static void Log(string message)
    {
        try
        {
            lock (LogLock)
                File.AppendAllText(P("debug.log"), $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {message}{Environment.NewLine}");
        }
        catch
        {
            // ponytail: 記錄失敗不影響漢化流程
        }
    }
}
