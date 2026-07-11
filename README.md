# FFXIV Translation Patch Tool

FFXIV 國際服的中文漢化器。以 C#/.NET 10（WPF + Blazor Hybrid）重寫，程式碼在 [`dotnet/`](dotnet/README.md)。

相較於上游原版：
1. 針對 5.5X 以後版本修正中文字庫補丁。
2. 使用 CSV（修改過的 SaintCoinach 輸出）進行漢化，**僅支援 CSV 模式**（中國服檔案 / 漢化覆蓋檔模式已移除）。
3. 刪除原版 exe 中與 teemo 連線的部分。
4. 以 C#/.NET 重寫，含 `--selftest` 二進位格式自檢與翻譯 CSV 檢查工具。

## 授權與溯源

本專案以 [GNU GPLv3](LICENSE) 授權。.NET 版移植自 [GpointChen/FFXIVChnTextPatch-GP](https://github.com/GpointChen/FFXIVChnTextPatch-GP)（Java Swing 版，其前身為 yumao 的 FFXIVChnTextPatch，2019-09-01 開源），為其衍生著作，沿用 GPLv3。原 Java 原始碼已自工作目錄移除，可在本 repo 的 git 歷史（`dotnet10-upgrade` 分支之前的 `src/`）或上游專案取得。

## 使用

從本專案的 [Releases](https://github.com/dks50217/FFXIVChnTextPatch-MC/releases) 下載，或自行編譯。需要 Windows + WebView2 Runtime。

1. 開啟 `FFXIVChnTextPatch.exe`，首次啟動會進入「漢化設置」
2. 「遊戲路徑」：選擇 FFXIV 遊戲根目錄（目錄內須有 `game/ffxiv_dx11.exe`，預設名為 `FINAL FANTASY XIV ONLINE`）
3. 「原始語言」：想要覆蓋遊戲中的哪種語言（建議日文，覆蓋其他語言不保證沒問題）
4. 視需求勾選「替換字體」「替換文本」，點「確認」
5. 回到主畫面點「漢化」；「還原」可隨時回復備份，不需任何設定

漢化前會自動備份六個 index/dat 檔到 `backup/`。注意事項：

- 為避免遊戲更新時出問題，建議每次更新前先「還原」，更新完成後再重新漢化。
- 程式會拒絕在已漢化的檔案上重複漢化（避免備份被已漢化的檔案覆蓋導致無法還原）。
- 主畫面的「檢查翻譯 CSV」可檢查 `resource/rawexd` 翻譯檔的格式與覆蓋率。

## 編譯

需要 .NET 10 SDK（Windows），詳見 [`dotnet/README.md`](dotnet/README.md)。

```bash
cd dotnet/FFXIVChnTextPatch
dotnet build
dotnet run
```

發佈單一執行檔：

```bash
dotnet publish -c Release -r win-x64 --self-contained -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true
```

產出的 `FFXIVChnTextPatch.exe` 與 `wwwroot/` 須連同 `conf/`、`resource/` 一起發佈。

舊 Java 版的編譯筆記可參考[這裡](https://hackmd.io/@GpointChen/SJi_gv-ad)（原始碼在 git 歷史中）。

## 翻譯資源

- `resource/rawexd/` — CSV 翻譯檔（每個 EXD 表一個檔案）。各 CSV 對應遊戲內哪些文本，可參考 [Souma 版的 CSV 文件說明](https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma/wiki/CSV%E6%96%87%E4%BB%B6)。
- `resource/font/` — 替換字體（`.fdt` + `.tex`）。
- 設置頁的「跳過的資料表」可勾選漢化時要跳過的表（含中文說明、可搜尋），也可直接編輯 `conf/global.properties` 的 `SkipFiles`（`|` 分隔，格式如 `exd/quest`）。
- `conf/exd-names.csv` — 表名對應遊戲內文本位置的說明檔，用於設置頁清單、漢化進度與檢查報告。

## 免責聲明（沿自原項目）

- 本程式以修改客戶端的方式載入中文資源，此舉違反官方規則，使用即表示自行承擔一切後果。
- 本專案僅供學習與技術交流使用，嚴禁任何商業用途。
