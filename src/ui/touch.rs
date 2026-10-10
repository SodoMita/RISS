//! Touch and pointer gesture handling.
//!
//! Translates drags, taps and long presses on the launcher's empty area into
//! the configurable actions stored under the `gesture-*` settings.

use super::{RissApp, Screen};
use eframe::egui;

impl RissApp {
    fn perform_gesture(&mut self, action: &str, ctx: &egui::Context) {
        match action {
            "display-keyboard" => ctx.memory_mut(|m| m.request_focus(egui::Id::new("riss-search"))),
            "hide-keyboard" => ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("riss-search"))),
            "display-apps" => {
                self.show_all_apps = true;
                self.update_results();
            }
            "display-history" => {
                self.show_all_apps = false;
                self.update_results();
            }
            "display-menu" => self.screen = Screen::Settings,
            "go-to-homescreen" => {
                self.query.clear();
                self.show_all_apps = false;
                self.update_results();
            }
            "launch-pojo" => self.set_status("Choose a launch target in gesture settings"),
            "display-notifications" | "display-quicksettings" => {
                self.set_status("This system gesture is not available on this platform")
            }
            _ => {}
        }
    }

    pub(super) fn handle_empty_area_gestures(
        &mut self,
        response: &egui::Response,
        ctx: &egui::Context,
    ) {
        let now = ctx.input(|i| i.time);
        if response.drag_started() {
            if let Some(p) = ctx.input(|i| i.pointer.press_origin()) {
                self.touch_start = Some((p, now));
            }
        }
        if response.drag_stopped() {
            if let (Some((start, _)), Some(end)) = (
                self.touch_start.take(),
                ctx.input(|i| i.pointer.interact_pos()),
            ) {
                let delta = end - start;
                if delta.length() > 55.0 {
                    let key = if delta.x.abs() > delta.y.abs() {
                        if delta.x > 0.0 {
                            "gesture-right"
                        } else {
                            "gesture-left"
                        }
                    } else if delta.y > 0.0 {
                        "gesture-down"
                    } else {
                        "gesture-up"
                    };
                    let action = self.settings.value(key).to_owned();
                    self.perform_gesture(&action, ctx);
                }
            }
        }
        if response.long_touched() {
            let action = self.settings.value("gesture-long-press").to_owned();
            self.perform_gesture(&action, ctx);
        }
        if response.clicked() {
            if self.settings.enabled("history-onclick") {
                self.show_all_apps = false;
                self.update_results();
            }
            if self.settings.enabled("double-tap") {
                let pos = ctx
                    .input(|i| i.pointer.interact_pos())
                    .unwrap_or(response.rect.center());
                if let Some((last, time)) = self.last_empty_tap {
                    if now - time < 0.35 && last.distance(pos) < 30.0 {
                        self.set_status(
                            "Double-tap lock requires Android accessibility permission",
                        );
                        self.last_empty_tap = None;
                        return;
                    }
                }
                self.last_empty_tap = Some((pos, now));
            }
        }
    }
}
