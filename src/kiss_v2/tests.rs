use super::data_handler::DataHandler;
use super::normalizer::StringNormalizer;
use super::platform::{InstalledApp, Platform};
use super::scorer::{FuzzyScorer, MatchTier};
use std::sync::Arc;

#[derive(Debug)]
struct MockPlatform {
    apps: Vec<InstalledApp>,
}

impl Platform for MockPlatform {
    fn list_applications(&self) -> Vec<InstalledApp> {
        self.apps.clone()
    }
    fn launch_application(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn uninstall_application(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_app_info(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    fn view_in_store(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn test_normalizer_accents_and_hyphens() {
    let res = StringNormalizer::normalize("Wi-Fi Écran Örebro", true);
    assert_eq!(res.text, "wifi ecran orebro");
}

#[test]
fn test_scorer_exact_and_prefix() {
    let exact = FuzzyScorer::score("calc", "calc").expect("exact match");
    assert_eq!(exact.tier, MatchTier::ExactMatch);

    let prefix = FuzzyScorer::score("calc", "calculator").expect("prefix match");
    assert_eq!(prefix.tier, MatchTier::PrefixMatch);
    assert!(exact.score > prefix.score);
}

#[test]
fn test_data_handler_search_and_ranking() {
    let mock = Arc::new(MockPlatform {
        apps: vec![
            InstalledApp {
                id: "com.android.calculator2".into(),
                name: "Calculator".into(),
                package_name: "com.android.calculator2".into(),
                activity_name: None,
                icon_path: None,
                is_system: true,
            },
            InstalledApp {
                id: "com.google.calendar".into(),
                name: "Calendar".into(),
                package_name: "com.google.calendar".into(),
                activity_name: None,
                icon_path: None,
                is_system: false,
            },
            InstalledApp {
                id: "org.mozilla.firefox".into(),
                name: "Firefox".into(),
                package_name: "org.mozilla.firefox".into(),
                activity_name: None,
                icon_path: None,
                is_system: false,
            },
        ],
    });

    let mut handler = DataHandler::new(mock);
    let results = handler.search("calc", 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.app.name, "Calculator");

    // Launch boost
    handler.record_launch("com.google.calendar", 100);
    let empty_search = handler.search("", 10);
    assert_eq!(empty_search[0].item.app.name, "Calendar");
}
