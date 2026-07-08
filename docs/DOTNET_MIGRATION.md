# 遷移到 .NET WPF 計畫

## 目標

將現有 Java 8 + Swing 專案重寫為 C# + WPF (.NET 8)，保留所有業務邏輯，改善 UI 體驗。

---

## 技術對照

| Java 元件 | .NET 對應 | 備註 |
|---|---|---|
| Java Swing (`JFrame`/`JPanel`/`JButton`) | WPF (`Window`/`UserControl`/`Button`) | |
| `Thread` / `Runnable` | `Task` + `async/await` | 需改寫進度回報方式 |
| `java.util.Properties` 讀寫 | 手動解析 key=value 或用現成 parser | 格式相同，可直接相容 |
| `jzlib` (zlib inflate) | `System.IO.Compression.DeflateStream` | 內建，見下方說明 |
| `nlp-lang` (`JianFan` 簡繁轉換) | 自行實作 Dictionary 查表 | 見下方說明 |
| `univocity-parsers` (CSV 解析) | `CsvHelper` (NuGet) 或 `TextFieldParser` | |
| `LERandomAccessFile` / `LERandomBytes` | 自訂 `LEBinaryReader` / `LEBinaryWriter` | 封裝 `BinaryReader`/`BinaryWriter` |
| `java.util.logging.Logger` | `Microsoft.Extensions.Logging` 或 `Serilog` | |

---

## 專案結構

```
FFXIVChnTextPatch/
├── FFXIVChnTextPatch.csproj       (WPF, net8.0-windows)
├── App.xaml
├── MainWindow.xaml                ← TextPatchPanel 對應
├── ConfigWindow.xaml              ← ConfigApplicationPanel 對應
├── Models/
│   ├── SqPackIndex.cs
│   ├── SqPackDatFile.cs
│   ├── EXHFFile.cs
│   ├── EXDFFile.cs
│   ├── EXDFEntry.cs
│   ├── EXDFDataset.cs
│   ├── EXDFPage.cs
│   └── Language.cs
├── Core/
│   ├── ReplaceEXDF.cs
│   ├── ReplaceFont.cs
│   └── RollbackService.cs
├── Builders/
│   ├── BinaryBlockBuilder.cs
│   ├── EXDFBuilder.cs
│   └── TexBlockBuilder.cs
├── Utils/
│   ├── FFCRC.cs
│   ├── FFXIVString.cs
│   ├── LEBinaryReader.cs
│   ├── LEBinaryWriter.cs
│   ├── JianFan.cs
│   └── Config.cs
└── conf/
    └── global.properties          (直接沿用)
```

---

## 各層遷移說明

### 建議開發順序

1. 建立 .csproj + 空白視窗（確認環境）
2. Utils 層（純邏輯，最容易獨立測試）
3. Models 層
4. Builders 層
5. Core 層
6. WPF UI

---

### Utils 層

#### `FFCRC.cs`
對應：`FFCRC.java`

直接搬移，只有一個語法差異：Java 的 `>>>` 無符號右移在 C# 要改寫。

```java
// Java
dwCRC >>> 8
```
```csharp
// C# 寫法一：cast 成 uint
(int)((uint)dwCRC >> 8)

// C# 寫法二：從 .NET 7 起支援原生 >>> 運算子
dwCRC >>> 8
```

目標 .NET 8，可以直接使用 `>>>` 。

---

#### `LEBinaryReader.cs` / `LEBinaryWriter.cs`
對應：`LERandomAccessFile.java`、`LERandomBytes.java`

`BinaryReader`/`BinaryWriter` 在 x86/x64 上預設就是小端序，基本直接對應。

需要實作的方法：

| Java 方法 | C# 對應 |
|---|---|
| `readInt()` | `reader.ReadInt32()` |
| `readShort()` | `reader.ReadInt16()` |
| `readByte()` | `reader.ReadByte()` |
| `readFully(byte[])` | `reader.Read(buffer, 0, len)` |
| `seek(long)` | `stream.Seek(pos, SeekOrigin.Begin)` |
| `getFilePointer()` | `stream.Position` |
| `skipBytes(int)` | `stream.Seek(n, SeekOrigin.Current)` |
| `writeInt(int)` | `writer.Write(value)` |
| `length()` | `stream.Length` |

`LERandomBytes`（in-memory 版本）對應改用 `MemoryStream` + `BinaryReader`/`BinaryWriter`。

---

#### `JianFan.cs`
對應：`JianFan.java` + `nlp-lang` 函式庫

原專案用 nlp-lang 讀取 `resource/nlpcn/trad.txt` 做簡繁轉換。
.NET 沒有等效的現成 NuGet 套件，但實作很簡單：

```csharp
public class JianFan
{
    private readonly Dictionary<char, char> _map = new();

    public JianFan(string tradFilePath)
    {
        // trad.txt 格式：每行 "簡體字 繁體字"
        foreach (var line in File.ReadLines(tradFilePath, Encoding.UTF8))
        {
            if (line.Length >= 2)
                _map[line[0]] = line[1];
        }
    }

    public string ToTraditional(string input)
        => new string(input.Select(c => _map.TryGetValue(c, out var t) ? t : c).ToArray());
}
```

需要先確認 `resource/nlpcn/trad.txt` 的實際格式（每行幾個字、分隔符號）再調整。

---

#### `FFXIVString.cs`
對應：`FFXIVString.java`

純邏輯轉換。FFXIV 字串格式：
- `0x02` = 標籤開始
- 接著 1 byte type、1 byte(+) size、body bytes
- `0x03` = 標籤結束

`parseFFXIVString`：把 binary 轉成含 `<hex:XXXX>` 標記的字串。
`fstr2bytes`：把含 `<hex:XXXX>` 標記的字串轉回 binary。

這是整個系統最需要謹慎測試的部分，建議移植後用 Java 版本的 `main()` 裡的 hex 測資驗證輸出結果一致。

---

#### `Config.cs`
對應：`Config.java` + `ConfigResource.java`

`global.properties` 格式是標準的 `key=value`，可以簡單實作：

```csharp
public class Config
{
    private readonly Dictionary<string, string> _props = new();
    private string _filePath;

    public void Load(string path)
    {
        _filePath = path;
        foreach (var line in File.ReadLines(path))
        {
            if (line.StartsWith("#") || !line.Contains('=')) continue;
            var idx = line.IndexOf('=');
            _props[line[..idx].Trim()] = line[(idx + 1)..];
        }
    }

    public string? Get(string key) => _props.TryGetValue(key, out var v) ? v : null;
    public void Set(string key, string value) => _props[key] = value;

    public void Save()
    {
        var lines = _props.Select(kv => $"{kv.Key}={kv.Value}");
        File.WriteAllLines(_filePath, lines);
    }
}
```

注意：`global.properties` 裡的 Windows 路徑有 `\\:` 的跳脫（如 `D\:\\FF14\\...`），讀進來後需要 unescape（把 `\:` 換成 `:`）。

---

### Models 層

#### `SqPackIndex.cs`
對應：`SqPackIndex.java`

讀取 `.index` 檔案，解析 folder/file 的 CRC hash → offset 對照表。邏輯直接移植，注意：
- 所有整數欄位都是小端序（`LEBinaryReader` 處理）
- `resloveIndex()` → 建議改名 `ResolveIndex()`

#### `SqPackDatFile.cs`
對應：`SqPackDatFile.java`

最複雜的一個 model，負責從 `.dat0` 提取並解壓縮檔案。有三種 content type：

- **Type 2**（binary，如 EXD）：多個 deflate 壓縮 block 串接
- **Type 3**（model）：11 個 chunk 的 MDL 格式
- **Type 4**（texture）：含 mipmap 的 TEX 格式

**zlib 解壓的關鍵差異：**

Java 版在壓縮資料前手動補上 zlib header（`0x78 0x9C`）再用 jzlib inflate。
.NET 的 `DeflateStream` 做的是 raw deflate（不含 header），所以**不需要補 header**，直接對原始壓縮資料解壓即可：

```csharp
// Java 版（jzlib，需要加 header）：
gzipedData[0] = 0x78;
gzipedData[1] = 0x9C;
// ... 複製壓縮資料到 gzipedData[2..] ...
// 再用 Inflater 解壓

// C# 版（DeflateStream，不需要 header）：
using var compressed = new MemoryStream(compressedData);
using var deflate = new DeflateStream(compressed, CompressionMode.Decompress);
deflate.ReadExactly(decompressedBuffer, 0, decompressedSize);
```

同時，Java 版手動計算 Adler-32 checksum 並附在末尾，C# 版完全不需要這個步驟。

#### `EXHFFile.cs`
對應：`EXHFFile.java`

讀取 EXH 標頭檔，big-endian 格式（注意：EXH 是 big-endian，其他大多數是 little-endian）。
C# 的 `BinaryReader` 是 little-endian，需要用 `BinaryPrimitives.ReverseEndianness()` 或手動 swap 來讀 big-endian 的 `short`/`int`。

```csharp
// 讀 big-endian int16
short ReadBigEndianInt16(BinaryReader r)
{
    var bytes = r.ReadBytes(2);
    Array.Reverse(bytes);
    return BitConverter.ToInt16(bytes);
}
// 或用 BinaryPrimitives：
BinaryPrimitives.ReadInt16BigEndian(r.ReadBytes(2));
```

---

### Builders 層

#### `BinaryBlockBuilder.cs`
對應：`BinaryBlockBuilder.java`

把解壓的資料重新切成每塊最大 16000 bytes，分別用 deflate 壓縮後組成 SqPack block。

**壓縮的關鍵差異：**
Java 用 jzlib 的 `Deflater` 壓縮（預設 level）。
C# 用 `DeflateStream` 搭配 `CompressionLevel.Optimal`，輸出是 raw deflate（不含 header/checksum），可直接使用。

```csharp
byte[] Compress(byte[] data)
{
    using var output = new MemoryStream();
    using (var deflate = new DeflateStream(output, CompressionLevel.Optimal))
        deflate.Write(data);
    return output.ToArray();
}
```

#### `EXDFBuilder.cs`
對應：`EXDFBuilder.java`

從修改後的 row 資料重建 EXD 二進位格式。純邏輯，直接移植。

#### `TexBlockBuilder.cs`
對應：`TexBlockBuilder.java`

處理 TEX 格式的 content type 4 block 重建。直接移植。

---

### Core 層

#### `ReplaceFont.cs`
對應：`ReplaceFont.java`

從 `resource/font/` 讀取 `.fdt`/`.tex` 檔，用 `BinaryBlockBuilder`/`TexBlockBuilder` 打包後寫入 `.dat0`，並更新 `.index` 的 offset。邏輯直接移植，相對單純。

#### `ReplaceEXDF.cs`
對應：`ReplaceEXDF.java`

最複雜的核心邏輯，流程：

1. 讀取 `root.exl` 取得所有 EXH 檔案清單
2. 對每個 EXH 逐一處理：
   a. 用 FFCRC 計算路徑 hash → 查 index 取得 offset → 解壓取得 EXH
   b. 對 EXH 中的每個 page，取得對應 EXD
   c. 對 EXD 中每個 row 的每個 string dataset，套用翻譯（CSV 來源或 CN 檔案來源）
   d. 把 FFXIV 特殊標籤（`<hex:...>`）轉回 binary
   e. 重建 EXD block，寫入 `.dat0`，更新 `.index` offset

**進度回報改寫：**
Java 版直接呼叫 `percentPanel.percentShow()`（Swing 沒有 thread 限制）。
C# 版背景執行，需用 `IProgress<T>` 傳遞進度：

```csharp
// 定義進度資料結構
record PatchProgress(double Percent, string Message);

// Core 方法簽名
async Task ReplaceAsync(IProgress<PatchProgress> progress, CancellationToken ct)

// 回報進度
progress.Report(new PatchProgress(fileCount / (double)total, $"正在替換：{fileName}"));
```

#### `RollbackService.cs`
對應：`RollbackThread.java`

還原備份檔案。邏輯直接移植，改為 `async Task`。

---

### WPF UI 層

#### `MainWindow.xaml`
對應：`TextPatchPanel.java`

```xml
<Window ...>
    <Grid>
        <!-- 漢化 / 還原 按鈕 -->
        <!-- 進度條 ProgressBar -->
        <!-- 狀態文字 TextBlock -->
    </Grid>
</Window>
```

Code-behind 或 ViewModel（MVVM）：
- 按下「漢化」→ `await replaceService.RunAsync(progress, ct)`
- `IProgress<PatchProgress>` callback 更新 `ProgressBar.Value` 和 `TextBlock.Text`
- 執行中 disable 按鈕，完成後 enable

#### `ConfigWindow.xaml`
對應：`ConfigApplicationPanel.java`

表單欄位：
- 遊戲路徑（`TextBox` + 瀏覽按鈕，用 `FolderBrowserDialog`）
- 原始語言 `SLanguage`（`ComboBox`：JA / EN / DE / FR）
- 檔案語言 `FLanguage`（`ComboBox`：CSV / CHS / CHT）
- 取代字體 `ReplaFont`（`CheckBox`）
- 取代文字 `ReplaText`（`CheckBox`）
- 跳過檔案 `SkipFiles`（`TextBox`）
- 確定按鈕 → 存回 `global.properties`

---

## 需要特別測試的部分

1. **FFCRC 輸出**：用 Java 版對同一組輸入算出的值驗證 C# 版是否一致。
2. **SqPack 解壓縮**：解出的 EXH/EXD byte 陣列和 Java 版完全相同。
3. **FFXIVString 解析/重建**：`parseFFXIVString` ↔ `fstr2bytes` 來回轉換結果一致（可用 `FFXIVString.java` 的 `main()` 裡的 hex 測資）。
4. **Block 重建後的壓縮格式**：寫入 `.dat0` 的 block header 格式正確，遊戲能讀取。
5. **`global.properties` 路徑 unescape**：Windows 路徑中的 `\:` 正確轉回 `:`。

---

## 不需要遷移的部分

- `org.eclipse.jdt.internal.jarinjarloader/` — 這是 Eclipse 的 JAR-in-JAR loader，.NET 不需要。
- `TransMode` 相關的 teemo.name 遠端下載邏輯 — 該 server 已不可用，且已被 CSV 模式取代，可以直接移除。
- `EXDFUtil.java` 中的 `exQuestMap`/`transMap` 邏輯 — 僅在非 CSV 模式（CN 檔案模式）下使用，可視需求決定是否移植。
