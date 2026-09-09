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

## Validation before reporting done

Run what the change touched, and say what passed. This is the same set `.github/workflows/build.yml` runs on every push and PR, so running it locally first just saves a red CI:

| Changed | Run |
|---------|-----|
| any C# | `dotnet build` |
| binary format, CSV merge, ZhConvert, Config | + `--selftest` (exit code = failure count) |
| `resource/rawexd/*.csv` | + `--lint` (exit code = errors that would break patching) |
| after `--update` | + `--driftcheck` (warn-only in CI) |

New non-trivial logic leaves one `--selftest` check behind — the smallest assertion that fails if it breaks. No test framework; `SelfTest.cs` is the whole harness.

## Architecture (`dotnet/FFXIVChnTextPatch/`)

- `Core/PatchService.cs` — orchestrates backup → font replace → CSV text replace, and rollback. Progress via `IProgress<PatchProgress>`.
- `Core/SqPack.cs` — SqPack `.index` parsing (CRC hash → offset map) and `.dat` extraction (content type 2 only; types 3/4 extraction intentionally not ported).
- `Core/Exd.cs` — EXH/EXD game-data table parsing. **Big-endian**, unlike SqPack index/dat which are little-endian.
- `Core/Builders.cs` — rebuilds modified binary blocks (`BinaryBlockBuilder` type 2, `TexBlockBuilder` type 4 fonts, `EXDFBuilder` EXD rows).
- `Core/FFCRC.cs` — FFXIV's custom CRC for file-path hashing. Verified bit-exact against the Java original via `--selftest` vectors.
- `Core/Config.cs` — Java `.properties`-compatible read/write of `conf/global.properties` (handles `\:` escapes).
- `Core/ExdNames.cs` — EXD sheet name → Chinese UI-location description, loaded from `conf/exd-names.csv` (folder keys end with `/`, e.g. `quest/`, to avoid case-insensitive collision with sheet names like `Quest`). Used by patch progress, lint report, and the settings skip-list.
- `Core/RawexdUpdater.cs` — one-click translation update (UI「更新翻譯 CSV」button or `--update` CLI flag): git sparse clone of the upstream repo (`UpstreamRepo` config key, default Souma's repo) → `ZhConvert` → `RawexdMerge`. Backs up `resource/rawexd` to `backup/rawexd-before-update.zip` first. Requires git on PATH.
- `Core/RawexdMerge.cs` — cell-level rawexd CSV merge: non-empty local cells always win; empty cells / missing rows / missing files are filled from upstream; columns aligned by the offset row (survives cross-version column changes); local-only rows appended at EOF.
- `Core/ZhConvert.cs` — simplified→traditional with Taiwan vocabulary (OpenCC s2twp equivalent plus FFXIV-specific fixes) via longest-forward-match over TSV dictionaries in `resource/opencc/`: GPPhrases+STPhrases+STCharacters → TWPhrases → TWVariants. `GPPhrases.txt` is the FFXIV exception glossary inherited from the Java GP version (converted from `resource/nlpcn/traditional.txt` in git history; includes quote rules and English-name protection entries). `UserPhrases.txt` is the user-editable override list, loaded ahead of rounds 1 and 2 so it beats everything; simplified or traditional keys both work.
- `Main.razor` + `wwwroot/` — UI (main panel + settings) hosted in a WPF `BlazorWebView` (`MainWindow.xaml`).
- `SelfTest.cs` — run with `--selftest`; keep it passing when touching any binary-format code.

## When the game crashes after patching

A crash confined to one UI is almost always a translated cell whose SeString control tags don't match what the international client's original string has — the CN client's phrasing carries a different tag structure, and the UI reads a parameter that was never passed. Text content itself never causes this.

Diagnosis is empirical; static analysis of `<hex:>` tags alone produces too many false positives to name a row (parameter bytes routinely contain `02 XX` sequences that look like tag starts).

1. Bisect by sheet with `SkipFiles` (`exd/<lowercase name>`, pipe-separated) until one sheet is confirmed.
2. Export the same game version's JA rawexd with SaintCoinach (`SaintCoinach.Cmd`, output under `<version>/rawexd/`). Same exporter, so `<hex:>` chunking is identical and chunk sequences can be compared literally — this is the only reliable comparison.
3. In the affected row range, list rows whose tag chunk sequence differs from the JA original. That candidate set contains the culprit.
4. Blank those cells (empty = keep original text), re-patch, then halve until one row is left.
5. Fix by re-translating the row to the JA tag structure, not by leaving it blank — **`--update` refills empty cells from upstream**, so a blanked workaround silently comes back.

Observed in the one case diagnosed so far (Moogle guidebook, `Addon`): two rows that drop tags relative to the JA original — 15949 and 15955 — were excluded by bisection, so dropping a tag did not trigger *that* crash. This is not a general rule: it holds for those two rows only. Treat every row whose tag sequence differs from the JA original as a candidate, whichever direction it differs in, until re-patching rules it out.

Compare full chunk sequences, not tag counts. The five rows that survived bisection all carry the *same* data-tag counts as the JA original and differ only in parameter bytes or ordering — a count-based check would have cleared every one of them.

Row-id drift was ruled out in that case: rows aligned one-to-one with the JA export.

## Key constraints

- **Only CSV translation mode is supported** (`FLanguage=CSV`, reads `resource/rawexd/*.csv`). The legacy CN-client-file mode (EXDFUtil/JianFan/transtable) was deliberately not ported.
- rawexd CSVs: lines starting with `#` are comments; effective row 1 (0-based) is the column-offset row, data starts at row 3. Empty CSV cells mean "keep original text".
- Patching appends rebuilt blocks to the end of `.dat0` and repoints the `.index` entry offset (at entry position + 8) — original data is never modified in place. Backups of the six index/dat files go to `backup/` before patching.
- International-server "say-to-do" quests: in `quest/*` CSVs, translate `*_SAYTODO_*` rows to English (the phrase the player must actually type — typing Chinese is discouraged on the intl client), and in the paired `*_SYSTEM_*` prompt keep the Chinese but append the English in parentheses after the 「」quoted phrase. `LintTool` flags any `*_SAYTODO_*` cell still containing Chinese.

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
- `resource/opencc/` — OpenCC dictionary TSVs used by `ZhConvert` (ship with the app)
- `docs/DOTNET_MIGRATION.md` — Java→C# port notes (behavioral reference now that Java code is removed)
