# FFXIVChnTextPatch (.NET WPF Blazor Hybrid)

Java Swing 版的 C# 移植：WPF 視窗內嵌 `BlazorWebView`，UI 用 Razor 元件，核心二進位邏輯（SqPack/EXD/CRC/區塊重建）從 Java 逐一移植。

## 需求

- .NET 10 SDK（建置）／.NET 10 Desktop Runtime（執行）
- Microsoft Edge WebView2 Runtime（Win10/11 通常已內建）

## 建置與執行

```bash
cd dotnet/FFXIVChnTextPatch
dotnet build
dotnet run          # 或直接執行 bin/Debug/net10.0-windows10.0.17763.0/FFXIVChnTextPatch.exe
```

程式會從執行檔位置向上尋找 `conf/global.properties` 來決定基準目錄（`conf/`、`resource/`、`backup/`、`debug.log` 都相對於它），所以放在本 repo 內任何深度執行都可以。

## 驗證

```bash
FFXIVChnTextPatch.exe --selftest   # 結果寫到 repo 根目錄 selftest.log
```

包含：FFCRC 與 Java 原版輸出比對（6 組向量）、deflate round-trip、EXDF 建置/解析 round-trip、SqPack 區塊建置/解壓 round-trip、properties 路徑跳脫。

## 與 Java 版的差異

- **僅支援 CSV 翻譯模式**（`FLanguage=CSV`，讀 `resource/rawexd/*.csv`）。舊的「CN 客戶端檔案」模式（`EXDFUtil`／`JianFan` 簡繁轉換／`transtable`／teemo.name 遠端下載）未移植，需要時再從 Java 版補。
- SqPack 解壓只實作 content type 2（漢化流程只會解 EXH/EXD/root.exl）；type 3/4 解壓未移植（字體是「寫入」type 4，這部分有移植）。
- zlib：Java 版用 jzlib 手動補 zlib header + Adler-32；.NET 的 `DeflateStream` 是 raw deflate，直接處理，行為等價。
- CSV 解析改用內建 `TextFieldParser`（`#` 開頭視為註解列，對齊 univocity 預設行為）。
