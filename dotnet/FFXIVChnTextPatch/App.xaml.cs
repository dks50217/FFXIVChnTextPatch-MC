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
            SelfTest.Run();
            Shutdown();
            return;
        }

        if (e.Args.Contains("--lint"))
        {
            LintTool.Run();
            Shutdown();
            return;
        }

        base.OnStartup(e);
        new MainWindow().Show();
    }
}
