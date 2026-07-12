# Release 描述範本

每次發版把下面內容貼到 GitHub Release 描述，填好版本號即可。
資產命名要對得上程式與 README：`FFXIVChnTextPatch.exe`、`rawexd-opencc.zip`、`YYYYMMDDXX_CHT.zip`。
（`rawexd-opencc.zip` 根目錄需直接是 `rawexd/` 與 `opencc/` 兩個資料夾。）

---

## FFXIV 繁中漢化 — 對應遊戲版本 `YYYY.MM.DD.XXXX`

三個檔案，依需求擇一下載：

### 檔案說明

| 檔案 | 內容 | 給誰 |
|------|------|------|
| **`FFXIVChnTextPatch.exe`** | 單檔漢化工具（自帶 .NET Runtime）。自己漢化、可備份還原、可線上更新翻譯。 | 想自己控制漢化流程的人 |
| **`rawexd-opencc.zip`** | 翻譯文本（`rawexd`）＋ 簡繁字典（`opencc`）。 | 搭配上面的 exe；缺翻譯檔時 exe 會提示自動下載此包，通常不用手動抓。 |
| **`YYYYMMDDXX_CHT.zip`** | 已漢化完成的六個 `index/dat` 檔。 | 只想直接玩、不跑工具的人。 |

### 用法

**方式 A：用 exe 自己漢化（可還原、可更新翻譯）**
1. 下載 `FFXIVChnTextPatch.exe` 直接開啟（需 Windows + WebView2 Runtime）。
2. 首次啟動偵測不到翻譯檔時，會提示一鍵下載 `rawexd-opencc.zip`（不需 git）。
3. 設定遊戲路徑 → 點「漢化」。可隨時「還原」。
   - 要「替換字體」需自行到本 repo 下載字體檔放進 `resource/font`（太大未內含）。

**方式 B：直接套用漢化好的檔（最快）**
1. 下載 `YYYYMMDDXX_CHT.zip`。
2. 解壓後把六個檔覆蓋到 `<遊戲根目錄>\game\sqpack\ffxiv\`（覆蓋前請自行備份）。
3. 此方式不能用工具「還原」；遊戲改版後刪掉覆蓋檔讓官方更新即可。

### 注意
- 修改客戶端違反官方規則，使用者自行承擔後果。
- 遊戲更新後漢化會失效，請重新漢化（方式 A）或重新套用新版 zip（方式 B）。

---

**變更**
- （這裡列本版翻譯/程式更動）
