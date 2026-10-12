//! Headless taps on the real launcher (issues #9 and #18).
//!
//! The production `RissApp` is built through eframe's own test constructor
//! and fed the exact event sequences egui-winit 0.32 produces: for a finger,
//! `Touch` + `PointerMoved` + `PointerButton` in the frame the touch starts
//! and `Touch` + `PointerButton` + `PointerGone` in the frame it ends (no
//! hover frame before the press, no pointer after the release); for a mouse,
//! a hover frame, a press frame and a release frame. Each tap position along
//! a result row is then classified by what the app actually did. Nothing is
//! re-derived here: the row, the ★ and ⋮ buttons, the row's click area and
//! the deferred activation are the code the binary runs.

use super::*;
use eframe::egui::{
    Event, Modifiers, PointerButton, Pos2, RawInput, Rect, TouchDeviceId, TouchId, TouchPhase, Vec2,
};
use eframe::App as _;

/// Launching this fails at `spawn`, so a "launch" is visible as a status
/// message and no process is ever started by the tests.
const EXEC: &str = "/nonexistent/riss-tap-target";
/// A phone-sized window, in points.
const SCREEN: Vec2 = Vec2::new(392.0, 760.0);

/// What one tap did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Nothing,
    Launch,
    Favorite,
    Menu,
    Several,
}

impl Hit {
    fn glyph(self) -> char {
        match self {
            Hit::Nothing => '.',
            Hit::Launch => 'L',
            Hit::Favorite => 'F',
            Hit::Menu => 'M',
            Hit::Several => 'X',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
    Mouse,
    Finger,
    /// A short tap on a busy device: touch start and end are delivered in
    /// the same frame, so press, release and `PointerGone` arrive together.
    QuickFinger,
}

/// Keep `save_history` away from the developer's real config directory.
fn isolate_storage() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("riss-tap-tests-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var(crate::storage::DATA_DIR_ENV, &dir);
    });
}

struct Rig {
    ctx: egui::Context,
    app: RissApp,
    frame: eframe::Frame,
    time: f64,
    pixels_per_point: f32,
}

impl Rig {
    /// The real app, showing one installed (non-virtual) app in the
    /// all-apps list, after two settled frames.
    fn new(pixels_per_point: f32) -> Self {
        isolate_storage();
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = RissApp::with_settings(&cc, SettingsData::default(), Vec::new());
        let mut entry = AppEntry::virtual_entry("Tap Target", "", EXEC);
        entry.desktop_file = std::path::PathBuf::from("/nonexistent/riss-tap-target.desktop");
        assert!(!entry.is_virtual(), "the row must carry ★ and ⋮");
        app.all_apps = vec![entry.clone()];
        app.apps = vec![entry];
        app.show_all_apps = true;
        let mut rig = Self {
            ctx,
            app,
            frame: eframe::Frame::_new_kittest(),
            time: 0.0,
            pixels_per_point,
        };
        rig.reset();
        rig
    }

    fn step(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        input
            .viewports
            .entry(input.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(self.pixels_per_point);
        let Self {
            ctx, app, frame, ..
        } = self;
        let _ = ctx.run(input, |ctx| app.update(ctx, frame));
    }

    fn tap(&mut self, input: Input, pos: Pos2) {
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        match input {
            Input::Finger => {
                let touch = |phase| Event::Touch {
                    device_id: TouchDeviceId(0),
                    id: TouchId(0),
                    phase,
                    pos,
                    force: None,
                };
                self.step(vec![
                    touch(TouchPhase::Start),
                    Event::PointerMoved(pos),
                    button(true),
                ]);
                self.step(vec![
                    touch(TouchPhase::End),
                    button(false),
                    Event::PointerGone,
                ]);
            }
            Input::QuickFinger => {
                let touch = |phase| Event::Touch {
                    device_id: TouchDeviceId(0),
                    id: TouchId(0),
                    phase,
                    pos,
                    force: None,
                };
                self.step(vec![
                    touch(TouchPhase::Start),
                    Event::PointerMoved(pos),
                    button(true),
                    touch(TouchPhase::End),
                    button(false),
                    Event::PointerGone,
                ]);
            }
            Input::Mouse => {
                self.step(vec![Event::PointerMoved(pos)]);
                self.step(vec![button(true)]);
                self.step(vec![button(false)]);
                self.step(vec![Event::PointerGone]);
            }
        }
        // Row activation is deferred to the end of the frame that saw the
        // click, and popups open on the frame after; let both land.
        self.step(Vec::new());
    }

    fn outcome(&self) -> Hit {
        let launch = self
            .app
            .status_message
            .as_ref()
            .is_some_and(|(message, _)| message.starts_with("Failed to launch"));
        let favorite = self.app.history.is_favorite(EXEC);
        let menu = egui::Popup::is_any_open(&self.ctx);
        match (launch, favorite, menu) {
            (false, false, false) => Hit::Nothing,
            (true, false, false) => Hit::Launch,
            (false, true, false) => Hit::Favorite,
            (false, false, true) => Hit::Menu,
            _ => Hit::Several,
        }
    }

    /// Back to a clean slate: no favorite, no status, no popup, no pointer.
    fn reset(&mut self) {
        self.app.history = HistoryData::default();
        self.app.status_message = None;
        self.app.reapply_usage();
        // Not Escape: the launcher treats it as "back" and leaves the
        // all-apps list, which would make every later tap hit nothing.
        egui::Popup::close_all(&self.ctx);
        self.app.show_all_apps = true;
        self.app.update_results();
        self.step(vec![Event::PointerGone]);
        self.step(Vec::new());
        self.app.status_message = None;
        assert_eq!(
            self.outcome(),
            Hit::Nothing,
            "reset must leave nothing open"
        );
    }

    fn tap_and_classify(&mut self, input: Input, pos: Pos2) -> Hit {
        self.tap(input, pos);
        let hit = self.outcome();
        self.reset();
        hit
    }
}

/// Vertical centre of the first result row, found by tapping down the
/// left edge (the app badge) with a mouse until taps launch.
fn first_row_center(rig: &mut Rig) -> f32 {
    let band: Vec<f32> = (0..240)
        .step_by(2)
        .map(|y| y as f32 + 0.5)
        .filter(|&y| rig.tap_and_classify(Input::Mouse, Pos2::new(40.0, y)) == Hit::Launch)
        .collect();
    assert!(!band.is_empty(), "no tap on the badge launched the app");
    (band[0] + band[band.len() - 1]) / 2.0
}

/// What a tap does at every 2 points along the first row's centre line.
fn hit_map(input: Input, pixels_per_point: f32) -> String {
    let mut rig = Rig::new(pixels_per_point);
    let y = first_row_center(&mut rig);
    (0..SCREEN.x as i32)
        .step_by(2)
        .map(|x| {
            rig.tap_and_classify(input, Pos2::new(x as f32 + 0.5, y))
                .glyph()
        })
        .collect()
}

fn widest_run(map: &str, glyph: char) -> usize {
    map.split(|c| c != glyph).map(str::len).max().unwrap_or(0)
}

#[test]
fn star_and_kebab_take_finger_taps_like_mouse_clicks() {
    let mouse = hit_map(Input::Mouse, 1.0);
    let finger = hit_map(Input::Finger, 1.0);
    let phone = hit_map(Input::Finger, 2.75);
    let quick = hit_map(Input::QuickFinger, 2.75);
    eprintln!("mouse  {mouse}\nfinger {finger}\nphone  {phone}\nquick  {quick}");

    for (name, map) in [
        ("mouse", &mouse),
        ("finger", &finger),
        ("phone", &phone),
        ("quick", &quick),
    ] {
        assert!(!map.contains('X'), "{name}: one tap did two things: {map}");
        // Each button is 28 points wide; demand at least 20 of them answer.
        assert!(
            widest_run(map, 'F') >= 10,
            "{name}: ★ answers on fewer than 20 points: {map}"
        );
        assert!(
            widest_run(map, 'M') >= 10,
            "{name}: ⋮ answers on fewer than 20 points: {map}"
        );
    }
    assert_eq!(finger, mouse, "a finger tap must do what a click does");
    assert_eq!(phone, mouse, "and so on a 2.75x phone screen");
    assert_eq!(quick, mouse, "and when the whole tap lands in one frame");
}
