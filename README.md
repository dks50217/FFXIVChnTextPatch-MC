# FFXIVChnTextPatch-MC
基於 [FFXIVChnTextPatch-GP](https://github.com/GpointChen/FFXIVChnTextPatch-GP) 與 [FFXIVChnTextPatch-Souma](https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma) 的中文化工具。  
**注意：本工具會修改客戶端檔案，可能違反官方規範，使用者須自行承擔風險。**

## 支援與相容性 (Compatibility)
- 客戶端版本：國際服 7.38
- 平台：Windows（SE/Steam 版）、SteamOS 未測試

## 使用 CSV 進行漢化 (Using CSV)
- CSV 來源：SaintCoinach 匯出
- 流程：抽取 → 校對 → 生成 CSV → 匯入 → 客戶端注入  
- CSV 範例：見 `resource/rawexd/Addon.csv` (介面相關)

## 備份 / 還原 (Backup & Restore)
- 初次漢化前會自動備份至 `backup/`  
- 還原按鈕會回復到最近一次備份；**更新前務必先還原**。  
- 驗證方式：檢查 `*.dat0/index/index2` 檔案時間戳與大小。

或者執行以下指令備份，指定目錄至 `FINAL FANTASY XIV - A Realm Reborn\game\sqpack`

```bat
set backFolder="C:\FF14_bak"

if not exist %backFolder% mkdir %backFolder%
REM backup the org file
echo F | xcopy /Y "ffxiv\000000.win32.dat0" "%backFolder%\000000.win32.dat0"
echo F | xcopy /Y "ffxiv\000000.win32.index" "%backFolder%\000000.win32.index"
echo F | xcopy /Y "ffxiv\000000.win32.index2" "%backFolder%\000000.win32.index2"
echo F | xcopy /Y "ffxiv\0a0000.win32.dat0" "%backFolder%\0a0000.win32.dat0"
echo F | xcopy /Y "ffxiv\0a0000.win32.index" "%backFolder%\0a0000.win32.index"
echo F | xcopy /Y "ffxiv\0a0000.win32.index2" "%backFolder%\0a0000.win32.index2"
```

## 授權與致謝 (License & Credits)
- 上游專案：
  - [FFXIVChnTextPatch-GP](https://github.com/GpointChen/FFXIVChnTextPatch-GP)
  - [FFXIVChnTextPatch-Souma](https://github.com/Souma-Sumire/FFXIVChnTextPatch-Souma)

本專案遵循 **GPL-3.0 License**，基於上述上游專案進行修改與擴充。

其中，以下部分與上游專案保持完全一致：
- Java 執行環境（未來版本將可能改為 .NET 架構）
- `FFXIVChnTextPatchGP.exe`：漢化工具主程式（來自 FFXIVChnTextPatch-GP）

以下內容為本專案原創或自行維護：
- `resource/rawexd/`：以 **FFXIVChnTextPatch-Souma** 提供之簡體中文 CSV 為基礎，  
  由本人進行繁體中文翻譯與在地化調整，使其更貼近台灣用語習慣。

---

> 本專案依照 GPL-3.0 授權條款釋出，所有修改後之內容均遵循相同授權。  
> 原 CSV 檔案之著作權與原始貢獻者歸屬於各自上游專案作者。

## 風險與免責 (Disclaimer)
本程式以修改客戶端方式載入中文資源，**可能違反官方規範**。  
使用即表示你了解並願意承擔所有風險。本專案不提供任何商業用途，也不包含受保護的中文文本。