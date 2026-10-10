//! On-screen keyboard visibility over the D-Bus session bus.
//!
//! Sources:
//! - **KWin** (Plasma, Plasma Mobile): `org.kde.KWin` `/VirtualKeyboard`,
//!   interface `org.kde.kwin.VirtualKeyboard`, property `visible`. KWin
//!   announces changes with its own `visibleChanged` signal rather than
//!   `PropertiesChanged`.
//! - **`sm.puri.OSK0`** (squeekboard on Phosh, sway and others): property
//!   `Visible`, announced with the standard `PropertiesChanged` signal.
//!
//! A single background thread subscribes to those signals (plus owner changes
//! of both names, so a keyboard that quits does not stay "visible"), re-reads
//! the properties whenever one fires and asks egui to repaint on changes.

use super::{OskVisibility, SharedVisibility};
use eframe::egui;
use zbus::blocking::{fdo::DBusProxy, Connection, MessageIterator};
use zbus::message::Type;
use zbus::names::BusName;
use zbus::zvariant::OwnedValue;
use zbus::MatchRule;

/// A boolean "keyboard is visible" property exported by a keyboard service.
struct Source {
    service: &'static str,
    path: &'static str,
    interface: &'static str,
    property: &'static str,
}

const SOURCES: [Source; 2] = [
    Source {
        service: "org.kde.KWin",
        path: "/VirtualKeyboard",
        interface: "org.kde.kwin.VirtualKeyboard",
        property: "visible",
    },
    Source {
        service: "sm.puri.OSK0",
        path: "/sm/puri/OSK0",
        interface: "sm.puri.OSK0",
        property: "Visible",
    },
];

const MATCH_RULES: [&str; 4] = [
    "type='signal',interface='org.kde.kwin.VirtualKeyboard',path='/VirtualKeyboard',member='visibleChanged'",
    "type='signal',interface='org.freedesktop.DBus.Properties',path='/sm/puri/OSK0',member='PropertiesChanged'",
    "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='org.kde.KWin'",
    "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='sm.puri.OSK0'",
];

pub fn spawn_watcher(ctx: egui::Context, shared: SharedVisibility) {
    let spawned = std::thread::Builder::new()
        .name("riss-osk-dbus".into())
        .spawn(move || {
            if let Err(err) = watch(&ctx, &shared) {
                // No session bus (e.g. a bare compositor): fall back to the
                // touch heuristic, which `Unknown` already selects.
                log::info!("On-screen keyboard detection over D-Bus unavailable: {err}");
            }
        });
    if let Err(err) = spawned {
        log::error!("Could not start the on-screen keyboard watcher: {err}");
    }
}

fn watch(ctx: &egui::Context, shared: &SharedVisibility) -> zbus::Result<()> {
    let conn = Connection::session()?;
    // Start listening before the first query so no change slips in between.
    let messages = MessageIterator::from(&conn);
    let bus = DBusProxy::new(&conn)?;
    for rule in MATCH_RULES {
        bus.add_match_rule(MatchRule::try_from(rule)?)?;
    }
    publish(ctx, shared, query(&conn, &bus));
    // Only the subscribed signals arrive as signals (plus the bus's own
    // NameAcquired, which merely causes one extra query).
    for message in messages {
        if message?.message_type() == Type::Signal {
            publish(ctx, shared, query(&conn, &bus));
        }
    }
    Ok(())
}

fn query(conn: &Connection, bus: &DBusProxy<'_>) -> OskVisibility {
    OskVisibility::combine(SOURCES.iter().map(|source| read(conn, bus, source)))
}

/// Read a source's visibility; `None` if the service is not running.
fn read(conn: &Connection, bus: &DBusProxy<'_>, source: &Source) -> Option<bool> {
    // Check ownership first: a plain method call could D-Bus-activate a
    // keyboard the user did not start.
    let name = BusName::try_from(source.service).ok()?;
    if !bus.name_has_owner(name).ok()? {
        return None;
    }
    let reply = conn
        .call_method(
            Some(source.service),
            source.path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(source.interface, source.property),
        )
        .ok()?;
    let value: OwnedValue = reply.body().deserialize().ok()?;
    bool::try_from(value).ok()
}

fn publish(ctx: &egui::Context, shared: &SharedVisibility, visibility: OskVisibility) {
    let changed = match shared.lock() {
        Ok(mut current) => std::mem::replace(&mut *current, visibility) != visibility,
        Err(_) => false,
    };
    if changed {
        log::debug!("On-screen keyboard visibility: {visibility:?}");
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_rules_parse() {
        for rule in MATCH_RULES {
            assert!(MatchRule::try_from(rule).is_ok(), "invalid rule: {rule}");
        }
    }
}
