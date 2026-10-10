use eframe::egui::{self, Color32, Pos2, Shape, Stroke, Vec2};

/// Bottom-bar icons in the style of KISS for Android (`main.xml`), with one
/// deliberate deviation: the settings button is a cog, not KISS's kebab
/// (`dots.xml`).
///
/// * `Kiss` — the hollow circle from `ic_launcher_white.xml`, tinted with the
///   primary/accent color (white on the all-apps "kiss bar"). Opens the app
///   list; while it is open the ring gets a filled center circle (our
///   addition — KISS keeps the ring hollow).
/// * `Cog` — settings gear, tinted with the search (text) color. Opens
///   settings.
/// * `Clear` — the X from `ic_close.xml`, tinted with the search color.
///   Clears the query.
///
/// All geometry is expressed in a 24px glyph box (KISS's 24dp viewport)
/// centered in the 44px touch target. The box origin is snapped to whole
/// pixels so integer geometry lands on integer pixels; egui adds ~1px of
/// feathering around every shape on top of that.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BarIcon {
    Kiss {
        /// Fill the ring center while the all-apps list is open.
        filled_center: bool,
    },
    Cog,
    Clear,
}

/// Translucent wash for the ripple behind the glyph, from an opaque theme
/// color. Unlike [`Color32::gamma_multiply`] (which keeps alpha at 255 and
/// paints an opaque blob), this puts the darkness in the alpha channel;
/// `Color32` stores premultiplied channels, so the wash renders correctly
/// under egui's blending.
fn feedback_fill(base: Color32, alpha: u8) -> Color32 {
    debug_assert_eq!(base.a(), 255);
    Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
}

/// One cog tooth: a flat-cut trapezoid rooted under the hub ring (r ≈ 5) and
/// reaching r = 10.5 at the tip, pointing in the `angle` direction (0 = up).
fn cog_tooth(center: Pos2, angle: f32) -> Vec<Pos2> {
    let (sin, cos) = angle.sin_cos();
    let rotate = |x: f32, y: f32| center + Vec2::new(x * cos - y * sin, x * sin + y * cos);
    vec![
        rotate(-2.3, -4.5),
        rotate(-1.8, -10.5),
        rotate(1.8, -10.5),
        rotate(2.3, -4.5),
    ]
}

/// KISS-style icon button: a 44x44 touch target with a vector glyph painted
/// via the egui painter instead of a font-dependent unicode character.
///
/// `icon_color` is the glyph color picked by the caller (accent for the
/// Kiss ring, white on the all-apps bar, text color otherwise); hover/press
/// feedback is a translucent wash of the same color.
/// `show_glyph` implements KISS `pref-hide-circle`: the touch target stays
/// clickable, only the glyph is hidden.
pub(super) fn bar_icon_button(
    ui: &mut egui::Ui,
    icon: BarIcon,
    icon_color: Color32,
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

        // 24px glyph box centered in the target, snapped to whole pixels.
        let origin = (rect.center() - Vec2::splat(12.0)).round();
        let center = origin + Vec2::splat(12.0);

        // KISS uses a ripple (`selectableItemBackground`); approximate it
        // with a translucent disc. Pressed wins over hover because a touch
        // tap also reports hover.
        if response.is_pointer_button_down_on() {
            painter.circle_filled(center, 17.0, feedback_fill(icon_color, 55));
        } else if response.hovered() {
            painter.circle_filled(center, 17.0, feedback_fill(icon_color, 30));
        }

        if show_glyph {
            match icon {
                BarIcon::Kiss { filled_center } => {
                    // `ic_launcher_white.xml`: outer diameter is 87% of the
                    // box, stroke 14%. r=9, w=3 (Ø 87.5%, stroke 12.5%) is
                    // the closest whole-pixel match.
                    painter.circle_stroke(center, 9.0, Stroke::new(3.0_f32, icon_color));
                    if filled_center {
                        painter.circle_filled(center, 5.0, icon_color);
                    }
                }
                BarIcon::Cog => {
                    // Hollow hub ring with six flat-cut teeth. Outer extent
                    // (Ø 21) matches the Kiss ring.
                    painter.circle_stroke(center, 5.5, Stroke::new(3.0_f32, icon_color));
                    for i in 0..6 {
                        let angle = i as f32 * std::f32::consts::FRAC_PI_3;
                        let tooth = cog_tooth(center, angle);
                        painter.add(Shape::convex_polygon(tooth, icon_color, Stroke::NONE));
                    }
                }
                BarIcon::Clear => {
                    // `ic_close.xml` (viewBox 960, ÷40): a filled 12-gon
                    // with flat-cut tips and ~8%-wide arms — the union of its
                    // two diagonal bars. epaint 0.32 fan-fills closed paths
                    // (convex only), so each bar is drawn as its own quad.
                    let sw_ne = vec![
                        origin + Vec2::new(17.6, 5.0),
                        origin + Vec2::new(19.0, 6.4),
                        origin + Vec2::new(6.4, 19.0),
                        origin + Vec2::new(5.0, 17.6),
                    ];
                    let nw_se = vec![
                        origin + Vec2::new(6.4, 5.0),
                        origin + Vec2::new(19.0, 17.6),
                        origin + Vec2::new(17.6, 19.0),
                        origin + Vec2::new(5.0, 6.4),
                    ];
                    for bar in [sw_ne, nw_se] {
                        painter.add(Shape::convex_polygon(bar, icon_color, Stroke::NONE));
                    }
                }
            }
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_fill_makes_a_translucent_wash() {
        let c = feedback_fill(Color32::from_rgb(10, 20, 30), 55);
        // The point of the helper: a real alpha value, not an opaque blob.
        assert_eq!(c.a(), 55);
        // `Color32` stores premultiplied channels, so the wash is darker
        // than the opaque base while keeping its hue.
        assert!(c.r() < 10);
        assert!(c.g() < 20);
        assert!(c.b() < 30);
    }

    #[test]
    fn cog_teeth_reach_the_ring_and_stay_in_the_box() {
        let center = Pos2::new(12.0, 12.0);
        for i in 0..6 {
            let angle = i as f32 * std::f32::consts::FRAC_PI_3;
            for p in cog_tooth(center, angle) {
                let r = (p - center).length();
                // Roots tuck under the hub ring (r 4-7), tips stop just
                // short of the 24px glyph box (max r = 12).
                assert!(r >= 4.0);
                assert!(r <= 11.0);
            }
        }
    }
}
