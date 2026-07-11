# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A tool that applies Chinese localization patches to the FFXIV (Final Fantasy XIV) international client. It reads FFXIV's proprietary SqPack binary format, replaces text content with Chinese translations from CSV files (SaintCoinach rawexd exports), and optionally replaces font files.

The current implementation is **C#/.NET 10 WPF Blazor Hybrid** in `dotnet/FFXIVChnTextPatch/`. It was ported from a Java Swing app; the Java sources were removed from the working tree but remain in git history (and `docs/DOTNET_MIGRATION.md` documents the port).

## Build & Run

Requires .NET 10 SDK (Windows) and WebView2 Runtime.

```bash
cd dotnet/FFXIVChnTextPatch
dotnet build
dotnet run                                  # GUI
./bin/Debug/net10.0-windows10.0.17763.0/FFXIVChnTextPatch.exe --selftest
                                            # binary-format checks → selftest.log in repo root
dotnet publish -c Release -r win-x64 --self-contained -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true
                                            # distributable: publish/FFXIVChnTextPatch.exe + wwwroot/
                                            # (ship together with conf/ and resource/)
```

Note: the exe is a GUI app — invoking `--selftest` from a shell returns immediately; wait a moment before reading selftest.log.

The app locates its base directory (for `conf/`, `resource/`, `backup/`, `debug.log`) by walking up from the exe until it finds `conf/global.properties`.

## Architecture (`dotnet/FFXIVChnTextPatch/`)

- `Core/PatchService.cs` — orchestrates backup → font replace → CSV text replace, and rollback. Progress via `IProgress<PatchProgress>`.
- `Core/SqPack.cs` — SqPack `.index` parsing (CRC hash → offset map) and `.dat` extraction (content type 2 only; types 3/4 extraction intentionally not ported).
- `Core/Exd.cs` — EXH/EXD game-data table parsing. **Big-endian**, unlike SqPack index/dat which are little-endian.
- `Core/Builders.cs` — rebuilds modified binary blocks (`BinaryBlockBuilder` type 2, `TexBlockBuilder` type 4 fonts, `EXDFBuilder` EXD rows).
- `Core/FFCRC.cs` — FFXIV's custom CRC for file-path hashing. Verified bit-exact against the Java original via `--selftest` vectors.
- `Core/Config.cs` — Java `.properties`-compatible read/write of `conf/global.properties` (handles `\:` escapes).
- `Core/ExdNames.cs` — EXD sheet name → Chinese UI-location description, loaded from `conf/exd-names.csv` (folder keys end with `/`, e.g. `quest/`, to avoid case-insensitive collision with sheet names like `Quest`). Used by patch progress, lint report, and the settings skip-list.
- `Main.razor` + `wwwroot/` — UI (main panel + settings) hosted in a WPF `BlazorWebView` (`MainWindow.xaml`).
- `SelfTest.cs` — run with `--selftest`; keep it passing when touching any binary-format code.

## Key constraints

- **Only CSV translation mode is supported** (`FLanguage=CSV`, reads `resource/rawexd/*.csv`). The legacy CN-client-file mode (EXDFUtil/JianFan/transtable) was deliberately not ported.
- rawexd CSVs: lines starting with `#` are comments; effective row 1 (0-based) is the column-offset row, data starts at row 3. Empty CSV cells mean "keep original text".
- Patching appends rebuilt blocks to the end of `.dat0` and repoints the `.index` entry offset (at entry position + 8) — original data is never modified in place. Backups of the six index/dat files go to `backup/` before patching.

## Configuration (`conf/global.properties`)

| Key | Description |
|-----|-------------|
| `GamePath` | FFXIV root directory (must contain `game/ffxiv_dx11.exe`) |
| `SLanguage` | Source language to overwrite (usually `JA`) |
| `FLanguage` | Must be `CSV` (only supported mode) |
| `ReplaFont` / `ReplaText` | `1`/`0` toggles for font and text replacement |
| `SkipFiles` | Pipe-separated EXD names to skip, format `exd/<lowercase name>` (folder entries skip the whole subtree); editable via the settings-page checklist |

## Resources

- `resource/rawexd/` — CSV translation files (one per EXD sheet)
- `resource/font/` — replacement fonts (`.fdt` + `.tex`)
- `docs/DOTNET_MIGRATION.md` — Java→C# port notes (behavioral reference now that Java code is removed)
