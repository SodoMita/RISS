#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};
use crate::history::HistoryData;
use crate::search::{self, MatchType, SearchEngine, SearchResult};
use eframe::egui;
use egui::{Color32, CornerRadius, FontId, RichText, Stroke, Vec2};

/// RISS color scheme
struct Colors;

impl Colors {
    const BG: Color32 = Color32::from_rgb(30, 30, 46);
    const BG_LIGHT: Color32 = Color32::from_rgb(40, 40, 58);
    const BG_HOVER: Color32 = Color32::from_rgb(55, 55, 80);
    const TEXT: Color32 = Color32::from_rgb(205, 214, 244);
    const TEXT_DIM: Color32 = Color32::from_rgb(147, 153, 178);
    const ACCENT: Color32 = Color32::from_rgb(137, 180, 250);
    const ACCENT_DIM: Color32 = Color32::from_rgb(110, 145, 210);
    const FAVORITE: Color32 = Color32::from_rgb(250, 204, 94);
    const BORDER: Color32 = Color32::from_rgb(69, 71, 90);
    const SEARCH_BG: Color32 = Color32::from_rgb(49, 50, 68);
    const CALC_BG: Color32 = Color32::from_rgb(50, 70, 50);
    const CALC_TEXT: Color32 = Color32::from_rgb(166, 227, 161);
}

/// Main application state
pub struct RissApp {
    /// Search query
    query: String,
    /// All discovered applications
    apps: Vec<AppEntry>,
    /// Current search results
    results: Vec<SearchResult>,
    /// Search engine
    search_engine: SearchEngine,
    /// History data
    history: HistoryData,
    /// Selected result index (for keyboard nav)
    selected_index: usize,
    /// Whether the search bar has focus
    search_focused: bool,
    /// Status message (shown briefly)
    status_message: Option<(String, std::time::Instant)>,
    /// Tag editing state
    editing_tags: Option<String>,
    tag_input: String,
}

impl RissApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let history = HistoryData::load();
        let mut apps = app_entry::discover_apps();

        // Merge in builtin entries (only if no similar app found)
        let builtins = app_entry::builtin_entries();
        for builtin in builtins {
            let dominated = apps
                .iter()
                .any(|a| a.name.to_lowercase() == builtin.name.to_lowercase());
            if !dominated {
                apps.push(builtin);
            }
        }

        // Merge history data into apps
        for app in &mut apps {
            app.launch_count = history.get_launch_count(&app.exec);
            app.last_launched = history.get_last_launched(&app.exec);
            app.is_favorite = history.is_favorite(&app.exec);
            app.tags = history.get_tags(&app.exec);
        }

        let search_engine = SearchEngine::new();

        let mut riss = Self {
            query: String::new(),
            apps,
            results: Vec::new(),
            search_engine,
            history,
            selected_index: 0,
            search_focused: true,
            status_message: None,
            editing_tags: None,
            tag_input: String::new(),
        };

        riss.update_results();
        riss
    }

    fn update_results(&mut self) {
        if self.query.trim().is_empty() {
            // Show ALL apps when query is empty, sorted by favorites first, then by name
            let mut all_apps: Vec<SearchResult> = self
                .apps
                .iter()
                .map(|app| SearchResult {
                    entry: app.clone(),
                    score: if app.is_favorite { 1000 } else { 0 } + app.launch_count as i64,
                    match_type: MatchType::Exact,
                })
                .collect();

            // Sort: favorites first, then by launch count, then alphabetically
            all_apps.sort_by(|a, b| {
                b.score.cmp(&a.score).then_with(|| {
                    a.entry
                        .name
                        .to_lowercase()
                        .cmp(&b.entry.name.to_lowercase())
                })
            });

            self.results = all_apps;
        } else {
            self.results = self.search_engine.search(&self.query, &self.apps, 20);
        }
        self.selected_index = 0;
    }

    fn launch_app(&mut self, index: usize) {
        if let Some(result) = self.results.get(index) {
            let exec = result.entry.exec.clone();
            let name = result.entry.name.clone();

            match result.entry.launch() {
                Ok(()) => {
                    self.history.record_launch(&exec);
                    // Update in-memory state
                    for app in &mut self.apps {
                        if app.exec == exec {
                            app.launch_count = self.history.get_launch_count(&app.exec);
                            app.last_launched = self.history.get_last_launched(&app.exec);
                        }
                    }
                    self.history.save();
                    self.set_status(format!("Launched {}", name));
                    // Clear search
                    self.query.clear();
                    self.update_results();
                }
                Err(e) => {
                    self.set_status(format!("Error: {}", e));
                }
            }
        }
    }

    fn toggle_favorite(&mut self, index: usize) {
        // Clone the necessary data to avoid borrow conflicts
        let entry_data = self
            .results
            .get(index)
            .map(|r| (r.entry.exec.clone(), r.entry.name.clone()));

        if let Some((exec, name)) = entry_data {
            let is_fav = self.history.toggle_favorite(&exec);
            for app in &mut self.apps {
                if app.exec == exec {
                    app.is_favorite = is_fav;
                }
            }
            self.history.save();
            self.update_results();
            self.set_status(if is_fav {
                format!("Added {} to favorites", name)
            } else {
                format!("Removed {} from favorites", name)
            });
        }
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, std::time::Instant::now()));
    }

    fn get_category_icon(category: &str) -> &str {
        match category.to_lowercase().as_str() {
            "game" | "games" => "🎮",
            "development" => "💻",
            "education" => "📚",
            "graphics" => "🎨",
            "audio" | "audiovideo" | "music" => "🎵",
            "video" => "🎬",
            "network" | "internet" => "🌐",
            "office" => "📄",
            "settings" | "system" => "⚙️",
            "utility" | "utilities" => "🔧",
            "accessories" => "📎",
            "science" => "🔬",
            "filemanager" => "📁",
            _ => "📦",
        }
    }

    fn get_app_icon(entry: &AppEntry) -> String {
        // Use category icon or a generic one
        if let Some(cat) = entry.categories.first() {
            Self::get_category_icon(cat).to_string()
        } else if entry.name.to_lowercase().contains("terminal") {
            "⌨️".to_string()
        } else if entry.name.to_lowercase().contains("browser")
            || entry.name.to_lowercase().contains("firefox")
            || entry.name.to_lowercase().contains("chrome")
        {
            "🌐".to_string()
        } else if entry.name.to_lowercase().contains("file") {
            "📁".to_string()
        } else if entry.name.to_lowercase().contains("text")
            || entry.name.to_lowercase().contains("editor")
        {
            "📝".to_string()
        } else if entry.name.to_lowercase().contains("calc") {
            "🧮".to_string()
        } else if entry.name.to_lowercase().contains("mail")
            || entry.name.to_lowercase().contains("thunder")
        {
            "✉️".to_string()
        } else if entry.name.to_lowercase().contains("music")
            || entry.name.to_lowercase().contains("spotify")
        {
            "🎵".to_string()
        } else {
            // First letter of the name
            entry
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string()
        }
    }
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(not(target_os = "android"))]
    {
        // Try to load DejaVu Sans from system fonts on Linux
        let font_paths = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ];

        for path in &font_paths {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert(
                    "system_font".to_owned(),
                    egui::FontData::from_owned(data).into(),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "system_font".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, "system_font".to_owned());
                break;
            }
        }
    }

    #[cfg(target_os = "android")]
    {
        // On Android, try to load Roboto from system fonts
        let android_font_paths = [
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/NotoSans-Regular.ttf",
            "/system/fonts/DroidSans.ttf",
        ];

        for path in &android_font_paths {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert(
                    "system_font".to_owned(),
                    egui::FontData::from_owned(data).into(),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "system_font".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, "system_font".to_owned());
                break;
            }
        }
    }

    // Ignore font errors silently - fall back to default fonts
    ctx.set_fonts(fonts);
}

impl eframe::App for RissApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply dark theme
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Colors::TEXT);
        visuals.window_fill = Colors::BG;
        visuals.panel_fill = Colors::BG;
        visuals.widgets.noninteractive.bg_fill = Colors::BG_LIGHT;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Colors::TEXT);
        visuals.widgets.inactive.bg_fill = Colors::SEARCH_BG;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Colors::TEXT);
        visuals.widgets.hovered.bg_fill = Colors::BG_HOVER;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Colors::TEXT);
        visuals.widgets.active.bg_fill = Colors::ACCENT_DIM;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Colors::TEXT);
        visuals.selection.bg_fill = Colors::ACCENT_DIM;
        ctx.set_visuals(visuals);

        // Handle keyboard shortcuts
        self.handle_keyboard(ctx);

        // Clear stale status messages (after 3 seconds)
        if let Some((_, time)) = &self.status_message {
            if time.elapsed().as_secs() > 3 {
                self.status_message = None;
            }
        }

        // Main layout: Top area with results, bottom area with search bar (RISS style)
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                // Top: Header with branding
                ui.allocate_ui(Vec2::new(ui.available_width(), 40.0), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("⚡ RISS")
                                .color(Colors::ACCENT)
                                .font(FontId::proportional(18.0))
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Launcher")
                                .color(Colors::TEXT_DIM)
                                .font(FontId::proportional(18.0)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(12.0);
                            if ui
                                .add(
                                    egui::Label::new(
                                        RichText::new("🔄")
                                            .font(FontId::proportional(16.0))
                                            .color(Colors::TEXT_DIM),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .clicked()
                            {
                                self.history = HistoryData::load();
                                self.apps = app_entry::discover_apps();
                                for app in &mut self.apps {
                                    app.launch_count = self.history.get_launch_count(&app.exec);
                                    app.last_launched = self.history.get_last_launched(&app.exec);
                                    app.is_favorite = self.history.is_favorite(&app.exec);
                                    app.tags = history::get_tags_safe(&self.history, &app.exec);
                                }
                                self.update_results();
                                self.set_status("Apps refreshed".to_string());
                            }
                            ui.label(
                                RichText::new(format!("{} apps", self.apps.len()))
                                    .color(Colors::TEXT_DIM)
                                    .font(FontId::proportional(12.0)),
                            );
                        });
                    });
                });

                // Middle: Results area
                let available_height = ui.available_height() - 120.0; // Reserve space for search bar
                egui::ScrollArea::vertical()
                    .max_height(available_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.show_results(ui);
                    });

                // Bottom: Search bar (RISS style - at the bottom)
                ui.allocate_ui(Vec2::new(ui.available_width(), 120.0), |ui| {
                    ui.add_space(8.0);
                    self.show_search_bar(ui, ctx);

                    // Status message
                    if let Some((msg, _)) = &self.status_message {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(msg)
                                    .color(Colors::TEXT_DIM)
                                    .font(FontId::proportional(11.0)),
                            );
                        });
                    }
                });
            });
    }
}

impl RissApp {
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        // Handle keyboard input
        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let up_pressed = ctx.input(|i| i.key_pressed(egui::Key::ArrowUp));
        let down_pressed = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown));
        let escape_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let tab_pressed = ctx.input(|i| i.key_pressed(egui::Key::Tab));

        if enter_pressed && !self.results.is_empty() {
            self.launch_app(self.selected_index);
        }

        if up_pressed && self.selected_index > 0 {
            self.selected_index -= 1;
        }

        if down_pressed && !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.results.len() - 1);
        }

        if escape_pressed && !self.query.is_empty() {
            self.query.clear();
            self.update_results();
        }

        if tab_pressed && !self.results.is_empty() {
            // Tab cycles through results
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }

    fn show_results(&mut self, ui: &mut egui::Ui) {
        // Show calculator result if applicable
        if !self.query.is_empty() {
            if let Some(calc_result) = search::try_calculate(&self.query) {
                let frame = egui::Frame::NONE
                    .fill(Colors::CALC_BG)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 8));

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🧮").font(FontId::proportional(20.0)));
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(&self.query)
                                .color(Colors::TEXT_DIM)
                                .font(FontId::proportional(14.0)),
                        );
                        ui.label(
                            RichText::new(&calc_result)
                                .color(Colors::CALC_TEXT)
                                .font(FontId::proportional(18.0))
                                .strong(),
                        );
                    });
                });
                ui.add_space(8.0);
            }
        }

        if self.results.is_empty() {
            ui.add_space(40.0);
            ui.centered_and_justified(|ui| {
                if self.query.is_empty() {
                    ui.label(
                        RichText::new("Type to search apps...\n\nFavorites and frequently used apps will appear here.")
                            .color(Colors::TEXT_DIM)
                            .font(FontId::proportional(14.0)),
                    );
                } else {
                    ui.label(
                        RichText::new(format!("No results for \"{}\"", self.query))
                            .color(Colors::TEXT_DIM)
                            .font(FontId::proportional(14.0)),
                    );
                }
            });
            return;
        }

        // Show section headers
        let show_favorites_header =
            self.query.is_empty() && self.results.iter().any(|r| r.entry.is_favorite);
        let show_frequent_header = self.query.is_empty()
            && self
                .results
                .iter()
                .any(|r| !r.entry.is_favorite && r.entry.launch_count > 0);

        let mut shown_favorites = false;
        let mut shown_frequent = false;

        // Use index-based loop to avoid borrow conflicts
        let result_count = self.results.len();
        for index in 0..result_count {
            // Check section headers
            let is_favorite = self.results[index].entry.is_favorite;
            let launch_count = self.results[index].entry.launch_count;

            // Section headers for default view
            if self.query.is_empty() {
                if is_favorite && !shown_favorites && show_favorites_header {
                    shown_favorites = true;
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("⭐ FAVORITES")
                            .color(Colors::TEXT_DIM)
                            .font(FontId::proportional(11.0))
                            .strong(),
                    );
                    ui.add_space(4.0);
                } else if !is_favorite
                    && launch_count > 0
                    && !shown_frequent
                    && show_frequent_header
                {
                    shown_frequent = true;
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("📊 FREQUENTLY USED")
                            .color(Colors::TEXT_DIM)
                            .font(FontId::proportional(11.0))
                            .strong(),
                    );
                    ui.add_space(4.0);
                }
            }

            // Clone the result to avoid borrow conflicts
            let result = self.results[index].clone();
            self.show_result_item(ui, index, &result);
        }
    }

    fn show_result_item(&mut self, ui: &mut egui::Ui, index: usize, result: &SearchResult) {
        let is_selected = index == self.selected_index;

        let bg_color = if is_selected {
            Colors::BG_HOVER
        } else {
            Colors::BG
        };

        let frame = egui::Frame::NONE
            .fill(bg_color)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .stroke(if is_selected {
                Stroke::new(1.0_f32, Colors::ACCENT_DIM)
            } else {
                Stroke::NONE
            });

        let response = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // App icon
                let icon_text = Self::get_app_icon(&result.entry);
                let icon_label =
                    ui.label(RichText::new(&icon_text).font(FontId::proportional(24.0)));
                let _ = icon_label;

                ui.add_space(8.0);

                // App info
                ui.vertical(|ui| {
                    // App name with match highlight
                    let name_color = if result.entry.is_favorite {
                        Colors::FAVORITE
                    } else if result.match_type == MatchType::Exact {
                        Colors::ACCENT
                    } else {
                        Colors::TEXT
                    };

                    ui.label(
                        RichText::new(&result.entry.name)
                            .color(name_color)
                            .font(FontId::proportional(15.0))
                            .strong(),
                    );

                    // Comment/description
                    if !result.entry.comment.is_empty() {
                        ui.label(
                            RichText::new(&result.entry.comment)
                                .color(Colors::TEXT_DIM)
                                .font(FontId::proportional(11.0)),
                        );
                    } else if !result.entry.categories.is_empty() {
                        ui.label(
                            RichText::new(result.entry.categories.join(" • "))
                                .color(Colors::TEXT_DIM)
                                .font(FontId::proportional(11.0)),
                        );
                    }

                    // Tags
                    if !result.entry.tags.is_empty() {
                        ui.horizontal(|ui| {
                            for tag in &result.entry.tags {
                                let tag_frame = egui::Frame::NONE
                                    .fill(Colors::BG_LIGHT)
                                    .corner_radius(CornerRadius::same(4))
                                    .inner_margin(egui::Margin::symmetric(4, 1));
                                tag_frame.show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("#{}", tag))
                                            .color(Colors::ACCENT_DIM)
                                            .font(FontId::proportional(9.0)),
                                    );
                                });
                            }
                        });
                    }
                });

                // Right side: launch count + favorite button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Favorite button
                    let fav_text = if result.entry.is_favorite {
                        "★"
                    } else {
                        "☆"
                    };
                    let fav_color = if result.entry.is_favorite {
                        Colors::FAVORITE
                    } else {
                        Colors::TEXT_DIM
                    };
                    let fav_btn = ui.add(
                        egui::Label::new(
                            RichText::new(fav_text)
                                .font(FontId::proportional(20.0))
                                .color(fav_color),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if fav_btn.clicked() {
                        self.toggle_favorite(index);
                    }

                    // Launch count
                    if result.entry.launch_count > 0 {
                        ui.label(
                            RichText::new(format!("×{}", result.entry.launch_count))
                                .color(Colors::TEXT_DIM)
                                .font(FontId::proportional(11.0)),
                        );
                    }

                    // Tag edit button
                    let tag_btn = ui.add(
                        egui::Label::new(
                            RichText::new("🏷️")
                                .font(FontId::proportional(14.0))
                                .color(Colors::TEXT_DIM),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if tag_btn.clicked() {
                        if self.editing_tags.as_deref() == Some(&result.entry.exec) {
                            self.editing_tags = None;
                        } else {
                            self.editing_tags = Some(result.entry.exec.clone());
                            self.tag_input = result.entry.tags.join(", ");
                        }
                    }
                });
            });
        });

        // Tag editing popup
        if self.editing_tags.as_deref() == Some(&result.entry.exec) {
            let exec = result.entry.exec.clone();
            let frame = egui::Frame::NONE
                .fill(Colors::BG_LIGHT)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::same(8))
                .stroke(Stroke::new(1.0_f32, Colors::BORDER));
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Tags:")
                            .color(Colors::TEXT_DIM)
                            .font(FontId::proportional(11.0)),
                    );
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut self.tag_input)
                            .desired_width(200.0)
                            .hint_text("tag1, tag2, ...")
                            .font(FontId::proportional(12.0)),
                    );
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Save")
                                    .font(FontId::proportional(11.0))
                                    .color(Colors::TEXT),
                            )
                            .fill(Colors::ACCENT_DIM)
                            .corner_radius(CornerRadius::same(4)),
                        )
                        .clicked()
                        || te.lost_focus()
                    {
                        let tags: Vec<String> = self
                            .tag_input
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        self.history.set_tags(&exec, tags.clone());
                        for app in &mut self.apps {
                            if app.exec == exec {
                                app.tags = tags.clone();
                            }
                        }
                        self.history.save();
                        self.editing_tags = None;
                        self.update_results();
                    }
                });
            });
        }

        // Click to launch
        if response.response.clicked() {
            self.launch_app(index);
        }

        // Hover to select
        if response.response.hovered() {
            self.selected_index = index;
        }

        ui.add_space(2.0);
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let frame = egui::Frame::NONE
            .fill(Colors::SEARCH_BG)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(12))
            .stroke(Stroke::new(1.0_f32, Colors::BORDER));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // Search icon
                ui.label(RichText::new("🔍").font(FontId::proportional(18.0)));
                ui.add_space(4.0);

                // Search input
                let search_response = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .desired_width(ui.available_width() - 60.0)
                        .hint_text("Search apps, calculate, or type a command...")
                        .font(FontId::proportional(16.0))
                        .text_color(Colors::TEXT),
                );

                if search_response.changed() {
                    self.update_results();
                }

                // Auto-focus
                if !search_response.has_focus() {
                    search_response.request_focus();
                }

                // Clear button
                if !self.query.is_empty()
                    && ui
                        .add(
                            egui::Button::new(
                                RichText::new("✕")
                                    .font(FontId::proportional(14.0))
                                    .color(Colors::TEXT_DIM),
                            )
                            .fill(Color32::TRANSPARENT),
                        )
                        .clicked()
                {
                    self.query.clear();
                    self.update_results();
                }
            });

            // Show keyboard hints
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("↑↓")
                        .color(Colors::ACCENT_DIM)
                        .font(FontId::proportional(10.0)),
                );
                ui.label(
                    RichText::new("navigate")
                        .color(Colors::TEXT_DIM)
                        .font(FontId::proportional(10.0)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("↵")
                        .color(Colors::ACCENT_DIM)
                        .font(FontId::proportional(10.0)),
                );
                ui.label(
                    RichText::new("launch")
                        .color(Colors::TEXT_DIM)
                        .font(FontId::proportional(10.0)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Esc")
                        .color(Colors::ACCENT_DIM)
                        .font(FontId::proportional(10.0)),
                );
                ui.label(
                    RichText::new("clear")
                        .color(Colors::TEXT_DIM)
                        .font(FontId::proportional(10.0)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Tab")
                        .color(Colors::ACCENT_DIM)
                        .font(FontId::proportional(10.0)),
                );
                ui.label(
                    RichText::new("cycle")
                        .color(Colors::TEXT_DIM)
                        .font(FontId::proportional(10.0)),
                );
            });
        });

        // Request repaint for animations
        if self.status_message.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
    }
}

/// Helper to avoid borrow issues
mod history {
    use crate::history::HistoryData;
    pub fn get_tags_safe(history: &HistoryData, exec: &str) -> Vec<String> {
        history.get_tags(exec)
    }
}
