//! Small, dependency free widgets: hand drawn vector icons, application
//! badges and the buttons used by the launcher and by the settings screen.
//!
//! KISS relies on the platform font for its icons. Rather than depending on an
//! emoji font (which is not always installed), every glyph here is drawn with
//! the egui painter, so the launcher looks identical everywhere and stays crisp
//! at any size.

use super::colors::{badge_color, Palette};
#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2,
};

/// Every icon used by the launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Icon {
    Search,
    Close,
    Refresh,
    Settings,
    Star,
    StarFilled,
    Tag,
    History,
    Grid,
    Apps,
    List,
    ChevronRight,
    ChevronLeft,
    Trash,
    Copy,
    Check,
    Clock,
    Terminal,
    Folder,
    Lock,
    Info,
    Sliders,
    Kebab,
    Pin,
    Pencil,
    Hash,
    Globe,
    Calculator,
    Sun,
    Moon,
    Filter,
    Power,
    Keyboard,
    Home,
}

/// Draw an icon centred in an already allocated `size × size` area.
pub fn draw_icon(ui: &mut Ui, icon: Icon, center: Pos2, size: f32, color: Color32, bg: Color32) {
    let painter = ui.painter();
    let s = size.max(4.0);
    let r = s * 0.5;
    let thin: f32 = (s * 0.09).clamp(1.0f32, 2.2f32);
    let stroke = Stroke::new(thin, color);

    match icon {
        Icon::Close => {
            let d = r * 0.62;
            painter.line_segment(
                [center + Vec2::new(-d, -d), center + Vec2::new(d, d)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-d, d), center + Vec2::new(d, -d)],
                stroke,
            );
        }
        Icon::Check => {
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.6, r * 0.05),
                    center + Vec2::new(-r * 0.15, r * 0.5),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.15, r * 0.5),
                    center + Vec2::new(r * 0.65, -r * 0.5),
                ],
                stroke,
            );
        }
        Icon::Search => {
            let ring_r = r * 0.6;
            let ring_center = center - Vec2::new(r * 0.12, r * 0.12);
            painter.circle_stroke(ring_center, ring_r, stroke);
            painter.line_segment(
                [
                    ring_center + Vec2::new(ring_r * 0.7, ring_r * 0.7),
                    center + Vec2::new(r * 0.62, r * 0.62),
                ],
                stroke,
            );
        }
        Icon::Refresh => {
            let mut points = Vec::new();
            for i in 0..=12 {
                let angle = 0.6 + (i as f32 / 12.0) * 4.6;
                points.push(center + Vec2::new(angle.cos(), angle.sin()) * (r * 0.78));
            }
            let tip = points[12];
            painter.add(Shape::line(points, stroke));
            painter.add(Shape::convex_polygon(
                vec![
                    tip + Vec2::new(r * 0.30, 0.0),
                    tip + Vec2::new(-r * 0.10, r * 0.26),
                    tip + Vec2::new(-r * 0.06, -r * 0.24),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Settings => {
            painter.circle_stroke(center, r * 0.44, stroke);
            for i in 0..6 {
                let angle = i as f32 * std::f32::consts::PI / 3.0;
                let dir = Vec2::new(angle.cos(), angle.sin());
                painter.line_segment([center + dir * r * 0.58, center + dir * r * 0.95], stroke);
            }
        }
        Icon::Star | Icon::StarFilled => {
            let outer = star_points(center, r * 0.95, r * 0.42);
            if icon == Icon::StarFilled {
                painter.add(Shape::convex_polygon(
                    outer,
                    color,
                    Stroke::new(1.0f32, color),
                ));
            } else {
                painter.add(Shape::convex_polygon(outer, bg, stroke));
            }
        }
        Icon::Tag => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-r * 0.85, -r * 0.85),
                    center + Vec2::new(r * 0.15, -r * 0.85),
                    center + Vec2::new(r * 0.85, -r * 0.1),
                    center + Vec2::new(r * 0.85, r * 0.85),
                    center + Vec2::new(-r * 0.85, r * 0.85),
                ],
                color,
                Stroke::NONE,
            ));
            painter.circle_filled(center + Vec2::new(-r * 0.35, -r * 0.35), r * 0.18, bg);
        }
        Icon::History => {
            painter.circle_stroke(center, r * 0.8, stroke);
            painter.line_segment([center, center + Vec2::new(0.0, -r * 0.45)], stroke);
            painter.line_segment([center, center + Vec2::new(r * 0.4, r * 0.1)], stroke);
        }
        Icon::Clock => {
            painter.circle_stroke(center, r * 0.85, stroke);
            painter.line_segment([center, center + Vec2::new(0.0, -r * 0.5)], stroke);
            painter.line_segment([center, center + Vec2::new(r * 0.42, r * 0.2)], stroke);
        }
        Icon::Grid => {
            let cell = r * 0.78;
            let gap = r * 0.28;
            let origin = center - Vec2::new(cell, cell) - Vec2::new(gap * 0.5, gap * 0.5);
            for row in 0..2 {
                for col in 0..2 {
                    let at = origin
                        + Vec2::new((col as f32) * (cell + gap), (row as f32) * (cell + gap));
                    painter.rect_filled(Rect::from_min_size(at, Vec2::splat(cell)), 2.0, color);
                }
            }
        }
        Icon::List => {
            let dot = r * 0.18;
            for i in 0..3 {
                let y = center.y + (i as f32 - 1.0) * r * 0.6;
                painter.circle_filled(center + Vec2::new(-r * 0.7, y), dot, color);
                painter.line_segment(
                    [
                        center + Vec2::new(-r * 0.35, y),
                        center + Vec2::new(r * 0.75, y),
                    ],
                    stroke,
                );
            }
        }
        Icon::Apps => {
            for row in 0..3 {
                for col in 0..3 {
                    let at = center
                        + Vec2::new((col as f32 - 1.0) * r * 0.6, (row as f32 - 1.0) * r * 0.6);
                    painter.circle_filled(at, r * 0.19, color);
                }
            }
        }
        Icon::ChevronRight => {
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.35, -r * 0.65),
                    center + Vec2::new(r * 0.35, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(r * 0.35, 0.0),
                    center + Vec2::new(-r * 0.35, r * 0.65),
                ],
                stroke,
            );
        }
        Icon::ChevronLeft => {
            painter.line_segment(
                [
                    center + Vec2::new(r * 0.35, -r * 0.65),
                    center + Vec2::new(-r * 0.35, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.35, 0.0),
                    center + Vec2::new(r * 0.35, r * 0.65),
                ],
                stroke,
            );
        }
        Icon::Trash => {
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.8, -r * 0.45),
                    center + Vec2::new(r * 0.8, -r * 0.45),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.25, -r * 0.45),
                    center + Vec2::new(-r * 0.25, -r * 0.7),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(r * 0.25, -r * 0.45),
                    center + Vec2::new(r * 0.25, -r * 0.7),
                ],
                stroke,
            );
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-r * 0.6, -r * 0.35),
                    center + Vec2::new(r * 0.6, -r * 0.35),
                    center + Vec2::new(r * 0.45, r * 0.85),
                    center + Vec2::new(-r * 0.45, r * 0.85),
                ],
                bg,
                stroke,
            ));
        }
        Icon::Copy => {
            let size = Vec2::splat(r * 0.95);
            rect_outline(
                painter,
                Rect::from_center_size(center + Vec2::new(0.25, -0.25) * r, size),
                stroke,
                bg,
            );
            painter.rect_filled(
                Rect::from_center_size(center + Vec2::new(-0.25, 0.25) * r, size),
                2.0,
                color,
            );
        }
        Icon::Pencil => {
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.75, r * 0.75),
                    center + Vec2::new(r * 0.55, -r * 0.55),
                ],
                stroke,
            );
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(r * 0.55, -r * 0.55),
                    center + Vec2::new(r * 0.88, -r * 0.88),
                    center + Vec2::new(r * 0.88, -r * 0.3),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Terminal => {
            rect_outline(
                painter,
                Rect::from_center_size(center, Vec2::new(r * 1.7, r * 1.35)),
                stroke,
                bg,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.5, -r * 0.35),
                    center + Vec2::new(-r * 0.1, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.1, 0.0),
                    center + Vec2::new(-r * 0.5, r * 0.35),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(r * 0.15, r * 0.35),
                    center + Vec2::new(r * 0.65, r * 0.35),
                ],
                stroke,
            );
        }
        Icon::Folder => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-r * 0.85, -r * 0.55),
                    center + Vec2::new(-r * 0.25, -r * 0.55),
                    center + Vec2::new(-r * 0.05, -r * 0.3),
                    center + Vec2::new(r * 0.85, -r * 0.3),
                    center + Vec2::new(r * 0.85, r * 0.7),
                    center + Vec2::new(-r * 0.85, r * 0.7),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Lock => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-r * 0.65, -r * 0.05),
                    center + Vec2::new(r * 0.65, -r * 0.05),
                    center + Vec2::new(r * 0.65, r * 0.85),
                    center + Vec2::new(-r * 0.65, r * 0.85),
                ],
                color,
                Stroke::NONE,
            ));
            let mut shackle = Vec::new();
            for i in 0..=8 {
                let angle = std::f32::consts::PI + (i as f32 / 8.0) * std::f32::consts::PI;
                shackle.push(
                    center
                        + Vec2::new(-r * 0.4, -r * 0.05)
                        + Vec2::new(angle.cos(), angle.sin()) * (r * 0.4),
                );
            }
            painter.add(Shape::line(shackle, stroke));
        }
        Icon::Info => {
            painter.circle_stroke(center, r * 0.85, stroke);
            painter.circle_filled(center + Vec2::new(0.0, -r * 0.4), r * 0.12, color);
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -r * 0.1),
                    center + Vec2::new(0.0, r * 0.45),
                ],
                stroke,
            );
        }
        Icon::Sliders => {
            for i in 0..3 {
                let y = center.y + (i as f32 - 1.0) * r * 0.6;
                painter.line_segment(
                    [
                        center + Vec2::new(-r * 0.85, y),
                        center + Vec2::new(r * 0.85, y),
                    ],
                    stroke,
                );
                let knob_x = center.x + (i as f32 - 1.0) * r * 0.45;
                painter.circle_filled(center + Vec2::new(knob_x, y), r * 0.18, color);
            }
        }
        Icon::Kebab => {
            for i in 0..3 {
                painter.circle_filled(
                    center + Vec2::new(0.0, (i as f32 - 1.0) * r * 0.65),
                    r * 0.16,
                    color,
                );
            }
        }
        Icon::Pin => {
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -r * 0.9),
                    center + Vec2::new(0.0, r * 0.4),
                ],
                stroke,
            );
            painter.circle_stroke(center + Vec2::new(0.0, r * 0.55), r * 0.35, stroke);
        }
        Icon::Hash => {
            for offset in [-0.45, 0.45] {
                painter.line_segment(
                    [
                        center + Vec2::new(offset * r, -r * 0.7),
                        center + Vec2::new(offset * r, r * 0.7),
                    ],
                    stroke,
                );
            }
            for offset in [-0.25, 0.3] {
                painter.line_segment(
                    [
                        center + Vec2::new(-r * 0.7, offset * r),
                        center + Vec2::new(r * 0.7, offset * r),
                    ],
                    stroke,
                );
            }
        }
        Icon::Globe => {
            painter.circle_stroke(center, r * 0.85, stroke);
            painter.line_segment(
                [
                    center + Vec2::new(-r * 0.85, 0.0),
                    center + Vec2::new(r * 0.85, 0.0),
                ],
                stroke,
            );
            let mut ellipse = Vec::new();
            for i in 0..=10 {
                let angle = (i as f32 / 10.0) * std::f32::consts::TAU;
                ellipse.push(center + Vec2::new(angle.cos() * r * 0.42, angle.sin() * r * 0.85));
            }
            painter.add(Shape::closed_line(ellipse, stroke));
        }
        Icon::Calculator => {
            rect_outline(
                painter,
                Rect::from_center_size(center, Vec2::new(r * 1.3, r * 1.7)),
                stroke,
                bg,
            );
            painter.rect_filled(
                Rect::from_center_size(
                    center + Vec2::new(0.0, -r * 0.45),
                    Vec2::new(r * 0.9, r * 0.3),
                ),
                1.0,
                color,
            );
            for row in 0..2 {
                for col in 0..2 {
                    let at = center
                        + Vec2::new(
                            (col as f32 - 0.5) * r * 0.55,
                            (row as f32 - 0.5) * r * 0.55 + r * 0.35,
                        );
                    painter.circle_filled(at, r * 0.13, color);
                }
            }
        }
        Icon::Sun => {
            painter.circle_filled(center, r * 0.42, color);
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::PI / 4.0;
                let dir = Vec2::new(angle.cos(), angle.sin());
                painter.line_segment([center + dir * r * 0.62, center + dir * r * 0.9], stroke);
            }
        }
        Icon::Moon => {
            let mut crescent = Vec::new();
            for i in 0..=14 {
                let angle = (i as f32 / 14.0) * std::f32::consts::TAU;
                crescent.push(center + Vec2::new(angle.cos(), angle.sin()) * (r * 0.85));
            }
            painter.add(Shape::convex_polygon(crescent, color, Stroke::NONE));
            painter.circle_filled(
                center + Vec2::new(r * 0.42, -r * 0.28),
                r * 0.75,
                bg_color_of(ui),
            );
        }
        Icon::Filter => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-r * 0.85, -r * 0.6),
                    center + Vec2::new(r * 0.85, -r * 0.6),
                    center + Vec2::new(r * 0.2, r * 0.1),
                    center + Vec2::new(r * 0.2, r * 0.85),
                    center + Vec2::new(-r * 0.2, r * 0.6),
                    center + Vec2::new(-r * 0.2, r * 0.1),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Power => {
            let mut arc = Vec::new();
            for i in 0..=12 {
                let angle = 0.5 + (i as f32 / 12.0) * 4.8;
                arc.push(center + Vec2::new(angle.cos(), angle.sin()) * (r * 0.8));
            }
            painter.add(Shape::line(arc, stroke));
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -r * 0.85),
                    center + Vec2::new(0.0, -r * 0.1),
                ],
                stroke,
            );
        }
        Icon::Keyboard => {
            rect_outline(
                painter,
                Rect::from_center_size(center, Vec2::new(r * 1.8, r * 1.1)),
                stroke,
                bg,
            );
            for row in 0..2 {
                for col in 0..4 {
                    let at = center
                        + Vec2::new((col as f32 - 1.5) * r * 0.38, (row as f32 - 0.5) * r * 0.42);
                    painter.circle_filled(at, r * 0.11, color);
                }
            }
        }
        Icon::Home => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-r * 0.9, 0.0),
                    center + Vec2::new(0.0, -r * 0.85),
                    center + Vec2::new(r * 0.9, 0.0),
                ],
                color,
                Stroke::NONE,
            ));
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-r * 0.6, r * 0.05),
                    center + Vec2::new(r * 0.6, r * 0.05),
                    center + Vec2::new(r * 0.6, r * 0.85),
                    center + Vec2::new(-r * 0.6, r * 0.85),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}

/// The background colour currently used by the window; used to "punch holes".
fn bg_color_of(ui: &Ui) -> Color32 {
    ui.visuals().panel_fill
}

/// Draw an outlined rectangle without relying on `Painter::rect_stroke`
/// (whose signature differs between egui versions).
fn rect_outline(painter: &egui::Painter, rect: Rect, stroke: Stroke, fill: Color32) {
    painter.add(Shape::convex_polygon(
        vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ],
        fill,
        stroke,
    ));
}

fn star_points(center: Pos2, outer: f32, inner: f32) -> Vec<Pos2> {
    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let radius = if i % 2 == 0 { outer } else { inner };
        let angle = -std::f32::consts::FRAC_PI_2 + (i as f32) * std::f32::consts::PI / 5.0;
        points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
    }
    points
}

/// Allocate an icon without stealing pointer events from its parent row.
#[allow(dead_code)]
pub fn icon(
    ui: &mut Ui,
    icon: Icon,
    size: f32,
    color: Color32,
    palette: &Palette,
) -> egui::Response {
    let (_, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let center = response.rect.center();
    draw_icon(ui, icon, center, size, color, palette.bg);
    response
}

/// Paint the letter badge for `display_name` into an already allocated `rect`.
pub fn paint_app_badge(ui: &mut Ui, rect: Rect, display_name: &str, size: f32, palette: &Palette) {
    let color = badge_color(display_name);
    let radius = if size >= 34.0 {
        size * 0.28
    } else {
        size * 0.5
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(radius.round() as u8), color);

    let letter = first_letter(display_name);
    let on_color = if luminance(color) > 140.0 {
        palette.bg
    } else {
        Color32::WHITE
    };
    let font_size = (size * 0.42).clamp(9.0, 28.0);
    let badge_rect = rect;
    ui.scope_builder(egui::UiBuilder::new().max_rect(badge_rect), |ui| {
        ui.centered_and_justified(|ui| {
            let _ = ui.label(
                RichText::new(letter)
                    .color(on_color)
                    .size(font_size)
                    .strong(),
            );
        });
    });
}

/// A badge showing the first letter of an application, allocating `size × size`.
pub fn app_badge(
    ui: &mut Ui,
    _entry: &AppEntry,
    display_name: &str,
    size: f32,
    palette: &Palette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_app_badge(ui, rect, display_name, size, palette);
    response
}

/// First alphanumeric character of a name, upper cased.
pub fn first_letter(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "#".to_string())
}

fn luminance(color: Color32) -> f32 {
    let [r, g, b, _] = color.to_array();
    0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
}

/// A square icon button with hover / press feedback.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    size: f32,
    color: Color32,
    palette: &Palette,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if response.hovered() {
        let fill = if response.is_pointer_button_down_on() {
            palette.hover
        } else {
            palette.hover
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    }
    let draw_color = if response.hovered() {
        palette.text
    } else {
        color
    };
    draw_icon(ui, icon, rect.center(), size * 0.55, draw_color, palette.bg);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// A pill shaped text button (used for tags and quick filters).
#[allow(dead_code)]
pub fn chip(ui: &mut Ui, label: &str, active: bool, palette: &Palette) -> bool {
    let text = RichText::new(label)
        .size(11.0)
        .color(if active { palette.bg } else { palette.dim });
    let button = egui::Button::new(text)
        .fill(if active {
            palette.accent
        } else {
            palette.surface
        })
        .stroke(Stroke::new(
            1.0f32,
            if active {
                palette.accent
            } else {
                palette.border
            },
        ))
        .corner_radius(CornerRadius::same(10));
    ui.add(button).clicked()
}

/// A clickable area covering `rect`, with drag support so that scrolling a
/// list never triggers a click.
#[allow(dead_code)]
pub fn clickable(ui: &mut Ui, rect: Rect) -> egui::Response {
    ui.interact(rect, ui.next_auto_id(), Sense::click_and_drag())
}

/// Horizontal separator honouring the current palette.
#[allow(dead_code)]
pub fn divider(ui: &mut Ui, palette: &Palette, spacing: f32) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), spacing), Sense::hover());
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0f32, palette.border),
    );
}

/// Small section title used by the settings screen and by the list headers.
#[allow(dead_code)]
pub fn section_title(ui: &mut Ui, text: &str, palette: &Palette) {
    let _ = ui.label(
        RichText::new(text.to_uppercase())
            .size(11.0)
            .strong()
            .color(palette.dim),
    );
}

/// A keyboard key rendered as a small rounded box.
#[allow(dead_code)]
pub fn key_cap(ui: &mut Ui, label: &str, palette: &Palette) {
    let text = RichText::new(label).size(10.0).color(palette.dim);
    let button = egui::Button::new(text)
        .fill(palette.surface)
        .stroke(Stroke::new(1.0f32, palette.border))
        .corner_radius(CornerRadius::same(4))
        .min_size(Vec2::new(label.len() as f32 * 7.0 + 12.0, 18.0));
    let _ = ui.add(button);
}

/// Blend two colours, `t` in 0..=1.
#[allow(dead_code)]
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [r1, g1, b1, a1] = a.to_array();
    let [r2, g2, b2, a2] = b.to_array();
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_premultiplied(lerp(r1, r2), lerp(g1, g2), lerp(b1, b2), lerp(a1, a2))
}

/// Progress of the press animation, used for the touch feedback of rows.
#[allow(dead_code)]
pub fn press_progress(ui: &mut Ui, id: &str, pressed: bool) -> f32 {
    let animation_time = if pressed { 0.08 } else { 0.18 };
    ui.ctx()
        .animate_bool_with_time(egui::Id::new(id), pressed, animation_time)
}

/// Font used for the launcher, scaled by the `font_scale` setting.
#[allow(dead_code)]
pub fn scaled(size: f32, scale: f32) -> FontId {
    FontId::proportional(size * scale.clamp(0.6, 2.0))
}
