# FFXIV Translation Patch Tool

FFXIV 國際服的中文漢化器。以 Rust 重寫，程式碼在 [`rust/`](#編譯)：操作畫面用 egui，另有給腳本與 CI 用的命令列版。

舊的 C#/.NET 10（WPF + Blazor Hybrid）版保留在 [`csharp/`](csharp/README.md)，不再發行。兩版讀寫同一份 `conf/`、`resource/`、`backup/`，可以混用。

![程式畫面](docs/app1.png)

相較於上游原版：
1. 針對 5.5X 以後版本修正中文字庫補丁。
2. 使用 CSV（修改過的 SaintCoinach 輸出）進行漢化，**僅支援 CSV 模式**（中國服檔案 / 漢化覆蓋檔模式已移除）。
3. 刪除原版 exe 中與 teemo 連線的部分。
4. 以 Rust 重寫，含 `--selftest` 自檢、翻譯 CSV 檢查與控制碼參照檢查工具；漢化速度約為 C# 版的 4 倍。

## 授權與溯源

本專案以 [GNU GPLv3](LICENSE) 授權。程式移植自 [GpointChen/FFXIVChnTextPatch-GP](https://github.com/GpointChen/FFXIVChnTextPatch-GP)（Java Swing 版，其前身為 yumao 的 FFXIVChnTextPatch，2019-09-01 開源），為其衍生著作，沿用 GPLv3。Java 版先移植成 C#，再移植成 Rust；原 Java 原始碼已自工作目錄移除，可在本 repo 的 git 歷史（`dotnet10-upgrade` 分支之前的 `src/`）或上游專案取得。

- `resource/rawexd/` 翻譯資料部分合併自 [Souma-Sumire/FFXIVChnTextPatch-Souma](https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma)（GPL-3.0，簡體），**並非直接沿用其 CSV**：合併時經簡轉繁與台灣用語轉換（含 FFXIV 專屬詞彙校訂），並持續由本專案人工編輯、修訂為適合台灣玩家的漢化用字；既有的本地翻譯在更新時一律保留。
- `resource/opencc/` 字典檔取自 [OpenCC](https://github.com/BYVoid/OpenCC)（Apache-2.0）；其中 `GPPhrases.txt` 是 GP 版的 FFXIV 簡繁例外詞彙表，轉自本 repo git 歷史中的 `resource/nlpcn/traditional.txt`（GPLv3）。

## 發佈檔案

每個 [Release](https://github.com/dks50217/FFXIVChnTextPatch-MC/releases) 會有兩個檔案：

| 檔案 | 內容 | 給誰用 |
|------|------|--------|
| `FFXIVChnTextPatch.exe` | 單一執行檔（約 8MB，不需安裝任何 runtime），自己漢化用 | 想自己備份、隨時還原、之後用「更新翻譯 CSV」的人 |
| `YYYYMMDDXX_CHT.zip` | **已漢化完成的六個 index/dat 檔** | 只想直接玩、不想跑漢化流程的人 |

翻譯檔另外放在固定的 [`rawexd-latest`](https://github.com/dks50217/FFXIVChnTextPatch-MC/releases/tag/rawexd-latest) Release（`rawexd-opencc.zip`，內含 `resource/rawexd` 的 CSV 與 `resource/opencc` 字典），master 的翻譯有更新時由 CI 自動重新打包，不需要跟著發版。

兩種使用方式，擇一即可：

### A. 用 exe 自己漢化（可還原、可更新翻譯）

`FFXIVChnTextPatch.exe` 下載後直接執行（Windows 10/11）。首次執行若偵測不到翻譯 CSV，會跳出提示，一鍵下載 `rawexd-opencc.zip` 並解壓到 `resource/`（不需 git）。

> 字體檔（`resource/font`，約 248MB）太大不含在自動下載內。要「替換字體」請自行到本 repo 下載字體檔放進 `resource/font`；設置頁勾了替換字體但缺檔時會有提示。

### B. 直接套用漢化好的檔案（最快、不能用工具還原）

下載 `YYYYMMDDXX_CHT.zip`（`YYYYMMDDXX` 是遊戲版本號，`_CHT` 表繁中），解壓後把裡面的六個檔案覆蓋到遊戲目錄：

```
<遊戲根目錄>\game\sqpack\ffxiv\
```

覆蓋前建議自行備份那六個檔。這種方式沒有經過工具備份，**不能用程式的「還原」還原**；遊戲改版後請用官方啟動器的「檔案修復」取回原版檔案，或改用方式 A。

## 使用（方式 A 詳細步驟）

1. 開啟 `FFXIVChnTextPatch.exe`，首次啟動會進入「漢化設置」
2. 「遊戲路徑」：按「瀏覽…」選擇 FFXIV 遊戲根目錄（目錄內須有 `game/ffxiv_dx11.exe`，預設名為 `FINAL FANTASY XIV ONLINE`）
3. 「原始語言」：想要覆蓋遊戲中的哪種語言（建議日文，覆蓋其他語言不保證沒問題）
4. 視需求勾選「替換字體」「替換文本」，點「確認」
5. 回到主畫面點「漢化」；「還原」可隨時回復備份，不需任何設定

漢化前會自動備份六個 index/dat 檔到 `backup/`。注意事項：

- 為避免遊戲更新時出問題，建議每次更新前先「還原」，更新完成後再重新漢化。
- 程式會拒絕在已漢化的檔案上重複漢化（避免備份被已漢化的檔案覆蓋導致無法還原）。判斷方式：Addon 表的中文譯文過半已經在遊戲檔裡，或遊戲裡的 `.fdt` 字型跟 `resource/font` 的一樣，就視為已漢化。不管這次勾了哪些項目都會檢查兩者，因為漢化前的備份一律會複製全部六個檔。遊戲更新不一定會換掉漢化過的檔案，所以不能只看版本號。被拒絕時請先「還原」；如果遊戲更新過、還原也被拒絕，請用官方啟動器的「檔案修復」取回原版檔案後再漢化。
- 主畫面的「檢查翻譯 CSV」可檢查 `resource/rawexd` 翻譯檔的格式與覆蓋率，報告寫到 `lint-report.txt`。
- 主畫面的「更新翻譯 CSV」一鍵從 [Souma 上游](https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma) 下載最新翻譯（需要 git）、簡轉繁（含台灣用語，等同 OpenCC s2twp）後逐儲存格合併：**本地已有的翻譯永遠不會被覆蓋**，只補空格、新列與新檔。合併前會先備份到 `backup/rawexd-before-update.zip`。
- 「hex 標籤對照表」把翻譯 CSV 裡的 `<hex:>` 標籤解成可讀名稱，寫到 `hextags-report.txt`，給翻譯者辨認哪些標籤不能動。
- 用語轉換的例外與自訂譯法寫在 `resource/opencc/UserPhrases.txt`（格式見檔內說明，優先權最高），影響之後每次「更新翻譯 CSV」新補進來的文字。
- 執行過程與錯誤記錄在程式旁的 `debug.log`，回報問題時請附上。

## 編譯

需要 Rust stable（用 `rustup` 安裝）：

```bash
cd rust
cargo build --release
```

會產出兩個執行檔，都在 `rust\target\release\`：

- **`FFXIVChnTextPatch.exe`：操作畫面**，雙擊就能開，也就是發佈的那個 exe。只支援 Windows；中文字型用 Windows 內建的微軟正黑體。
- **`ffxiv_chn_text_patch.exe`：命令列**，給腳本和 CI 用。雙擊只會印出用法就關掉，請在 PowerShell 或命令提示字元裡加上指令執行：

```powershell
.\rust\target\release\ffxiv_chn_text_patch.exe --selftest
```

兩個都是單一 exe，不需要另外安裝任何 runtime：C runtime 已靜態連結（設定在 `rust/.cargo/config.toml`），只依賴 Windows 內建的 DLL。

程式從執行檔位置往上找 `conf/global.properties` 來決定基準目錄（`conf/`、`resource/`、`backup/`、`debug.log` 都相對於它），找不到才用目前目錄。所以 exe 留在 repo 裡、或搬到任何上層有 `conf/` 的資料夾都能用。命令列沒有設定功能，`--patch` 用的設定請先在操作畫面的「設置」設好，或直接編輯 `conf/global.properties`。

| 指令 | 作用 | 會改到的檔案 |
|------|------|------|
| `--patch` | 漢化（遊戲必須關閉） | 遊戲的六個 index/dat 檔；改之前先備份到 `backup/` |
| `--rollback` | 從 `backup/` 還原 | 遊戲的六個 index/dat 檔 |
| `--update` | 一鍵更新翻譯 CSV（需要 git） | `resource/rawexd`；改之前先備份到 `backup/rawexd-before-update.zip` |
| `--lint` | 檢查翻譯 CSV | 只寫報告 `lint-report.txt` |
| `--sheetsig [base-ref]` | `Sheet` 標籤參照檢查 | 只寫報告 `sheetsig-report.txt` |
| `--gensheetsig <目錄>` | 從 SaintCoinach 日文匯出產生 `Sheet` 參照表 | `resource/ja-sheetsig.txt.gz` |
| `--driftcheck` | 跟上游比對錯位（需要 git） | 只寫報告 `rawexd-drift.txt` |
| `--hextags` | hex 標籤對照表 | 只寫報告 `hextags-report.txt` |
| `--s2tw <檔案>` | 印出簡轉繁結果，方便試 `UserPhrases.txt` | 不改任何檔案 |
| `--selftest` | 核心邏輯自檢 | 不改任何檔案 |

訊息印在主控台，也記到 `debug.log`。檢查類指令的結束碼是錯誤數，超過 255 以 255 計。

### 發版

在 GitHub 網頁照常建立 Release、上傳 `_CHT.zip` 後按發佈，CI（`.github/workflows/release.yml`）會自動編譯 `FFXIVChnTextPatch.exe` 附到同一個 Release。翻譯檔不必處理，`rawexd-latest` 由 `.github/workflows/rawexd-asset.yml` 自動維護。

### 和 C# 版的差異

實測速度（同一台電腦）：

| 工作 | C# | Rust |
|------|----|------|
| 漢化（7118 個資料表） | 約 32 秒 | 約 7 秒 |
| 簡轉繁＋合併（上游全量） | 約 29 秒 | 約 12 秒 |
| `--lint` | 約 8 秒 | 約 2 秒 |

- 讀到非 UTF-8 的 CSV（舊工具產生的 Big5 檔）會回報失敗，不會自動轉成 UTF-8；這類檔案請先用 C# 版的 `--update` 修復。
- CSV 引號欄位裡的空行會保留。C# 版的 `TextFieldParser` 會把這些空行吃掉，例如 Lobby 職業說明裡「開始地點」前面那行空行。
- 「更新翻譯 CSV」下載上游的那段時間進度條不會動（C# 版會顯示 git 的百分比）。

C# 版的編譯方式見 [`csharp/README.md`](csharp/README.md)。舊 Java 版的編譯筆記可參考[這裡](https://hackmd.io/@GpointChen/SJi_gv-ad)（原始碼在 git 歷史中）。

## 翻譯資源

- `resource/rawexd/` — CSV 翻譯檔（每個 EXD 表一個檔案）。各 CSV 對應遊戲內哪些文本，可參考 [Souma 版的 CSV 文件說明](https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma/wiki/CSV%E6%96%87%E4%BB%B6)。
- `resource/font/` — 替換字體（`.fdt` + `.tex`）。
- 設置頁的「跳過的資料表」可勾選漢化時要跳過的表（含中文說明、可搜尋），也可直接編輯 `conf/global.properties` 的 `SkipFiles`（`|` 分隔，格式如 `exd/quest`）。
- `conf/exd-names.csv` — 表名對應遊戲內文本位置的說明檔，用於設置頁清單、漢化進度與檢查報告。
- `resource/ja-sheetsig.txt.gz` — 日文原文的 `Sheet` 標籤參照表（約 137KB）。譯文裡 `Sheet` 標籤的參數決定遊戲「去哪張表、讀第幾欄」，陸版客戶端的欄位配置跟國際服不一定相同，照陸版的參數翻過來會讓那個介面閃退（2026-09 的莫古莫古指南書就是這樣）。`ffxiv_chn_text_patch.exe --sheetsig` 掃全部翻譯、`--sheetsig <base-ref>` 只看該 ref 之後改動到的儲存格（PR 上由 CI 自動跑），報告寫到 `sheetsig-report.txt`。參照表由 `--gensheetsig <SaintCoinach 日文匯出的 rawexd 目錄>` 產生，需要本機有遊戲，遊戲改版後要重產。

## 免責聲明（沿自原項目）

- 本程式以修改客戶端的方式載入中文資源，此舉違反官方規則，使用即表示自行承擔一切後果。
- 本專案僅供學習與技術交流使用，嚴禁任何商業用途。
