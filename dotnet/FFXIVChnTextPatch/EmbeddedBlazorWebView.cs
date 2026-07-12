using Microsoft.AspNetCore.Components.WebView.Wpf;
using Microsoft.Extensions.FileProviders;

namespace FFXIVChnTextPatch;

/// <summary>
/// 把整個 wwwroot（index.html、css/app.css，以及 _framework/blazor.webview.js）從內嵌資源讀，
/// 讓單檔 exe 搬到任何資料夾都能單獨執行、不必在旁邊帶 wwwroot。
/// ponytail: wwwroot/_framework/blazor.webview.js 是從 WebView 套件輸出複製進來、版本綁定的檔；
/// 升級 Microsoft.AspNetCore.Components.WebView.Wpf 後要重新複製一份（否則新版 JS 對不上）。
/// </summary>
public sealed class EmbeddedBlazorWebView : BlazorWebView
{
    public override IFileProvider CreateFileProvider(string contentRootDir) =>
        new ManifestEmbeddedFileProvider(GetType().Assembly, "wwwroot");
}
