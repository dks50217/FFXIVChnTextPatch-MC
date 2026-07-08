using System.Windows;
using FFXIVChnTextPatch.Core;
using Microsoft.AspNetCore.Components.WebView.Wpf;
using Microsoft.Extensions.DependencyInjection;

namespace FFXIVChnTextPatch;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        var services = new ServiceCollection();
        services.AddWpfBlazorWebView();
        services.AddSingleton<PatchService>();
        // 資料夾選擇（WPF OpenFolderDialog，需在 UI 執行緒開啟）
        services.AddSingleton<Func<string?>>(() =>
        {
            string? selected = null;
            Application.Current.Dispatcher.Invoke(() =>
            {
                var dialog = new Microsoft.Win32.OpenFolderDialog { Title = "請選擇遊戲根目錄" };
                if (dialog.ShowDialog() == true)
                    selected = dialog.FolderName;
            });
            return selected;
        });

        blazorWebView.Services = services.BuildServiceProvider();
        blazorWebView.RootComponents.Add(new RootComponent
        {
            Selector = "#app",
            ComponentType = typeof(Main)
        });
    }
}
