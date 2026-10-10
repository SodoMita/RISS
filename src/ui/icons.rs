//! App icon textures: a name-keyed cache, a per-frame decode budget, and a
//! record of the icons that could not be loaded.
//!
//! Loading an icon is by far the most expensive thing a result row does. On
//! desktop it walks the XDG icon theme looking for a file and decodes a PNG;
//! on Android it goes through JNI to `PackageManager`, renders the drawable
//! into a bitmap and copies its pixels. Two things follow from that:
//!
//! * Textures are cached by *icon name* rather than by app, so every browser
//!   sharing one icon uploads it once.
//! * Only a bounded number of icons is decoded per frame. Scrolling a long
//!   list used to decode every newly revealed icon in the same frame, which is
//!   what made flicks stutter. Rows left without an icon ask for one more
//!   repaint, so the list fills in while it is at rest.

use std::collections::{HashMap, HashSet};

use eframe::egui::{self, ColorImage, TextureHandle, TextureId};

#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};

/// How many icons may be decoded during one frame.
const DECODE_BUDGET: usize = 6;

#[derive(Default)]
pub(super) struct IconCache {
    textures: HashMap<String, TextureHandle>,
    /// Keys with no loadable icon. Remembered so a missing icon is looked for
    /// exactly once instead of on every frame it is on screen.
    missing: HashSet<String>,
    budget: usize,
    pending: bool,
}

impl IconCache {
    /// Allow a fresh batch of decodes and forget last frame's backlog.
    ///
    /// Called once per frame by [`crate::ui`], before any panel draws, so that
    /// the favorites bar and the result list share one budget.
    pub(super) fn begin_frame(&mut self) {
        self.budget = DECODE_BUDGET;
        self.pending = false;
    }

    /// `true` while some visible row is still waiting for its icon, which is
    /// the caller's cue to schedule another repaint. Taking the flag clears
    /// it, so a screen that draws no icons cannot latch it on forever.
    pub(super) fn take_pending(&mut self) -> bool {
        std::mem::take(&mut self.pending)
    }

    /// Drop every cached texture, e.g. after an icon setting changed.
    pub(super) fn clear(&mut self) {
        self.textures.clear();
        self.missing.clear();
    }

    /// The icon texture for `entry`, decoding it when this frame still has
    /// budget. `None` means "no icon", either permanently (the icon does not
    /// exist) or for now (the decode is queued for a later frame).
    pub(super) fn texture(&mut self, ctx: &egui::Context, entry: &AppEntry) -> Option<TextureId> {
        let key = icon_key(entry);
        if let Some(texture) = self.textures.get(key) {
            return Some(texture.id());
        }
        if self.missing.contains(key) {
            return None;
        }
        if self.budget == 0 {
            self.pending = true;
            return None;
        }
        self.budget -= 1;

        let Some((width, height, rgba)) = app_entry::load_icon_rgba(entry) else {
            self.missing.insert(key.to_owned());
            return None;
        };
        let valid_len = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4));
        if width == 0 || height == 0 || valid_len != Some(rgba.len()) {
            self.missing.insert(key.to_owned());
            return None;
        }

        let image = ColorImage::from_rgba_unmultiplied([width, height], &rgba);
        let texture = ctx.load_texture(
            format!("app-icon:{key}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
        self.textures.insert(key.to_owned(), texture);
        Some(id)
    }
}

/// Icons are shared between apps that name the same file, so the icon name
/// (falling back to the exec command) is the cache key.
fn icon_key(entry: &AppEntry) -> &str {
    let icon = entry.icon.trim();
    if icon.is_empty() {
        &entry.exec
    } else {
        icon
    }
}

/// The letter shown inside the placeholder circle when an app has no icon.
pub(super) fn initial(name: &str) -> char {
    name.chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?')
}
