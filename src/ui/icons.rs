use eframe::egui::{self, Color32, Stroke, Vec2};

/// Bottom-bar icons matching KISS for Android (`main.xml`).
///
/// * `Kiss` — the hollow circle from `ic_launcher_white.xml`, tinted with the
///   primary/accent color. Opens the app list.
/// * `Menu` — the three vertical dots from `dots.xml`, tinted with the search
///   (text) color. Opens settings.
/// * `Clear` — the X from `ic_close.xml`, tinted with the search color.
///   Clears the query.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BarIcon {
    Kiss,
    Menu,
    Clear,
}

/// KISS-style icon button: a 44x44 touch target with a vector glyph painted
/// via the egui painter instead of a font-dependent unicode character.
///
/// `icon_color` is the glyph color (accent for [`BarIcon::Kiss`], text color
/// otherwise) and `hover_bg` is the circle fill shown on hover/press.
/// `show_glyph` implements KISS `pref-hide-circle`: the touch target stays
/// clickable, only the glyph is hidden.
pub(super) fn bar_icon_button(
    ui: &mut egui::Ui,
    icon: BarIcon,
    icon_color: Color32,
    hover_bg: Color32,
    tooltip: &str,
    show_glyph: bool,
) -> egui::Response {
    let size = Vec2::splat(44.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let response = response
        .on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = rect.center();

        // KISS uses a ripple (`selectableItemBackground`); approximate it with
        // a soft circle behind the glyph.
        if response.is_pointer_button_down_on() {
            painter.circle_filled(center, 20.0, icon_color.gamma_multiply(0.25));
        } else if response.hovered() {
            painter.circle_filled(center, 20.0, hover_bg);
        }

        if show_glyph {
            match icon {
                BarIcon::Kiss => {
                    // Hollow ring like `ic_launcher_white.xml`: stroke is ~14%
                    // of the 24dp glyph, i.e. ~3.3px here.
                    painter.circle_stroke(center, 8.5, Stroke::new(3.2_f32, icon_color));
                }
                BarIcon::Menu => {
                    // Three vertical dots like `dots.xml` (centers 6dp apart,
                    // radius 2dp on a 24dp viewport).
                    for dy in [-6.5, 0.0, 6.5] {
                        painter.circle_filled(center + Vec2::new(0.0, dy), 2.3, icon_color);
                    }
                }
                BarIcon::Clear => {
                    // X like `ic_close.xml`: spans ~58% of the 24dp viewport.
                    let half = 7.0;
                    let stroke = Stroke::new(2.6_f32, icon_color);
                    painter.line_segment(
                        [
                            center + Vec2::new(-half, -half),
                            center + Vec2::new(half, half),
                        ],
                        stroke,
                    );
                    painter.line_segment(
                        [
                            center + Vec2::new(-half, half),
                            center + Vec2::new(half, -half),
                        ],
                        stroke,
                    );
                }
            }
        }
    }

    response
}
