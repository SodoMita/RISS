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
        ("Brave", "https://search.brave.com/search?q={}"),
        (
            "Wikipedia",
            "https://en.wikipedia.org/w/index.php?search={}",
        ),
        ("GitHub", "https://github.com/search?q={}"),
        ("YouTube", "https://www.youtube.com/results?search_query={}"),
        ("Stack Overflow", "https://stackoverflow.com/search?q={}"),
        ("MDN", "https://developer.mozilla.org/en-US/search?q={}"),
        ("Maps", "https://www.openstreetmap.org/search?query={}"),
        ("Ecosia", "https://www.ecosia.org/search?q={}"),
    ]
}

/// Resolve the URL template for a provider, taking user-defined providers
/// into account.
///
/// Custom entries come from the `custom-search-provider-add` setting as
/// comma-separated `Name|https://…{}` pairs; the placeholder may be `{}` or
/// `%s`. Unknown names fall back to the built-in list.
pub fn provider_template(name: &str, custom: &str) -> Option<String> {
    for item in custom.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some((custom_name, template)) = item.split_once('|') {
            if custom_name.trim().eq_ignore_ascii_case(name) {
                return Some(template.trim().to_string());
            }
        }
    }
    builtin_web_providers()
        .into_iter()
        .find(|(built_in, _)| built_in.eq_ignore_ascii_case(name))
        .map(|(_, template)| template.to_string())
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

/// Locate `program` on `PATH`, returning its full path.
#[cfg(not(target_os = "android"))]
pub fn find_in_path(program: &str) -> Option<std::path::PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).map(|dir| dir.join(program)).find(is_executable_file)
}

#[cfg(not(target_os = "android"))]
fn is_executable_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Is `program` available on `PATH`?
#[cfg(not(target_os = "android"))]
pub fn command_exists(program: &str) -> bool {
    find_in_path(program).is_some()
}

/// Run a command inside a terminal emulator so the user can follow
/// interactive prompts, e.g. the `sudo` password entry used when `pkexec`
/// is unavailable for privileged uninstalls.
#[cfg(not(target_os = "android"))]
pub fn run_in_terminal(argv: &[String]) -> Result<(), String> {
    // (terminal, flags preceding the command) — every flag variant passes
    // the remaining arguments as the program to execute.
    const TERMINALS: &[(&str, &[&str])] = &[
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("mate-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-x"]),
        ("terminator", &["-x"]),
        ("alacritty", &["-e"]),
        ("foot", &["-e"]),
        ("kitty", &[]),
        ("xterm", &["-e"]),
        ("wezterm", &["start", "--"]),
    ];
    for (terminal, flags) in TERMINALS {
        if find_in_path(terminal).is_none() {
            continue;
        }
        let mut full: Vec<String> = vec![(*terminal).to_string()];
        full.extend(flags.iter().map(|flag| (*flag).to_string()));
        full.extend(argv.iter().cloned());
        let Some((program, args)) = full.split_first() else {
            continue;
        };
        if std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }
    Err(format!("No terminal emulator found to run {}", argv.join(" ")))
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

/// Open a file or folder with the system default handler.
pub fn open_path(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let _ = path;
        Err("Opening files is not supported here".to_string())
    }

    #[cfg(not(target_os = "android"))]
    {
        open::that(path).map_err(|e| format!("Could not open {}: {}", path.display(), e))
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
        assert_eq!(
            provider_url("https://example.org/", "hi"),
            "https://example.org/hi"
        );
    }

    #[test]
    fn resolves_provider_templates() {
        assert_eq!(
            provider_template("duckduckgo", ""),
            Some("https://duckduckgo.com/?q={}".to_string())
        );
        assert_eq!(
            provider_template("Brave", ""),
            Some("https://search.brave.com/search?q={}".to_string())
        );
        assert_eq!(
            provider_template("mine", "Mine|https://example.org/find?%s"),
            Some("https://example.org/find?%s".to_string())
        );
        assert_eq!(provider_template("unknown", ""), None);
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

    #[test]
    fn detects_math() {
        assert!(looks_like_math("2+2"));
        assert!(!looks_like_math("firefox"));
    }
}
