//! A typed query becoming result rows in egui's frame output.
//!
//! `crate::tests::live_app` proves the rows reach `RissApp::results`. This
//! goes one step further along the same production path: the query is typed
//! into the real search field as egui-winit would deliver it, the real
//! `update` runs frame after frame (so the worker's answer is collected by
//! `poll_workers` like on a device), and the assertion is on the shapes egui
//! hands to the renderer. The only thing left between here and a pixel is the
//! GPU, and that part is `scripts/gui-assertions.sh`.

use super::tap_tests::Rig;
use super::*;
use eframe::egui::{epaint::Shape, Event};
use std::time::{Duration, Instant};

fn app_entry(name: &str, exec: &str) -> AppEntry {
    let mut e = AppEntry::virtual_entry(name, "", exec);
    e.desktop_file = std::path::PathBuf::from(format!("/nonexistent/{exec}.desktop"));
    e
}

/// Every text shape in a frame, as (text, top-left y).
fn texts(output: &egui::FullOutput) -> Vec<(String, f32)> {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            Shape::Text(t) => Some((t.galley.text().to_string(), t.pos.y)),
            _ => None,
        })
        .collect()
}

#[test]
fn typing_a_query_puts_the_matching_row_in_the_frame_above_the_search_bar() {
    let mut rig = Rig::with_apps(
        vec![
            app_entry("Firefox", "firefox"),
            app_entry("Café Noir", "cafe"),
            app_entry("Nautilus", "nautilus"),
        ],
        1.0,
    );
    // Settle, then make sure nothing is listed yet: no history, no query.
    rig.step(Vec::new());
    let before = rig.frame_output(Vec::new());
    assert!(
        !texts(&before).iter().any(|(t, _)| t == "Firefox"),
        "an empty query with no history must list nothing"
    );

    // Focus the field and type, the way egui-winit delivers a key press.
    rig.ctx
        .memory_mut(|m| m.request_focus(egui::Id::new("riss-search")));
    rig.step(Vec::new());
    rig.step(vec![Event::Text("cafe".to_string())]);

    // The answer comes from the worker thread; frames collect it.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = None;
    while Instant::now() < deadline {
        let output = rig.frame_output(Vec::new());
        let found = texts(&output).iter().any(|(t, _)| t == "Café Noir");
        last = Some(output);
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let output = last.expect("no frame ran");
    let shapes = texts(&output);
    let row_y = shapes
        .iter()
        .find(|(t, _)| t == "Café Noir")
        .map(|(_, y)| *y)
        .unwrap_or_else(|| {
            panic!("\"cafe\" never produced a Café Noir row; frame text: {shapes:?}")
        });
    let query_y = shapes
        .iter()
        .find(|(t, _)| t == "cafe")
        .map(|(_, y)| *y)
        .expect("the typed query must be painted in the search field");
    assert!(
        row_y < query_y,
        "RISS stacks results above the search bar (KISS layout); row at y={row_y}, query at y={query_y}"
    );
    // ... and the non-match is not there.
    assert!(
        !shapes.iter().any(|(t, _)| t == "Nautilus"),
        "a non-matching app must not be painted: {shapes:?}"
    );
}

#[test]
fn backspacing_to_empty_clears_the_rows_from_the_frame() {
    let mut rig = Rig::with_apps(vec![app_entry("Firefox", "firefox")], 1.0);
    rig.step(Vec::new());
    rig.ctx
        .memory_mut(|m| m.request_focus(egui::Id::new("riss-search")));
    rig.step(Vec::new());
    rig.step(vec![Event::Text("fire".to_string())]);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if texts(&rig.frame_output(Vec::new()))
            .iter()
            .any(|(t, _)| t == "Firefox")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    // Four backspaces, one per frame, as a user would produce them.
    for _ in 0..4 {
        rig.step(vec![Event::Key {
            key: egui::Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    let after = rig.frame_output(Vec::new());
    assert!(
        !texts(&after).iter().any(|(t, _)| t == "Firefox"),
        "an emptied field must take its rows with it: {:?}",
        texts(&after)
    );
}
