//! macOS-only browser control via AppleScript.
//!
//! This is the Safari story — Safari has no cross-browser extension model we
//! can ship without an Xcode container app, but it *does* expose its front tab
//! to Apple Events. Reading the URL and writing a replacement is exactly the
//! "rewrite the address bar" trick blocking tools use.
//!
//! Chromium browsers get the same treatment as a fallback, so web blocking
//! still works on macOS before the user installs the extension.
//!
//! Two constraints shape the implementation:
//!
//! * `tell application "Safari"` **launches** Safari if it isn't running, so
//!   these functions may only be aimed at a browser already known to be up.
//!   The engine satisfies that by only ever driving the *frontmost*
//!   application.
//! * The first Apple Event to a given app raises a system permission prompt
//!   that blocks `osascript` until the user answers. Every invocation is
//!   therefore bounded by a timeout so one unanswered dialog can't wedge the
//!   engine.
//!
//! Requires `NSAppleEventsUsageDescription` in `Info.plist` and the
//! `com.apple.security.automation.apple-events` entitlement; without both,
//! hardened-runtime builds are denied outright.

/// A browser we know how to drive through Apple Events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserApp {
    pub bundle_id: &'static str,
    /// Name as AppleScript knows it — must match the app's bundle name.
    pub app_name: &'static str,
    /// Chromium-family browsers say `active tab`, Safari says `current tab`.
    pub chromium: bool,
}

pub const BROWSERS: &[BrowserApp] = &[
    BrowserApp {
        bundle_id: "com.apple.Safari",
        app_name: "Safari",
        chromium: false,
    },
    BrowserApp {
        bundle_id: "com.google.Chrome",
        app_name: "Google Chrome",
        chromium: true,
    },
    BrowserApp {
        bundle_id: "com.microsoft.edgemac",
        app_name: "Microsoft Edge",
        chromium: true,
    },
    BrowserApp {
        bundle_id: "com.brave.Browser",
        app_name: "Brave Browser",
        chromium: true,
    },
    BrowserApp {
        bundle_id: "com.vivaldi.Vivaldi",
        app_name: "Vivaldi",
        chromium: true,
    },
];

/// Look up a browser by the bundle identifier of the foreground app.
pub fn browser_for_bundle(bundle_id: &str) -> Option<&'static BrowserApp> {
    BROWSERS
        .iter()
        .find(|b| b.bundle_id.eq_ignore_ascii_case(bundle_id))
}

/// Escape a string for embedding in an AppleScript literal. Only backslash
/// and double-quote are special.
pub fn applescript_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Script that reads the front window's active tab URL.
pub fn read_script(browser: &BrowserApp) -> String {
    let tab = if browser.chromium {
        "active tab"
    } else {
        "current tab"
    };
    format!(
        "tell application \"{}\" to return URL of {} of front window",
        applescript_escape(browser.app_name),
        tab
    )
}

/// Script that replaces the front window's active tab URL.
pub fn write_script(browser: &BrowserApp, url: &str) -> String {
    let tab = if browser.chromium {
        "active tab"
    } else {
        "current tab"
    };
    format!(
        "tell application \"{}\" to set URL of {} of front window to \"{}\"",
        applescript_escape(browser.app_name),
        tab,
        applescript_escape(url)
    )
}

#[cfg(target_os = "macos")]
mod imp {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// Longest we let a single Apple Event block the engine tick. Generous
    /// enough for a cold app, short enough that an unanswered permission
    /// dialog doesn't freeze focus enforcement.
    const OSASCRIPT_TIMEOUT: Duration = Duration::from_millis(1500);
    const POLL_INTERVAL: Duration = Duration::from_millis(25);

    /// Run an AppleScript to completion, bounded by [`OSASCRIPT_TIMEOUT`].
    ///
    /// A timeout kills the child and yields `None` — callers treat that as
    /// "couldn't tell", which is the safe direction: we never block a page we
    /// failed to inspect.
    fn exec(script: &str) -> Option<std::process::Output> {
        let mut child = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;

        let deadline = Instant::now() + OSASCRIPT_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        tracing::debug!("focus: osascript timed out");
                        return None;
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(_) => return None,
            }
        }

        child.wait_with_output().ok()
    }

    /// Trimmed stdout, or `None` when the script failed or printed nothing.
    pub fn run_osascript(script: &str) -> Option<String> {
        let output = exec(script)?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Did the script run cleanly? For scripts whose success is an empty
    /// stdout, which [`run_osascript`] cannot distinguish from failure.
    pub fn run_osascript_ok(script: &str) -> bool {
        exec(script).is_some_and(|output| output.status.success())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn run_osascript(_script: &str) -> Option<String> {
        None
    }
    pub fn run_osascript_ok(_script: &str) -> bool {
        false
    }
}

/// URL of the front window's active tab, or `None` if the browser has no
/// window or refused the Apple Event.
///
/// **The caller must already know the browser is running.** Every caller
/// reaches this through [`browser_for_bundle`] on the *frontmost* application,
/// which is proof enough; probing again would enumerate every running
/// application once per second for no new information. The guard still matters
/// in principle — `tell application "Safari"` launches Safari — but a browser
/// that is in the foreground is, definitionally, already up.
pub fn read_active_url(browser: &BrowserApp) -> Option<String> {
    imp::run_osascript(&read_script(browser))
}

/// Point the front window's active tab at `url`. Same precondition as
/// [`read_active_url`].
///
/// Reports the script's exit status rather than re-reading the tab to confirm.
/// The re-read cost a second Apple Event and lied anyway: the tab has not
/// committed the new URL by the time the write returns, so a successful
/// redirect routinely looked like a failure.
pub fn set_active_url(browser: &BrowserApp, url: &str) -> bool {
    imp::run_osascript_ok(&write_script(browser, url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safari_and_chromium_use_different_tab_nouns() {
        let safari = browser_for_bundle("com.apple.Safari").unwrap();
        let chrome = browser_for_bundle("com.google.Chrome").unwrap();
        assert!(read_script(safari).contains("current tab"));
        assert!(read_script(chrome).contains("active tab"));
    }

    #[test]
    fn bundle_lookup_is_case_insensitive() {
        assert!(browser_for_bundle("COM.APPLE.SAFARI").is_some());
        assert!(browser_for_bundle("com.example.NotABrowser").is_none());
    }

    /// Quotes that AppleScript would treat as literal delimiters, i.e. those
    /// not preceded by a backslash.
    fn unescaped_quotes(s: &str) -> usize {
        let bytes = s.as_bytes();
        (0..bytes.len())
            .filter(|&i| bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\\'))
            .count()
    }

    #[test]
    fn quotes_in_the_url_cannot_escape_the_applescript_literal() {
        let safari = browser_for_bundle("com.apple.Safari").unwrap();
        let hostile = "http://x/\" & (do shell script \"boom\") & \"";
        let script = write_script(safari, hostile);
        assert!(script.contains(&applescript_escape(hostile)));
        // Exactly four delimiters survive: around the app name and around the
        // URL. Anything more would mean the payload broke out.
        assert_eq!(unescaped_quotes(&script), 4);
    }

    #[test]
    fn backslashes_are_escaped_before_quotes() {
        assert_eq!(applescript_escape(r#"a\"b"#), r#"a\\\"b"#);
    }
}
