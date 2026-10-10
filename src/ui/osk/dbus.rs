//! Keyboard visibility from the session bus:
//! - KWin: `org.kde.KWin /VirtualKeyboard`, property `visible`, announced by
//!   its own `visibleChanged` signal;
//! - `sm.puri.OSK0` (squeekboard on Phosh, sway, …): property `Visible`,
//!   announced by `PropertiesChanged`.
//!
//! One thread waits for those signals (and owner changes of both names) and
//! re-reads the properties whenever one arrives.

use super::OskVisibility;
use eframe::egui;
use std::sync::{Arc, Mutex};
use zbus::blocking::{fdo::DBusProxy, Connection, MessageIterator};
use zbus::{message::Type, names::BusName, zvariant::OwnedValue, MatchRule};

/// (service, object path, interface, property)
const SOURCES: [(&str, &str, &str, &str); 2] = [
    (
        "org.kde.KWin",
        "/VirtualKeyboard",
        "org.kde.kwin.VirtualKeyboard",
        "visible",
    ),
    ("sm.puri.OSK0", "/sm/puri/OSK0", "sm.puri.OSK0", "Visible"),
];

const MATCH_RULES: [&str; 4] = [
    "type='signal',interface='org.kde.kwin.VirtualKeyboard',member='visibleChanged'",
    "type='signal',interface='org.freedesktop.DBus.Properties',path='/sm/puri/OSK0'",
    "type='signal',member='NameOwnerChanged',arg0='org.kde.KWin'",
    "type='signal',member='NameOwnerChanged',arg0='sm.puri.OSK0'",
];

pub fn spawn_watcher(ctx: egui::Context, shared: Arc<Mutex<OskVisibility>>) {
    std::thread::spawn(move || {
        if let Err(err) = watch(&ctx, &shared) {
            log::info!("No on-screen keyboard detection over D-Bus: {err}");
        }
    });
}

fn watch(ctx: &egui::Context, shared: &Mutex<OskVisibility>) -> zbus::Result<()> {
    let conn = Connection::session()?;
    let messages = MessageIterator::from(&conn); // Listen before the first read.
    let bus = DBusProxy::new(&conn)?;
    for rule in MATCH_RULES {
        bus.add_match_rule(MatchRule::try_from(rule)?)?;
    }
    let publish = || {
        let now = OskVisibility::combine(SOURCES.map(|source| read(&conn, &bus, source)));
        if shared
            .lock()
            .is_ok_and(|mut v| std::mem::replace(&mut *v, now) != now)
        {
            ctx.request_repaint();
        }
    };
    publish();
    for message in messages {
        if message?.message_type() == Type::Signal {
            publish();
        }
    }
    Ok(())
}

/// `None` if the service is not running. Ownership is checked first so a
/// keyboard the user did not start is never D-Bus-activated.
fn read(
    conn: &Connection,
    bus: &DBusProxy,
    (service, path, iface, prop): (&str, &str, &str, &str),
) -> Option<bool> {
    if !bus.name_has_owner(BusName::try_from(service).ok()?).ok()? {
        return None;
    }
    let props = Some("org.freedesktop.DBus.Properties");
    let reply = conn
        .call_method(Some(service), path, props, "Get", &(iface, prop))
        .ok()?;
    bool::try_from(reply.body().deserialize::<OwnedValue>().ok()?).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn match_rules_parse() {
        for rule in super::MATCH_RULES {
            assert!(zbus::MatchRule::try_from(rule).is_ok(), "{rule}");
        }
    }
}
