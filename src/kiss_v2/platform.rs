//! Platform Trait - Clean separation of host environment from launcher logic
//!
//! Allows pure Rust unit testing, Linux desktop integration, and Android Native/Host bridge.

use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub package_name: String,
    pub activity_name: Option<String>,
    pub icon_path: Option<String>,
    pub is_system: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactEntry {
    pub id: String,
    pub name: String,
    pub phone_number: String,
}

pub trait Platform: Send + Sync + Debug {
    /// Retrieve all installed applications
    fn list_applications(&self) -> Vec<InstalledApp>;

    /// Launch application by identifier or intent
    fn launch_application(&self, app_id: &str) -> Result<(), String>;

    /// Request application uninstallation
    fn uninstall_application(&self, app_id: &str) -> Result<(), String>;

    /// Open system app settings / info
    fn open_app_info(&self, app_id: &str) -> Result<(), String>;

    /// Open app in Google Play / store
    fn view_in_store(&self, app_id: &str) -> Result<(), String>;

    /// Optional contacts provider
    fn list_contacts(&self) -> Result<Vec<ContactEntry>, String> {
        Ok(Vec::new())
    }

    /// Check if wallpaper transparency is supported by windowing environment
    fn supports_transparency(&self) -> bool {
        true
    }
}
