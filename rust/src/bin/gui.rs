//! 操作畫面（egui）。對應 C# 版的 Main.razor：主畫面（漢化／還原／設置、檢查與更新翻譯 CSV）與設置頁。
//! 工作在背景執行緒跑，畫面每 0.1 秒讀一次 ffxiv_chn_text_patch::current_progress() 更新進度條。
// 雙擊開啟時不要帶出黑色主控台視窗
#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(non_snake_case)] // 執行檔名 FFXIVChnTextPatch 沿用 C# 版

#[cfg(not(windows))]
fn main() {
    eprintln!("操作畫面只支援 Windows；命令列請用 ffxiv_chn_text_patch。");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() -> eframe::Result {
    app::run()
}

#[cfg(windows)]
mod app {
    use eframe::egui::{self, Color32, RichText};
    use ffxiv_chn_text_patch::config::Config;
    use ffxiv_chn_text_patch::{bootstrap, current_progress, exdnames, hextags, lint, log, p, patch, update};
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;

    const WRONG_FOLDER: &str = "請選擇正確的遊戲根目錄，目錄預設名為：FINAL FANTASY XIV ONLINE";
    const LANGS: [(&str, &str); 5] = [("JA", "日文"), ("EN", "英文"), ("DE", "德文"), ("FR", "法文"), ("CHS", "簡體中文")];

    pub fn run() -> eframe::Result {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("FFXIVChnTextPatch")
                .with_inner_size([520.0, 440.0])
                .with_min_inner_size([420.0, 320.0])
                .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon.png")).expect("assets/icon.png 不是有效的 PNG")),
            ..Default::default()
        };
        eframe::run_native("FFXIVChnTextPatch", options, Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx)))))
    }

    /// egui 內建字型沒有中文，改用 Windows 內建的微軟正黑體（找不到就退回新細明體）。
    fn setup_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        for name in ["msjh.ttc", "msjh.ttf", "mingliu.ttc"] {
            if let Ok(bytes) = std::fs::read(format!(r"{windir}\Fonts\{name}")) {
                fonts.font_data.insert("cjk".into(), Arc::new(egui::FontData::from_owned(bytes)));
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    fonts.families.entry(family).or_default().insert(0, "cjk".into());
                }
                break;
            }
        }
        ctx.set_fonts(fonts);
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Job {
        Patch,
        Rollback,
        Update,
        Lint,
        Download,
        HexTags,
    }

    impl Job {
        fn label(self) -> &'static str {
            match self {
                Job::Patch => "漢化",
                Job::Rollback => "還原",
                Job::Update => "更新翻譯",
                Job::Lint => "檢查翻譯",
                Job::Download => "下載翻譯檔",
                Job::HexTags => "產生 hex 標籤對照表",
            }
        }
    }

    /// 跳過清單的一項：key 是寫進 SkipFiles 的值（exd/小寫表名，資料夾為前綴匹配）。
    struct SkipEntry {
        key: String,
        display: String,
        desc: Option<&'static str>,
    }

    struct App {
        cfg: Config,
        status: (String, bool),
        show_config: bool,
        // 設置頁表單
        game_path: String,
        s_lang: String,
        replace_font: bool,
        replace_text: bool,
        skip: BTreeSet<String>,
        skip_entries: Vec<SkipEntry>,
        skip_filter: String,
        // 工作與訊息
        confirm: Option<Job>,
        running: Option<(Job, JoinHandle<(bool, String)>)>,
        message: Option<(bool, String)>, // (是不是錯誤, 內容)
    }

    impl App {
        fn new(ctx: &egui::Context) -> App {
            setup_fonts(ctx);
            let cfg = Config::load(&p("conf/global.properties"));
            let mut app = App {
                status: patch::patch_status(&cfg),
                show_config: !patch::is_ffxiv_folder(cfg.get_or("GamePath", "")),
                cfg,
                game_path: String::new(),
                s_lang: String::new(),
                replace_font: false,
                replace_text: true,
                skip: BTreeSet::new(),
                skip_entries: build_skip_entries(),
                skip_filter: String::new(),
                confirm: None,
                running: None,
                message: None,
            };
            app.load_form();
            if bootstrap::needs_csv() {
                app.confirm = Some(Job::Download); // 跟 C# 版一樣，首次執行缺翻譯檔就問要不要下載
            }
            app
        }

        fn load_form(&mut self) {
            self.game_path = self.cfg.get_or("GamePath", "").to_string();
            self.s_lang = self.cfg.get_or("SLanguage", "JA").to_string();
            self.replace_font = self.cfg.get("ReplaFont") == Some("1");
            self.replace_text = self.cfg.get("ReplaText") != Some("0");
            self.skip = self.cfg.get_or("SkipFiles", "").split('|').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
        }

        fn save_config(&mut self) {
            if !patch::is_ffxiv_folder(&self.game_path) {
                self.message = Some((true, WRONG_FOLDER.into()));
                return;
            }
            let dlang = self.cfg.get_or("DLanguage", "CHS").to_string();
            let skip = self.skip.iter().cloned().collect::<Vec<_>>().join("|"); // BTreeSet = 跟 C# 一樣照 ordinal 排序
            for (k, v) in [
                ("GamePath", self.game_path.clone()),
                ("SLanguage", self.s_lang.clone()),
                ("DLanguage", dlang),
                ("FLanguage", "CSV".into()),
                ("ReplaFont", if self.replace_font { "1" } else { "0" }.into()),
                ("ReplaText", if self.replace_text { "1" } else { "0" }.into()),
                ("SkipFiles", skip),
                ("TransMode", "0".into()),
            ] {
                self.cfg.set(k, &v);
            }
            match self.cfg.save() {
                Ok(()) => {
                    self.message = None;
                    self.show_config = false;
                    self.status = patch::patch_status(&self.cfg);
                }
                Err(e) => self.message = Some((true, format!("設定存檔失敗：{e}"))),
            }
        }

        /// 工作在背景執行緒跑；它自己從磁碟讀設定、改完存回去，結束後畫面再重新讀一次。
        fn start(&mut self, job: Job) {
            self.message = None;
            ffxiv_chn_text_patch::progress(0.0, "準備中……", "");
            let handle = std::thread::spawn(move || {
                let mut cfg = Config::load(&p("conf/global.properties"));
                let result = match job {
                    Job::Patch => patch::patch(&mut cfg),
                    Job::Rollback => patch::rollback(&mut cfg),
                    Job::Update => update::update(&cfg),
                    Job::Download => bootstrap::download(&cfg),
                    Job::HexTags => return (true, hextags::run()),
                    Job::Lint => {
                        let (errors, summary) = lint::run(&cfg);
                        return (errors == 0, summary);
                    }
                };
                match result {
                    Ok(msg) => (true, msg),
                    Err(e) => (false, format!("{}失敗：{e}", job.label())),
                }
            });
            self.running = Some((job, handle));
        }

        fn poll_job(&mut self) {
            if !self.running.as_ref().is_some_and(|(_, h)| h.is_finished()) {
                return;
            }
            let (job, handle) = self.running.take().unwrap();
            let (ok, msg) = handle.join().unwrap_or_else(|_| (false, format!("{}時發生未預期的錯誤", job.label())));
            log(&msg);
            self.message = Some((!ok, msg));
            self.cfg = Config::load(&p("conf/global.properties"));
            self.status = patch::patch_status(&self.cfg);
            if matches!(job, Job::Update | Job::Download) {
                self.skip_entries = build_skip_entries(); // 可能多了新檔
            }
        }

        fn main_page(&mut self, ui: &mut egui::Ui) {
            let busy = self.running.is_some();
            ui.heading("PatchTool");
            ui.label(RichText::new(self.cfg.get_or("GamePath", "")).weak());
            let (text, warn) = &self.status;
            ui.label(if *warn { RichText::new(text).color(Color32::from_rgb(230, 140, 0)) } else { RichText::new(text) });
            ui.add_space(12.0);

            let big = |text| egui::Button::new(RichText::new(text).size(18.0)).min_size(egui::vec2(110.0, 40.0));
            ui.horizontal(|ui| {
                let idle = !busy && self.confirm.is_none();
                if ui.add_enabled(idle, big("漢化")).clicked() {
                    self.confirm = Some(Job::Patch);
                }
                if ui.add_enabled(idle, big("還原")).clicked() {
                    self.confirm = Some(Job::Rollback);
                }
                if ui.add_enabled(idle, big("設置")).clicked() {
                    self.message = None;
                    self.show_config = true;
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!busy, egui::Button::new("檢查翻譯 CSV")).clicked() {
                    self.start(Job::Lint);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("更新翻譯 CSV"))
                    .on_hover_text("下載 Souma 上游翻譯 → 簡轉繁 → 合併（本地已有的翻譯不會被覆蓋）。需要 git。")
                    .clicked()
                {
                    self.start(Job::Update);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("hex 標籤對照表"))
                    .on_hover_text("掃描所有翻譯 CSV 的 <hex:> 標籤，解成可讀名稱輸出 hextags-report.txt（給翻譯者對照，不修改任何檔案）")
                    .clicked()
                {
                    self.start(Job::HexTags);
                }
            });

            self.show_progress(ui);
            self.show_message(ui);
        }

        /// 背景工作中就畫進度條。兩頁都會呼叫：首次執行還沒設遊戲路徑時停在設置頁，下載翻譯檔的進度要看得到。
        fn show_progress(&self, ui: &mut egui::Ui) {
            if self.running.is_some() {
                ui.add_space(12.0);
                let pr = current_progress();
                ui.add(egui::ProgressBar::new(pr.percent).show_percentage());
                ui.label(format!("{}{}", pr.action, pr.detail));
            }
        }

        fn config_page(&mut self, ui: &mut egui::Ui) {
            ui.heading("漢化設置");
            ui.add_space(8.0);
            egui::Grid::new("config").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("遊戲路徑");
                ui.horizontal(|ui| {
                    if ui.button("瀏覽…").clicked() {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            let dir = dir.to_string_lossy().to_string();
                            if patch::is_ffxiv_folder(&dir) {
                                self.game_path = dir;
                                self.message = None;
                            } else {
                                self.message = Some((true, WRONG_FOLDER.into()));
                            }
                        }
                    }
                    let shown = if self.game_path.is_empty() { "（未設定）" } else { self.game_path.as_str() };
                    ui.add(egui::Label::new(shown).truncate()).on_hover_text(shown); // 路徑太長時截斷，滑過看完整路徑
                });
                ui.end_row();

                ui.label("檔案語言");
                ui.label("CSV（resource/rawexd）");
                ui.end_row();

                ui.label("原始語言");
                let current = LANGS.iter().find(|(k, _)| *k == self.s_lang).map_or(self.s_lang.as_str(), |(_, v)| v);
                egui::ComboBox::from_id_salt("slang").selected_text(current).show_ui(ui, |ui| {
                    for (k, v) in LANGS {
                        ui.selectable_value(&mut self.s_lang, k.to_string(), v);
                    }
                });
                ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.replace_font, "替換字體");
                ui.checkbox(&mut self.replace_text, "替換文本");
            });
            if self.replace_font && !bootstrap::has_font() {
                ui.label(RichText::new(format!("找不到字體檔（resource/font）。字體檔較大未隨程式附帶，需替換字體請自行到 {} 下載後放入 resource/font。", bootstrap::REPO_URL)).color(Color32::from_rgb(230, 140, 0)));
            }

            egui::CollapsingHeader::new(format!("跳過的資料表（已勾選 {} 項）", self.skip.len())).show(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.skip_filter).hint_text("搜尋表名或說明…"));
                let filter = self.skip_filter.to_lowercase();
                let shown: Vec<usize> = (0..self.skip_entries.len())
                    .filter(|&i| {
                        let e = &self.skip_entries[i];
                        filter.is_empty() || e.display.to_lowercase().contains(&filter) || e.desc.is_some_and(|d| d.contains(&filter))
                    })
                    .collect();
                let row_height = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
                // 七千多項，只畫看得到的那幾列
                egui::ScrollArea::vertical().max_height(180.0).show_rows(ui, row_height, shown.len(), |ui, range| {
                    for &i in &shown[range] {
                        let e = &self.skip_entries[i];
                        let mut on = self.skip.contains(&e.key);
                        let text = match e.desc {
                            Some(d) => format!("{}　{d}", e.display),
                            None => e.display.clone(),
                        };
                        if ui.checkbox(&mut on, text).changed() {
                            if on {
                                self.skip.insert(e.key.clone());
                            } else {
                                self.skip.remove(&e.key);
                            }
                        }
                    }
                });
            });

            self.show_progress(ui);
            self.show_message(ui);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("確認").strong()).clicked() {
                    self.save_config();
                }
                // 還沒設好遊戲路徑就沒有主畫面可回
                if ui.add_enabled(patch::is_ffxiv_folder(self.cfg.get_or("GamePath", "")), egui::Button::new("返回")).clicked() {
                    self.load_form();
                    self.message = None;
                    self.show_config = false;
                }
            });
        }

        fn show_message(&self, ui: &mut egui::Ui) {
            if let Some((is_error, msg)) = &self.message {
                ui.add_space(10.0);
                let color = if *is_error { Color32::from_rgb(220, 70, 70) } else { Color32::from_rgb(60, 170, 90) };
                ui.label(RichText::new(msg).color(color));
            }
        }

        fn confirm_dialog(&mut self, ctx: &egui::Context) {
            let Some(job) = self.confirm else { return };
            let modal = egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
                ui.set_width(320.0);
                let (title, note, cancel, ok) = match job {
                    Job::Download => (
                        "缺少翻譯檔".to_string(),
                        format!("偵測不到翻譯 CSV（resource/rawexd）。是否從 GitHub 下載最新翻譯檔？
字體檔太大不含在內，需替換字體請自行到 {} 下載。", bootstrap::REPO_URL),
                        "稍後",
                        "下載",
                    ),
                    Job::Rollback => (format!("確定要{}嗎？", job.label()), "將把六個遊戲檔還原成漢化前的備份。".into(), "取消", "確定"),
                    _ => (format!("確定要{}嗎？", job.label()), String::new(), "取消", "確定"),
                };
                ui.heading(title);
                if !note.is_empty() {
                    ui.label(note);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(cancel).clicked() {
                        self.confirm = None;
                    }
                    if ui.button(RichText::new(ok).strong()).clicked() {
                        self.confirm = None;
                        self.start(job);
                    }
                });
            });
            if modal.should_close() {
                self.confirm = None;
            }
        }
    }

    impl eframe::App for App {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.poll_job();
            if self.running.is_some() {
                ui.ctx().request_repaint_after(Duration::from_millis(100)); // 背景工作中，定時刷新進度
            }
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.show_config {
                        self.config_page(ui);
                    } else {
                        self.main_page(ui);
                    }
                });
            });
            let ctx = ui.ctx().clone();
            self.confirm_dialog(&ctx);
        }
    }

    /// rawexd 頂層的子資料夾各一項（跳過整個資料夾），頂層 CSV 各一項；照名稱不分大小寫排序。
    fn build_skip_entries() -> Vec<SkipEntry> {
        let (mut dirs, mut files) = (Vec::new(), Vec::new());
        for e in std::fs::read_dir(p("resource/rawexd")).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() {
                dirs.push(SkipEntry {
                    key: format!("exd/{}", name.to_lowercase()),
                    display: format!("{name}/（整個資料夾）"),
                    desc: exdnames::describe(&format!("{name}/")),
                });
            } else if let Some(stem) = name.strip_suffix(".csv") {
                files.push(SkipEntry { key: format!("exd/{}", stem.to_lowercase()), display: stem.to_string(), desc: exdnames::describe(stem) });
            }
        }
        dirs.sort_by_key(|e| e.display.to_uppercase());
        files.sort_by_key(|e| e.display.to_uppercase());
        dirs.extend(files);
        dirs
    }
}
