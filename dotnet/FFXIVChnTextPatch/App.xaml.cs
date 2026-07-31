using System.Windows;
using FFXIVChnTextPatch.Core;

namespace FFXIVChnTextPatch;

public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        Config.Load(AppEnv.P("conf", "global.properties"));
        AppEnv.Log(".NET Version: " + Environment.Version);

        if (e.Args.Contains("--selftest"))
        {
            // exit code = 失敗數，CI 以 Start-Process -Wait -PassThru 讀 ExitCode 判定。
            Shutdown(SelfTest.Run());
            return;
        }

        if (e.Args.Contains("--lint"))
        {
            // exit code = 會中斷漢化的錯誤數（0 = 乾淨）。
            Shutdown(LintTool.Run().ErrorCount);
            return;
        }

        if (e.Args.Contains("--hextags"))
        {
            HexTagTool.Run();
            Shutdown();
            return;
        }

        if (e.Args.Contains("--update"))
        {
            // CLI 版一鍵更新，階段進度與結果寫到 debug.log。
            // 不能用 Progress<T>：它會把 callback 排回被 GetResult() 卡住的 UI 執行緒。
            string lastAction = "";
            var progress = new DirectProgress(p =>
            {
                if (p.Action == lastAction) return;
                lastAction = p.Action;
                AppEnv.Log(p.Action);
            });
            var (_, message) = RawexdUpdater.UpdateAsync(progress).GetAwaiter().GetResult();
            AppEnv.Log(message);
            Shutdown();
            return;
        }

        base.OnStartup(e);
        new MainWindow().Show();
    }

    private sealed class DirectProgress(Action<PatchProgress> handler) : IProgress<PatchProgress>
    {
        public void Report(PatchProgress value) => handler(value);
    }
}
