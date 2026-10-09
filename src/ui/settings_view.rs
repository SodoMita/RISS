//! The settings screen.
//!
//! It mirrors the preference tree of KISS: every option of the original
//! launcher that makes sense on a desktop is exposed here, grouped in the same
//! categories, and the settings themselves can be searched (`Settings search`
//! in KISS) either from here or straight from the query bar.

use super::{Dialog, DialogAction};
use crate::history::HistoryData;
use crate::settings::{
    catalog, EdgePosition, GestureAction, HistoryMode, NightMode, ResultSize, SearchBarPosition,
    Settings, SettingsSection, TagSort, ThemeKind,
};
use crate::theme::Palette;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Color32, CornerRadius, RichText, Sense, Stroke, Vec2};

#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;

/// Mutable state of the settings screen.
pub struct SettingsUi {
    pub section: SettingsSection,
    pub filter: String,
    pub highlight: Option<String>,
    pub new_provider_name: String,
    pub new_provider_url: String,
    pub exclude_filter: String,
}

impl Default for SettingsUi {
    fn default() -> Self {
        Self {
            section: SettingsSection::Touch,
            filter: String::new(),
            highlight: None,
            new_provider_name: String::new(),
            new_provider_url: String::new(),
            exclude_filter: String::new(),
        }
    }
}

/// Things the settings screen asks the launcher to do.
pub enum SettingsAction {
    Close,
    Confirm(Dialog),
    AddProvider(String, String),
    RemoveProvider(usize),
    RestoreExcluded(String),
    RefreshApps,
}

const ACCENTS: [([u8; 4], &str); 8] = [
    ([137, 180, 250, 255], "Blue"),
    ([166, 227, 161, 255], "Green"),
    ([249, 226, 175, 255], "Sand"),
    ([243, 139, 168, 255], "Pink"),
    ([180, 190, 254, 255], "Lavender"),
    ([148, 226, 213, 255], "Teal"),
    ([247, 208, 166, 255], "Orange"),
    ([203, 166, 247, 255], "Purple"),
];

/// Draw the settings screen; returns the actions the launcher must perform.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut SettingsUi,
    settings: &mut Settings,
    history: &HistoryData,
    apps: &[AppEntry],
    palette: &Palette,
) -> Vec<SettingsAction> {
    let mut actions: Vec<SettingsAction> = Vec::new();
    let scale = settings.font_scale.clamp(0.6, 2.0);

    // --- header ---------------------------------------------------------
    ui.horizontal(|ui| {
        let _ = widgets::icon(ui, Icon::Settings, 18.0, palette.accent, palette);
        ui.add_space(6.0);
        let _ = ui.label(
            RichText::new("Settings")
                .size(16.0 * scale)
                .strong()
                .color(palette.text),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Close, 26.0, palette.text_dim, palette, "Close").clicked()
            {
                actions.push(SettingsAction::Close);
            }
            let response = ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .hint_text("Search settings")
                    .desired_width(150.0)
                    .font(egui::FontId::proportional(12.0 * scale)),
            );
            if response.changed() && state.filter.is_empty() {
                state.highlight = None;
            }
        });
    });
    widgets::divider(ui, palette, 8.0);

    let body_height = (ui.available_height() - 8.0).max(80.0);
    ui.horizontal_top(|ui| {
        let sidebar_width = 150.0;
        if state.filter.is_empty() {
            ui.allocate_ui(Vec2::new(sidebar_width, body_height), |ui| {
                render_sidebar(ui, state, palette);
            });
            widgets::divider(ui, palette, 0.0);
        }
        let content_width = (ui.available_width() - 8.0).max(120.0);
        ui.allocate_ui(Vec2::new(content_width, body_height), |ui| {
            if state.filter.is_empty() {
                render_section(ui, state, settings, history, apps, palette, &mut actions);
            } else {
                render_filtered(ui, state, settings, palette);
            }
        });
    });

    actions
}

fn render_sidebar(ui: &mut egui::Ui, state: &mut SettingsUi, palette: &Palette) {
    egui::ScrollArea::vertical()
        .id_salt("riss_settings_sidebar")
        .auto_shrink([false, false])
        .max_height(ui.available_height())
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for section in SettingsSection::ALL {
                let active = state.section == section && state.highlight.is_none();
                let text = RichText::new(section.title())
                    .size(12.0)
                    .color(if active { palette.accent } else { palette.text });
                let button = egui::Button::new(text)
                    .fill(if active { palette.selected } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .min_size(Vec2::new(138.0, 26.0));
                if ui.add(button).clicked() {
                    state.section = section;
                    state.highlight = None;
                }
                let _ = ui.add(
                    egui::Label::new(
                        RichText::new(section.summary())
                            .size(9.0)
                            .color(palette.text_dim),
                    )
                    .truncate(),
                );
                ui.add_space(4.0);
            }
        });
}

/// When a filter is active the screen lists every matching setting, exactly
/// like KISS' settings search.
fn render_filtered(
    ui: &mut egui::Ui,
    state: &mut SettingsUi,
    settings: &Settings,
    palette: &Palette,
) {
    let needle = state.filter.to_lowercase();
    let entries: Vec<_> = catalog(settings)
        .into_iter()
        .filter(|entry| {
            let haystack = format!(
                "{} {} {}",
                entry.title,
                entry.keywords,
                entry.section.title()
            )
            .to_lowercase();
            haystack.contains(&needle)
        })
        .collect();

    egui::ScrollArea::vertical()
        .id_salt("riss_settings_filter")
        .auto_shrink([false, false])
        .max_height(ui.available_height())
        .show(ui, |ui| {
            if entries.is_empty() {
                let _ = ui.label(
                    RichText::new("No setting matches this search")
                        .size(12.0)
                        .color(palette.text_dim),
                );
                return;
            }
            let _ = ui.label(
                RichText::new(format!("{} matching settings", entries.len()))
                    .size(11.0)
                    .color(palette.text_dim),
            );
            ui.add_space(4.0);
            for entry in &entries {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let _ = ui.label(
                            RichText::new(entry.title)
                                .size(12.0)
                                .color(palette.text),
                        );
                        let _ = ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                entry.section.title(),
                                if entry.value.is_empty() {
                                    "-"
                                } else {
                                    entry.value.as_str()
                                }
                            ))
                            .size(10.0)
                            .color(palette.text_dim),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let text = RichText::new("Show").size(11.0).color(palette.accent);
                        let button = egui::Button::new(text)
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, palette.border))
                            .min_size(Vec2::new(58.0, 22.0));
                        if ui.add(button).clicked() {
                            state.section = entry.section;
                            state.highlight = Some(entry.id.to_string());
                            state.filter.clear();
                        }
                    });
                });
                widgets::divider(ui, palette, 2.0);
            }
        });
}

fn render_section(
    ui: &mut egui::Ui,
    state: &mut SettingsUi,
    settings: &mut Settings,
    history: &HistoryData,
    apps: &[AppEntry],
    palette: &Palette,
    actions: &mut Vec<SettingsAction>,
) {
    egui::ScrollArea::vertical()
        .id_salt(egui::Id::new("riss_settings_body"))
        .auto_shrink([false, false])
        .max_height(ui.available_height())
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            match state.section {
                SettingsSection::Touch => section_touch(ui, state, settings, palette),
                SettingsSection::Interface => section_interface(ui, state, settings, palette),
                SettingsSection::Results => section_results(ui, state, settings, palette),
                SettingsSection::SearchBar => section_search_bar(ui, state, settings, palette),
                SettingsSection::Favorites => section_favorites(ui, state, settings, palette, actions),
                SettingsSection::History => section_history(ui, state, settings, palette, actions),
                SettingsSection::Tags => section_tags(ui, state, settings, palette, actions),
                SettingsSection::Providers => {
                    section_providers(ui, state, settings, palette, actions)
                }
                SettingsSection::Excluded => {
                    section_excluded(ui, state, history, apps, palette, actions)
                }
                SettingsSection::Backup => section_backup(ui, palette, actions),
                SettingsSection::Advanced => section_advanced(ui, palette, actions),
            }
        });
}

fn is_highlighted(state: &SettingsUi, id: &str) -> bool {
    state.highlight.as_deref() == Some(id)
}

fn row_frame(ui: &mut egui::Ui, palette: &Palette, highlighted: bool) {
    if highlighted {
        let height = ui.spacing().item_spacing.y;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        ui.painter().rect_filled(rect, 4.0, palette.selected);
    }
}

fn toggle_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    state: &SettingsUi,
    id: &str,
    label: &str,
    hint: &str,
    value: &mut bool,
) {
    let highlighted = is_highlighted(state, id);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let color = if highlighted { palette.accent } else { palette.text };
            let _ = ui.label(RichText::new(label).size(12.0).color(color));
            let _ = ui.label(
                egui::Label::new(RichText::new(hint).size(10.0).color(palette.text_dim)).truncate(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut checkbox = *value;
            if ui.add(egui::Checkbox::new(&mut checkbox, "")).changed() {
                *value = checkbox;
            }
        });
    });
    row_frame(ui, palette, highlighted);
}

fn slider_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    state: &SettingsUi,
    id: &str,
    label: &str,
    hint: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    let highlighted = is_highlighted(state, id);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let color = if highlighted { palette.accent } else { palette.text };
            let _ = ui.label(RichText::new(label).size(12.0).color(color));
            let _ = ui.label(
                egui::Label::new(RichText::new(hint).size(10.0).color(palette.text_dim)).truncate(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let _ = ui.add(
                egui::Slider::new(value, range)
                    .show_value(true)
                    .trailing_fill(true),
            );
        });
    });
    row_frame(ui, palette, highlighted);
}

fn choice_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    state: &SettingsUi,
    id: &str,
    label: &str,
    hint: &str,
    options: &[&str],
    current: usize,
) -> Option<usize> {
    let highlighted = is_highlighted(state, id);
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let color = if highlighted { palette.accent } else { palette.text };
            let _ = ui.label(RichText::new(label).size(12.0).color(color));
            let _ = ui.label(
                egui::Label::new(RichText::new(hint).size(10.0).color(palette.text_dim)).truncate(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (index, option) in options.iter().enumerate() {
                let selected = index == current;
                let text = RichText::new(*option)
                    .size(11.0)
                    .color(if selected { palette.accent } else { palette.text_dim });
                let button = egui::Button::new(text)
                    .fill(if selected { palette.selected } else { Color32::TRANSPARENT })
                    .stroke(Stroke::new(
                        1.0,
                        if selected { palette.accent } else { palette.border },
                    ))
                    .corner_radius(CornerRadius::same(6))
                    .min_size(Vec2::new(0.0, 24.0));
                if ui.add(button).clicked() {
                    chosen = Some(index);
                }
            }
        });
    });
    row_frame(ui, palette, highlighted);
    chosen
}

fn action_row(ui: &mut egui::Ui, label: &str, hint: &str, button: &str, palette: &Palette) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let _ = ui.label(RichText::new(label).size(12.0).color(palette.text));
            let _ = ui.label(
                egui::Label::new(RichText::new(hint).size(10.0).color(palette.text_dim)).truncate(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = RichText::new(button).size(11.0).color(palette.text);
            if ui
                .add(
                    egui::Button::new(text)
                        .fill(palette.surface_alt)
                        .stroke(Stroke::new(1.0, palette.border))
                        .corner_radius(CornerRadius::same(6))
                        .min_size(Vec2::new(84.0, 26.0)),
                )
                .clicked()
            {
                clicked = true;
            }
        });
    });
    clicked
}

fn group(ui: &mut egui::Ui, title: &str, palette: &Palette) {
    ui.add_space(6.0);
    widgets::section_title(ui, title, palette);
    widgets::divider(ui, palette, 2.0);
}

// --- sections --------------------------------------------------------------

fn section_touch(
    ui: &mut egui::Ui,
    state: &SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
) {
    group(ui, "Taps", palette);
    toggle_row(
        ui,
        palette,
        state,
        "long_press_menu",
        "Long press opens the menu",
        "Hold a result to open the context menu (right click works too)",
        &mut settings.long_press_menu,
    );
    let mut delay = settings.long_press_delay_ms as f32;
    slider_row(
        ui,
        palette,
        state,
        "long_press_delay_ms",
        "Long press duration",
        "Milliseconds before the menu opens",
        &mut delay,
        200.0..=1500.0,
    );
    settings.long_press_delay_ms = delay.clamp(200.0, 1500.0) as u32;
    toggle_row(
        ui,
        palette,
        state,
        "prevent_fast_launch",
        "Prevent fast launch",
        "First tap selects, the second one launches",
        &mut settings.prevent_fast_launch,
    );
    toggle_row(
        ui,
        palette,
        state,
        "double_click_launches",
        "Double click launches",
        "A single click only selects the result",
        &mut settings.double_click_launches,
    );
    let mut timeout = settings.tap_timeout_ms as f32;
    slider_row(
        ui,
        palette,
        state,
        "tap_timeout_ms",
        "Tap timeout",
        "How long the first tap stays valid",
        &mut timeout,
        400.0..=3000.0,
    );
    settings.tap_timeout_ms = timeout.clamp(400.0, 3000.0) as u32;
    toggle_row(
        ui,
        palette,
        state,
        "press_feedback",
        "Press feedback",
        "Animate rows while they are held",
        &mut settings.press_feedback,
    );
    let mut touch = settings.min_touch_height;
    slider_row(
        ui,
        palette,
        state,
        "min_touch_height",
        "Minimum touch height",
        "Keeps rows comfortable to tap",
        &mut touch,
        32.0..=72.0,
    );
    settings.min_touch_height = touch.clamp(32.0, 72.0);

    group(ui, "Favorites bar", palette);
    toggle_row(
        ui,
        palette,
        state,
        "single_tap_favorite_launches",
        "Single tap launches favorites",
        "Otherwise a tap only selects the favorite",
        &mut settings.single_tap_favorite_launches,
    );
    toggle_row(
        ui,
        palette,
        state,
        "number_keys_to_launch",
        "Number keys launch results",
        "1–9 launch the matching row when the query is empty",
        &mut settings.number_keys_to_launch,
    );
    toggle_row(
        ui,
        palette,
        state,
        "hide_on_launch",
        "Hide after launching",
        "The launcher returns to the background once an app starts",
        &mut settings.hide_on_launch,
    );

    group(ui, "Gestures (Ctrl+Alt)", palette);
    let gestures = [
        ("gesture_up", "Gesture: up", GestureAction::ALL),
        ("gesture_down", "Gesture: down", GestureAction::ALL),
        ("gesture_left", "Gesture: left", GestureAction::ALL),
        ("gesture_right", "Gesture: right", GestureAction::ALL),
        (
            "gesture_long_press",
            "Gesture: long press (Ctrl+Alt+L)",
            GestureAction::ALL,
        ),
    ];
    for (id, label, options) in gestures {
        let labels: Vec<&str> = options.iter().map(|action| action.label()).collect();
        let current = match id {
            "gesture_up" => options.iter().position(|a| *a == settings.gesture_up),
            "gesture_down" => options.iter().position(|a| *a == settings.gesture_down),
            "gesture_left" => options.iter().position(|a| *a == settings.gesture_left),
            "gesture_right" => options.iter().position(|a| *a == settings.gesture_right),
            _ => options.iter().position(|a| *a == settings.gesture_long_press),
        }
        .unwrap_or(0);
        if let Some(index) = choice_row(ui, palette, state, id, label, "", &labels, current) {
            let action = options[index];
            match id {
                "gesture_up" => settings.gesture_up = action,
                "gesture_down" => settings.gesture_down = action,
                "gesture_left" => settings.gesture_left = action,
                "gesture_right" => settings.gesture_right = action,
                _ => settings.gesture_long_press = action,
            }
        }
    }
}

fn section_interface(
    ui: &mut egui::Ui,
    state: &SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
) {
    group(ui, "Colors", palette);
    let themes = ThemeKind::ALL;
    let labels: Vec<&str> = themes.iter().map(|theme| theme.label()).collect();
    let current = themes
        .iter()
        .position(|theme| *theme == settings.theme)
        .unwrap_or(0);
    if let Some(index) = choice_row(ui, palette, state, "theme", "Theme", "", &labels, current) {
        settings.theme = themes[index];
    }
    let nights = NightMode::ALL;
    let labels: Vec<&str> = nights.iter().map(|mode| mode.label()).collect();
    let current = nights
        .iter()
        .position(|mode| *mode == settings.night_mode)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "night_mode",
        "Night mode",
        "Force the light or the dark palette",
        &labels,
        current,
    ) {
        settings.night_mode = nights[index];
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let _ = ui.label(RichText::new("Main color").size(12.0).color(palette.text));
            let _ = ui.label(
                egui::Label::new(
                    RichText::new("Used for highlights, selection and the info bar")
                        .size(10.0)
                        .color(palette.text_dim),
                )
                .truncate(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (color, name) in ACCENTS.iter().rev() {
                let selected = settings.primary_color == *color;
                let (rect, response) = ui.allocate_exact_size(Vec2::s(22.0), Sense::click());
                if selected {
                    ui.painter().rect_filled(rect, 7.0, palette.text_dim);
                }
                let inner = rect.shrink(2.0);
                ui.painter().rect_filled(
                    inner,
                    5.0,
                    Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
                );
                if response.clicked() {
                    settings.primary_color = *color;
                }
                response.on_hover_text(*name);
            }
        });
    });

    group(ui, "Shape", palette);
    toggle_row(
        ui,
        palette,
        state,
        "rounded_list",
        "Rounded results",
        "Round the corners of the list items",
        &mut settings.rounded_list,
    );
    toggle_row(
        ui,
        palette,
        state,
        "rounded_bars",
        "Rounded bars",
        "Round the search and favorites bars",
        &mut settings.rounded_bars,
    );
    toggle_row(
        ui,
        palette,
        state,
        "large_result_list_margins",
        "Large result margins",
        "More breathing room between results",
        &mut settings.large_result_list_margins,
    );
    toggle_row(
        ui,
        palette,
        state,
        "show_separators",
        "Show separators",
        "A thin line between the results",
        &mut settings.show_separators,
    );
    let mut spacing = settings.widget_spacing;
    slider_row(
        ui,
        palette,
        state,
        "widget_spacing",
        "Widget spacing",
        "Extra space between the rows",
        &mut spacing,
        0.0..=16.0,
    );
    settings.widget_spacing = spacing.clamp(0.0, 16.0);
    let mut scale = settings.font_scale;
    slider_row(
        ui,
        palette,
        state,
        "font_scale",
        "Font scale",
        "Scales every text of the launcher",
        &mut scale,
        0.7..=1.6,
    );
    settings.font_scale = scale.clamp(0.7, 1.6);

    group(ui, "Window", palette);
    toggle_row(
        ui,
        palette,
        state,
        "fullscreen",
        "Fullscreen",
        "Use the whole screen",
        &mut settings.fullscreen,
    );
    toggle_row(
        ui,
        palette,
        state,
        "force_portrait",
        "Force portrait",
        "Keep a tall window, like a phone",
        &mut settings.force_portrait,
    );
    let mut width = settings.window_width;
    slider_row(
        ui,
        palette,
        state,
        "window_width",
        "Window width",
        "Applied on the next frame",
        &mut width,
        320.0..=1200.0,
    );
    settings.window_width = width.clamp(320.0, 1200.0);
    let mut height = settings.window_height;
    slider_row(
        ui,
        palette,
        state,
        "window_height",
        "Window height",
        "Applied on the next frame",
        &mut height,
        360.0..=1400.0,
    );
    settings.window_height = height.clamp(360.0, 1400.0);
}

fn section_results(
    ui: &mut egui::Ui,
    state: &SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
) {
    group(ui, "Layout", palette);
    let sizes = ResultSize::ALL;
    let labels: Vec<&str> = sizes.iter().map(|size| size.label()).collect();
    let current = sizes
        .iter()
        .position(|size| *size == settings.result_size)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "result_size",
        "Results size",
        "Base size of a row",
        &labels,
        current,
    ) {
        settings.result_size = sizes[index];
    }
    toggle_row(
        ui,
        palette,
        state,
        "adaptive_results",
        "Adaptive results",
        "Show fewer, bigger results when there are many",
        &mut settings.adaptive_results,
    );
    toggle_row(
        ui,
        palette,
        state,
        "adaptive_icon_size",
        "Adaptive icon size",
        "Icons grow when there are only a few results",
        &mut settings.adaptive_icon_size,
    );
    toggle_row(
        ui,
        palette,
        state,
        "adaptive_columns",
        "Adaptive columns",
        "Switch to a grid when the results are few",
        &mut settings.adaptive_columns,
    );
    let mut columns = settings.max_columns as f32;
    slider_row(
        ui,
        palette,
        state,
        "max_columns",
        "Maximum columns",
        "Upper bound of the grid",
        &mut columns,
        1.0..=8.0,
    );
    settings.max_columns = columns.clamp(1.0, 8.0) as usize;
    let edges = EdgePosition::ALL;
    let labels: Vec<&str> = edges.iter().map(|edge| edge.label()).collect();
    let current = edges
        .iter()
        .position(|edge| *edge == settings.grid_position)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "grid_position",
        "Numbers and actions",
        "Side of the row numbers and buttons",
        &labels,
        current,
    ) {
        settings.grid_position = edges[index];
    }

    group(ui, "Content", palette);
    toggle_row(
        ui,
        palette,
        state,
        "show_app_names",
        "Display app names",
        "Names and numbers of the results",
        &mut settings.show_app_names,
    );
    toggle_row(
        ui,
        palette,
        state,
        "show_subicons",
        "Display sub icons",
        "Comments and categories under the name",
        &mut settings.show_subicons,
    );
    toggle_row(
        ui,
        palette,
        state,
        "hide_main_icons",
        "Hide main icons",
        "Text only results",
        &mut settings.hide_main_icons,
    );
    toggle_row(
        ui,
        palette,
        state,
        "show_tags",
        "Display tags",
        "Show the tags of an application",
        &mut settings.show_tags,
    );
    toggle_row(
        ui,
        palette,
        state,
        "show_launch_count",
        "Display launch counters",
        "How often an application was started",
        &mut settings.show_launch_count,
    );
    toggle_row(
        ui,
        palette,
        state,
        "show_row_actions",
        "Row action buttons",
        "Favorite and tags buttons inside each row",
        &mut settings.show_row_actions,
    );
    toggle_row(
        ui,
        palette,
        state,
        "select_last_result",
        "Select the last result",
        "The closest result is ready for ↵",
        &mut settings.select_last_result,
    );
}

fn section_search_bar(
    ui: &mut egui::Ui,
    state: &SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
) {
    group(ui, "Position", palette);
    let positions = SearchBarPosition::ALL;
    let labels: Vec<&str> = positions.iter().map(|item| item.label()).collect();
    let current = positions
        .iter()
        .position(|item| *item == settings.search_bar_position)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "search_bar_position",
        "Search bar position",
        "Where the query is typed",
        &labels,
        current,
    ) {
        settings.search_bar_position = positions[index];
    }
    let bars = crate::settings::BarPosition::ALL;
    let labels: Vec<&str> = bars.iter().map(|item| item.label()).collect();
    let current = bars
        .iter()
        .position(|item| *item == settings.info_bar_position)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "info_bar_position",
        "Info bar position",
        "Counters and shortcuts",
        &labels,
        current,
    ) {
        settings.info_bar_position = bars[index];
    }

    group(ui, "Appearance", palette);
    toggle_row(
        ui,
        palette,
        state,
        "large_search_bar",
        "Large search bar",
        "A taller query field",
        &mut settings.large_search_bar,
    );
    toggle_row(
        ui,
        palette,
        state,
        "hide_search_bar_hint",
        "Hide search bar hint",
        "Start with an empty placeholder",
        &mut settings.hide_search_bar_hint,
    );
    toggle_row(
        ui,
        palette,
        state,
        "show_keyboard_hints",
        "Show keyboard hints",
        "The legend under the search bar",
        &mut settings.show_keyboard_hints,
    );
    toggle_row(
        ui,
        palette,
        state,
        "transparent_search_bar",
        "Transparent search bar",
        "Let the results show through",
        &mut settings.transparent_search_bar,
    );
    toggle_row(
        ui,
        palette,
        state,
        "swap_kiss_button_with_menu",
        "Swap launcher and menu buttons",
        "Show the menu on the left of the info bar",
        &mut settings.swap_kiss_button_with_menu,
    );
}

fn section_favorites(
    ui: &mut egui::Ui,
    state: &SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
    actions: &mut Vec<SettingsAction>,
) {
    group(ui, "Bar", palette);
    toggle_row(
        ui,
        palette,
        state,
        "favorites_bar",
        "Favorites bar",
        "Quick access to your favorites",
        &mut settings.favorites_bar,
    );
    toggle_row(
        ui,
        palette,
        state,
        "large_favorites_bar",
        "Large favorites bar",
        "Bigger buttons with names",
        &mut settings.large_favorites_bar,
    );
    toggle_row(
        ui,
        palette,
        state,
        "transparent_favorites_bar",
        "Transparent favorites bar",
        "Let the results show through",
        &mut settings.transparent_favorites_bar,
    );
    let bars = crate::settings::BarPosition::ALL;
    let labels: Vec<&str> = bars.iter().map(|item| item.label()).collect();
    let current = bars
        .iter()
        .position(|item| *item == settings.favorites_bar_position)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "favorites_bar_position",
        "Favorites bar position",
        "Above or below the results",
        &labels,
        current,
    ) {
        settings.favorites_bar_position = bars[index];
    }
    let mut limit = settings.favorites_limit as f32;
    slider_row(
        ui,
        palette,
        state,
        "favorites_limit",
        "Favorites bar capacity",
        "How many favorites are shown",
        &mut limit,
        2.0..=32.0,
    );
    settings.favorites_limit = limit.clamp(2.0, 32.0) as usize;

    group(ui, "Filtering", palette);
    toggle_row(
        ui,
        palette,
        state,
        "exclude_favorites_from_apps",
        "Exclude favorites from apps",
        "Favorites only live in the bar",
        &mut settings.exclude_favorites_from_apps,
    );
    toggle_row(
        ui,
        palette,
        state,
        "exclude_favorites_from_history",
        "Exclude favorites from history",
        "Favorites are not part of the history list",
        &mut settings.exclude_favorites_from_history,
    );

    let mut tags = settings.favorites_tags.join(", ");
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let _ = ui.label(RichText::new("Favorites tags").size(12.0).color(palette.text));
            let _ = ui.label(
                egui::Label::new(
                    RichText::new("Tags automatically given to favorites")
                        .size(10.0)
                        .color(palette.text_dim),
                )
                .truncate(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let _ = ui.add(
                egui::TextEdit::singleline(&mut tags)
                    .desired_width(150.0)
                    .hint_text("favorite"),
            );
        });
    });
    settings.favorites_tags = tags
        .split(',')
        .map(|tag| tag.trim().trim_start_matches('#').to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();

    group(ui, "Reset", palette);
    if action_row(
        ui,
        "Reset favorites",
        "Forget every favorite and shortcut",
        "Reset",
        palette,
    ) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Reset favorites?".to_string(),
            message: "Every favorite and number shortcut will be removed.".to_string(),
            confirm: "Reset".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::ResetFavorites,
        }));
    }
}

fn section_history(
    ui: &mut egui::Ui,
    state: &SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
    actions: &mut Vec<SettingsAction>,
) {
    group(ui, "Ranking", palette);
    let modes = HistoryMode::ALL;
    let labels: Vec<&str> = modes.iter().map(|mode| mode.label()).collect();
    let current = modes
        .iter()
        .position(|mode| *mode == settings.history_mode)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "history_mode",
        "History mode",
        "How the default list is ranked",
        &labels,
        current,
    ) {
        settings.history_mode = modes[index];
    }
    let mut length = settings.history_length as f32;
    slider_row(
        ui,
        palette,
        state,
        "history_length",
        "Number of displayed items",
        "Upper bound of the result list",
        &mut length,
        1.0..=150.0,
    );
    settings.history_length = length.clamp(1.0, 150.0) as usize;

    group(ui, "Recording", palette);
    toggle_row(
        ui,
        palette,
        state,
        "enable_app_history",
        "Track app history",
        "Count the launches to rank applications",
        &mut settings.enable_app_history,
    );
    toggle_row(
        ui,
        palette,
        state,
        "freeze_history",
        "Freeze history",
        "Stop recording anything",
        &mut settings.freeze_history,
    );
    toggle_row(
        ui,
        palette,
        state,
        "search_through_history",
        "Search through history",
        "Offer the searches you already made",
        &mut settings.search_through_history,
    );

    group(ui, "Reset", palette);
    if action_row(
        ui,
        "Reset history",
        "Forget launch counts, searches and commands",
        "Reset",
        palette,
    ) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Reset history?".to_string(),
            message: "Launch counters and searches will be erased.".to_string(),
            confirm: "Reset".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::ResetHistory,
        }));
    }
}

fn section_tags(
    ui: &mut egui::Ui,
    state: &SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
    actions: &mut Vec<SettingsAction>,
) {
    group(ui, "Tags", palette);
    toggle_row(
        ui,
        palette,
        state,
        "tags_visible",
        "Tags are visible",
        "Show the tags inside the results",
        &mut settings.tags_visible,
    );
    toggle_row(
        ui,
        palette,
        state,
        "tags_menu",
        "Tags menu",
        "A row of tags to filter the results",
        &mut settings.tags_menu,
    );
    toggle_row(
        ui,
        palette,
        state,
        "show_untagged",
        "Show untagged",
        "Applications without a tag can be searched",
        &mut settings.show_untagged,
    );
    let sorts = TagSort::ALL;
    let labels: Vec<&str> = sorts.iter().map(|sort| sort.label()).collect();
    let current = sorts
        .iter()
        .position(|sort| *sort == settings.tagged_sort)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "tagged_sort",
        "Tagged results sort mode",
        "Order of the filtered results",
        &labels,
        current,
    ) {
        settings.tagged_sort = sorts[index];
    }

    group(ui, "Reset", palette);
    if action_row(ui, "Reset tags", "Forget every custom tag", "Reset", palette) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Reset tags?".to_string(),
            message: "Every tag you created will be removed.".to_string(),
            confirm: "Reset".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::ResetTags,
        }));
    }
}

fn section_providers(
    ui: &mut egui::Ui,
    state: &mut SettingsUi,
    settings: &mut Settings,
    palette: &Palette,
    actions: &mut Vec<SettingsAction>,
) {
    group(ui, "Enabled providers", palette);
    toggle_row(
        ui,
        palette,
        state,
        "provider_settings",
        "Enable settings search",
        "Find a setting by typing its name",
        &mut settings.provider_settings,
    );
    toggle_row(
        ui,
        palette,
        state,
        "provider_excluded",
        "Enable excluded apps search",
        "Excluded applications stay findable",
        &mut settings.provider_excluded,
    );
    toggle_row(
        ui,
        palette,
        state,
        "provider_web",
        "Enable web search",
        "Offer a web search for every query",
        &mut settings.provider_web,
    );
    toggle_row(
        ui,
        palette,
        state,
        "provider_exec",
        "Enable command execution",
        "Run a shell command from the query bar",
        &mut settings.provider_exec,
    );
    toggle_row(
        ui,
        palette,
        state,
        "provider_timer",
        "Enable timer",
        "“sleep 5m” starts a countdown",
        &mut settings.provider_timer,
    );
    toggle_row(
        ui,
        palette,
        state,
        "provider_calc",
        "Enable calculator",
        "Evaluate expressions from the query bar",
        &mut settings.provider_calc,
    );
    toggle_row(
        ui,
        palette,
        state,
        "always_web_search_on_enter",
        "Always default web search on enter",
        "↵ searches the web even when an app matches",
        &mut settings.always_web_search_on_enter,
    );

    group(ui, "Search", palette);
    let providers = settings.all_providers();
    let labels: Vec<&str> = providers.iter().map(|item| item.name.as_str()).collect();
    let current = providers
        .iter()
        .position(|item| item.name == settings.default_web_provider)
        .unwrap_or(0);
    if let Some(index) = choice_row(
        ui,
        palette,
        state,
        "default_web_provider",
        "Default search provider",
        "Used when a query has no application match",
        &labels,
        current,
    ) {
        if let Some(provider) = providers.get(index) {
            settings.default_web_provider = provider.name.clone();
        }
    }
    let mut precision = settings.min_match_precision as f32;
    slider_row(
        ui,
        palette,
        state,
        "min_match_precision",
        "Min match precision",
        "1 accepts loose matches, 6 only perfect ones",
        &mut precision,
        1.0..=6.0,
    );
    settings.min_match_precision = precision.clamp(1.0, 6.0) as u8;
    toggle_row(
        ui,
        palette,
        state,
        "legacy_fuzzy",
        "Use legacy fuzzy search",
        "The simple matcher used by old KISS versions",
        &mut settings.legacy_fuzzy,
    );

    group(ui, "Custom providers", palette);
    let custom = settings.custom_web_providers.clone();
    for (index, provider) in custom.iter().enumerate() {
        ui.horizontal(|ui| {
            let _ = ui.label(
                RichText::new(&provider.name)
                    .size(12.0)
                    .color(palette.text),
            );
            let _ = ui.label(
                egui::Label::new(
                    RichText::new(&provider.url)
                        .size(10.0)
                        .color(palette.text_dim),
                )
                .truncate(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::Trash, 22.0, palette.danger, palette, "Remove")
                    .clicked()
                {
                    actions.push(SettingsAction::RemoveProvider(index));
                }
            });
        });
    }
    ui.horizontal(|ui| {
        let _ = ui.add(
            egui::TextEdit::singleline(&mut state.new_provider_name)
                .hint_text("Name")
                .desired_width(80.0),
        );
        let _ = ui.add(
            egui::TextEdit::singleline(&mut state.new_provider_url)
                .hint_text("https://…?q={}")
                .desired_width(160.0),
        );
        let name = state.new_provider_name.trim().to_string();
        let url = state.new_provider_url.trim().to_string();
        let ready = !name.is_empty() && (url.contains('{') || url.starts_with("http"));
        if ui
            .add_enabled(
                ready,
                egui::Button::new(RichText::new("Add").size(11.0).color(palette.text)),
            )
            .clicked()
        {
            actions.push(SettingsAction::AddProvider(name, url));
            state.new_provider_name.clear();
            state.new_provider_url.clear();
        }
    });
    let _ = ui.label(
        egui::Label::new(
            RichText::new("Use {} as the placeholder of the query.")
                .size(10.0)
                .color(palette.text_dim),
        )
        .truncate(),
    );
}

fn section_excluded(
    ui: &mut egui::Ui,
    state: &mut SettingsUi,
    history: &HistoryData,
    apps: &[AppEntry],
    palette: &Palette,
    actions: &mut Vec<SettingsAction>,
) {
    group(ui, "Excluded applications", palette);
    let _ = ui.label(
        egui::Label::new(
            RichText::new("Long press a result and pick “Exclude application”.")
                .size(10.0)
                .color(palette.text_dim),
        )
        .truncate(),
    );
    let _ = ui.add(
        egui::TextEdit::singleline(&mut state.exclude_filter)
            .hint_text("Filter")
            .desired_width(160.0),
    );
    ui.add_space(4.0);

    let filter = state.exclude_filter.to_lowercase();
    let mut count = 0;
    for exec in &history.excluded {
        let name = apps
            .iter()
            .find(|app| &app.exec == exec)
            .map(|app| app.name.clone())
            .unwrap_or_else(|| exec.clone());
        if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
            continue;
        }
        count += 1;
        ui.horizontal(|ui| {
            let _ = ui.label(RichText::new(&name).size(12.0).color(palette.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::Refresh, 22.0, palette.accent, palette, "Restore")
                    .clicked()
                {
                    actions.push(SettingsAction::RestoreExcluded(exec.clone()));
                }
            });
        });
    }
    if count == 0 {
        let _ = ui.label(
            RichText::new("Nothing is excluded.")
                .size(11.0)
                .color(palette.text_dim),
        );
    }

    group(ui, "Reset", palette);
    if action_row(
        ui,
        "Reset exclusion lists",
        "Every application becomes searchable again",
        "Reset",
        palette,
    ) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Reset exclusion lists?".to_string(),
            message: "Excluded and untracked applications will come back.".to_string(),
            confirm: "Reset".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::ResetExcluded,
        }));
    }
}

fn section_backup(ui: &mut egui::Ui, palette: &Palette, actions: &mut Vec<SettingsAction>) {
    group(ui, "Export", palette);
    let _ = ui.label(
        egui::Label::new(
            RichText::new("Writes settings and history to riss-backup.json in your config folder.")
                .size(10.0)
                .color(palette.text_dim),
        )
        .truncate(),
    );
    if action_row(ui, "Export everything", "Save a backup", "Export", palette) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Export data?".to_string(),
            message: "Settings and history will be written next to the configuration file."
                .to_string(),
            confirm: "Export".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::Export,
        }));
    }
    if action_row(ui, "Import everything", "Restore the last backup", "Import", palette) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Import data?".to_string(),
            message: "The current settings and history will be replaced.".to_string(),
            confirm: "Import".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::Import,
        }));
    }
}

fn section_advanced(ui: &mut egui::Ui, palette: &Palette, actions: &mut Vec<SettingsAction>) {
    group(ui, "Applications", palette);
    if action_row(
        ui,
        "Reload applications",
        "Rescan the desktop entries of the system",
        "Reload",
        palette,
    ) {
        actions.push(SettingsAction::RefreshApps);
    }

    group(ui, "Shortcuts", palette);
    if action_row(
        ui,
        "Reset shortcuts",
        "Freeze every number shortcut",
        "Reset",
        palette,
    ) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Reset shortcuts?".to_string(),
            message: "Every number shortcut will be removed.".to_string(),
            confirm: "Reset".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::ResetShortcuts,
        }));
    }

    group(ui, "Danger zone", palette);
    if action_row(ui, "Reset everything", "Wipe all launcher data", "Reset", palette) {
        actions.push(SettingsAction::Confirm(Dialog {
            title: "Reset everything?".to_string(),
            message: "History, favorites, tags and exclusions will be erased.".to_string(),
            confirm: "Reset".to_string(),
            cancel: "Cancel".to_string(),
            action: DialogAction::ResetAll,
        }));
    }

    group(ui, "About", palette);
    let _ = ui.label(
        RichText::new("RISS Launcher — a KISS inspired launcher for the desktop")
            .size(11.0)
            .color(palette.text_dim),
    );
    let _ = ui.label(
        RichText::new(format!(
            "Configuration: {}",
            crate::settings::config_dir().display()
        ))
        .size(10.0)
        .color(palette.text_dim),
    );
}
