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
    /// What the last frame painted, for layout measurements.
    shapes: Vec<egui::epaint::ClippedShape>,
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
        app.install_apps_for_test(vec![entry]);
        app.show_all_apps = true;
        let mut rig = Self {
            ctx,
            app,
            frame: eframe::Frame::_new_kittest(),
            time: 0.0,
            pixels_per_point,
            shapes: Vec::new(),
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
        self.shapes = ctx.run(input, |ctx| app.update(ctx, frame)).shapes;
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

/// Bottom edge of the result list's viewport: the clip rectangle the
/// rows' text is painted with.
fn list_viewport_bottom(rig: &Rig) -> f32 {
    rig.shapes
        .iter()
        .filter(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => text.galley.text().starts_with("App "),
            _ => false,
        })
        .map(|clipped| clipped.clip_rect.max.y)
        .fold(f32::NAN, f32::max)
}

/// Every filled rectangle the last frame painted.
fn filled_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, egui::Color32)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            egui::Shape::Rect(r) if r.fill.a() > 0 => out.push((r.rect, r.fill)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Lowest and highest `y` in `range` where a tap at `x` launches an app,
/// without resetting history between taps (favorites must survive).
fn launch_band(rig: &mut Rig, x: f32, range: std::ops::Range<i32>) -> Option<(f32, f32)> {
    let mut band: Option<(f32, f32)> = None;
    for y in range {
        let y = y as f32 + 0.5;
        rig.tap(Input::Mouse, Pos2::new(x, y));
        let launched = rig
            .app
            .status_message
            .take()
            .is_some_and(|(message, _)| message.starts_with("Failed to launch"));
        egui::Popup::close_all(&rig.ctx);
        rig.app.show_all_apps = true;
        rig.app.update_results();
        rig.step(Vec::new());
        rig.step(Vec::new());
        if launched {
            band = Some(band.map_or((y, y), |(lo, _)| (lo, y)));
        }
    }
    band
}

/// 40 launchable rows; row 0 launches [`EXEC`].
fn forty_apps(comment: &str, tags: &[&str]) -> Vec<AppEntry> {
    (0..40)
        .map(|i| {
            let exec = if i == 0 {
                EXEC.to_owned()
            } else {
                format!("{EXEC}-{i}")
            };
            let mut entry = AppEntry::virtual_entry(&format!("App {i:02}"), comment, &exec);
            entry.desktop_file = std::path::PathBuf::from("/nonexistent/app.desktop");
            entry.tags = tags.iter().map(|t| t.to_string()).collect();
            entry
        })
        .collect()
}

fn panel_rect(rig: &Rig, name: &str) -> Rect {
    egui::containers::panel::PanelState::load(&rig.ctx, egui::Id::new(name))
        .map(|state| state.rect)
        .expect("panel shown")
}

/// Issue #78 and the "too much empty space between apps and search" report:
/// the favorites row must sit midway between the end of the result list and
/// the search box, with no dead band, the favorite must answer taps over its
/// whole height, and the search box must keep the same margin on both sides.
#[test]
fn favorites_row_sits_midway_between_results_and_search() {
    for large in [false, true] {
        let mut rig = Rig::new(1.0);
        rig.app
            .settings
            .bools
            .insert("large-search-bar".into(), large);
        let apps = forty_apps("", &[]);
        rig.app.install_apps_for_test(apps);
        rig.app.history.add_favorite(EXEC);
        rig.app.reapply_usage();
        rig.app.show_all_apps = true;
        rig.app.update_results();
        rig.step(Vec::new());
        rig.step(Vec::new());

        let bar = panel_rect(&rig, "launcher-favorites");
        let list_end = list_viewport_bottom(&rig);
        let field = rig
            .ctx
            .read_response(egui::Id::new("riss-search"))
            .expect("search field")
            .rect;
        let search_box = filled_rects(&rig.shapes)
            .into_iter()
            .map(|(rect, _)| rect)
            .filter(|rect| rect.contains_rect(field))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .expect("search box is painted");
        // The favorite's letter is painted at the centre of its slot
        // ("transparent favorites" is the default).
        let centre = rig
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if bar.contains(text.pos) && text.galley.text() == "A" => {
                    Some(text.visual_bounding_rect().center().y)
                }
                _ => None,
            })
            .expect("favorite is painted in the bar");
        let taps = launch_band(&mut rig, 32.0, bar.min.y as i32..bar.max.y as i32)
            .expect("the favorite answers taps");

        let above = centre - list_end;
        let below = search_box.min.y - centre;
        eprintln!(
            "large={large}: bar {bar:?}, list ends {list_end}, favorite centre {centre}, taps {taps:?}, search box {search_box:?}, centre to list {above}, to search {below}"
        );
        // 4pt: egui lets rows paint 3pt past the list's content rect.
        assert!(
            (above - below).abs() <= 4.0,
            "row is off centre: {above} above, {below} below"
        );
        assert!(
            below <= 42.0,
            "dead band: search box {below}pt below the row's centre"
        );
        assert!(
            taps.1 - taps.0 >= 40.0,
            "favorite answers taps only on {taps:?}"
        );
        let left = search_box.min.x;
        let right = SCREEN.x - search_box.max.x;
        assert!(
            (left - right).abs() <= 1.0,
            "search box margins {left} / {right}"
        );
        if large {
            assert!(
                search_box.height() >= 70.0,
                "large search bar is not larger"
            );
        }
    }
}

/// The virtual list must draw rows down to the bottom edge of its viewport,
/// whatever the row shape. With the old row-height constants it reserved
/// 104pt for a 60pt row and ran out of rows a quarter screen early.
#[test]
fn result_rows_reach_the_bottom_of_the_list() {
    let shapes: [(&str, bool, &str, &[&str]); 4] = [
        ("default", true, "A comment", &["tag"]),
        ("default", true, "", &[]),
        ("small", false, "A comment", &[]),
        ("large", true, "A comment", &["tag", "other"]),
    ];
    for (size, lines, comment, tags) in shapes {
        let mut rig = Rig::new(1.0);
        rig.app
            .settings
            .values
            .insert("results-size".into(), size.into());
        rig.app
            .settings
            .bools
            .insert("subicon-visible".into(), lines);
        rig.app.settings.bools.insert("tags-visible".into(), lines);
        let apps = forty_apps(comment, tags);
        rig.app.install_apps_for_test(apps);
        rig.app.show_all_apps = true;
        rig.app.update_results();
        rig.step(Vec::new());
        rig.step(Vec::new());
        let list_end = list_viewport_bottom(&rig);
        let lowest = launch_band(&mut rig, 200.0, 200..list_end as i32 + 20)
            .expect("rows answer taps")
            .1;
        eprintln!("{size} lines={lines} comment={comment:?} tags={tags:?}: rows answer down to {lowest}, list ends {list_end}");
        // A row or the 8pt gap between two rows may meet the edge.
        assert!(
            lowest >= list_end - 12.0,
            "{size}/{lines}: rows stop at {lowest}, {}pt above the end of the list",
            list_end - lowest
        );
    }
}
