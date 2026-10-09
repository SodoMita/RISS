//! Launch providers: web search, shell commands, timers and small OS helpers.
//!
//! These mirror the optional search providers of the original KISS launcher
//! (web search, `exec`, timer) and are kept dependency free: opening URLs and
//! running commands is done with the tools already available on the system.

/// Web search providers offered out of the box.
pub fn builtin_web_providers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("DuckDuckGo", "https://duckduckgo.com/?q={}"),
        ("Google", "https://www.google.com/search?q={}"),
        ("Bing", "https://www.bing.com/search?q={}"),
        ("Wikipedia", "https://en.wikipedia.org/w/index.php?search={}"),
        ("GitHub", "https://github.com/search?q={}"),
        ("YouTube", "https://www.youtube.com/results?search_query={}"),
        ("Stack Overflow", "https://stackoverflow.com/search?q={}"),
        ("MDN", "https://developer.mozilla.org/en-US/search?q={}"),
        ("Maps", "https://www.openstreetmap.org/search?query={}"),
        ("Ecosia", "https://www.ecosia.org/search?q={}"),
    ]
}

/// Build the final URL for a provider template.
///
/// Supports `{}` and `%s` placeholders; templates without a placeholder get
/// the query appended, which is what users expect from simple custom engines.
pub fn provider_url(template: &str, query: &str) -> String {
    let encoded = url_encode(query);
    if template.contains("{}") {
        template.replace("{}", &encoded)
    } else if template.contains("%s") {
        template.replace("%s", &encoded)
    } else {
        format!("{}{}", template, encoded)
    }
}

/// Percent-encode a query string, keeping the characters people type.
pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let c = *byte as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | ':' | '/' | '+') {
            out.push(c);
        } else if c == ' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

/// Open a URL with the system default handler.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        crate::android_app_entry::android_jni::open_url_jni(url)
    }

    #[cfg(not(target_os = "android"))]
    {
        open::that(url).map_err(|e| format!("Could not open {}: {}", url, e))
    }
}

/// Run a shell command in the background (KISS' `exec` provider).
pub fn run_command(command: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let _ = command;
        Err("Command execution is not supported here".to_string())
    }

    #[cfg(not(target_os = "android"))]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not run {}: {}", command, e))
    }
}

/// Copy text to the clipboard using whichever helper is installed.
pub fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "android")]
    {
        let _ = text;
        false
    }

    #[cfg(not(target_os = "android"))]
    {
        let candidates: [(&str, Vec<&str>); 3] = [
            ("wl-copy", vec![]),
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
        ];
        for (program, args) in candidates {
            let spawned = std::process::Command::new(program)
                .args(&args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            if let Ok(mut child) = spawned {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let written = stdin.write_all(text.as_bytes()).is_ok();
                    drop(stdin);
                    if written && child.wait().map(|s| s.success()).unwrap_or(false) {
                        return true;
                    }
                } else {
                    let _ = child.kill();
                }
            }
        }
        false
    }
}

/// Parse a duration such as `5`, `30s`, `10m`, `1h` or `sleep 5 minutes`.
pub fn parse_timer(query: &str) -> Option<u64> {
    let mut text = query.trim().to_lowercase();
    for prefix in ["sleep ", "timer ", "wait ", "in "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim().to_string();
        }
    }
    let text = text.trim_end_matches(&['s', ' '][..]);
    let mut digits = String::new();
    let mut unit = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if !c.is_whitespace() {
            unit.push(c);
        }
    }
    let value: u64 = digits.parse().ok()?;
    if value == 0 {
        return None;
    }
    let unit = unit.trim();
    let multiplier = match unit {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => 1,
        "m" | "min" | "mins" | "minute" | "minutes" => 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
        _ => return None,
    };
    let seconds = value.saturating_mul(multiplier);
    if seconds == 0 || seconds > 24 * 3600 {
        None
    } else {
        Some(seconds)
    }
}

/// `95` becomes `1m 35s`, used by the timer provider.
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let rest = seconds % 60;
    match (hours, minutes, rest) {
        (0, 0, s) => format!("{}s", s),
        (0, m, 0) => format!("{}m", m),
        (0, m, s) => format!("{}m {}s", m, s),
        (h, 0, 0) => format!("{}h", h),
        (h, m, 0) => format!("{}h {}m", h, m),
        (h, m, s) => format!("{}h {}m {}s", h, m, s),
    }
}

/// Which generic launcher action does this query look like?
pub fn looks_like_math(query: &str) -> bool {
    query.chars().any(|c| "+-*/^%".contains(c)) && query.chars().any(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_urls() {
        assert_eq!(
            provider_url("https://duckduckgo.com/?q={}", "rust & egui"),
            "https://duckduckgo.com/?q=rust+%26+egui"
        );
        assert_eq!(
            provider_url("https://example.org/search?q=%s", "hi"),
            "https://example.org/search?q=hi"
        );
        assert_eq!(provider_url("https://example.org/", "hi"), "https://example.org/hi");
    }

    #[test]
    fn parses_timers() {
        assert_eq!(parse_timer("sleep 5"), Some(5));
        assert_eq!(parse_timer("30s"), Some(30));
        assert_eq!(parse_timer("10 minutes"), Some(600));
        assert_eq!(parse_timer("1h"), Some(3600));
        assert_eq!(parse_timer("0"), None);
        assert_eq!(parse_timer("firefox"), None);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(5), "5s");
        assert_eq!(format_duration(65), "1m 5s");
        assert_eq!(format_duration(3600), "1h");
    }
}