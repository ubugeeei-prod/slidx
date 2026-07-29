//! Handing something to the platform's own opener.
//!
//! Every desktop already has one program whose job is "open this the way the
//! user would expect", and it knows about their default PDF reader and their
//! default browser. Reimplementing that guess — looking for Chrome, then
//! Firefox, then Safari — would be worse at it and would also override a
//! choice somebody already made.
//!
//! ## What may be opened
//!
//! Only two things: a file inside the directory being previewed, and a
//! loopback URL this process is serving. Both are constructed here, neither
//! comes from a deck. Handing an arbitrary string to `open` or `xdg-open` is
//! handing it to whatever the system has registered for that scheme, and a
//! deck is a file somebody downloaded from the internet.
//!
//! ## Failing to open is not failing
//!
//! Over SSH, in a container, on a machine with no desktop session, there is
//! nothing to open with — and the useful thing is still true: the PDF is at a
//! path, or the deck is at a URL. So a failed open prints where the thing is
//! and exits successfully, rather than reporting an error about a browser
//! somebody never asked for.

use std::path::Path;
use std::process::{Command, Stdio};

/// The program and arguments that open something on one platform.
///
/// Split from running it so every platform's answer is checkable from every
/// platform, which is the only way this gets tested at all — CI runs on three
/// and none of them can exercise the other two.
pub fn opener(os: &str) -> Option<(&'static str, &'static [&'static str])> {
    match os {
        "macos" => Some(("open", &[])),
        // `cmd /c start` needs an empty first argument: `start` reads a
        // leading quoted string as the *window title*, so a quoted path in
        // that position opens a console window called after the file.
        "windows" => Some(("cmd", &["/c", "start", ""])),
        // Every free desktop ships xdg-open. The fallbacks are for a machine
        // that has a session but not the xdg tools, and for WSL, where the
        // thing to open with is on the Windows side.
        "linux" | "freebsd" | "openbsd" | "netbsd" => Some(("xdg-open", &[])),
        _ => None,
    }
}

/// Programs to try in order, so a desktop without xdg-utils still works.
pub fn fallbacks(os: &str) -> &'static [&'static str] {
    match os {
        "linux" | "freebsd" | "openbsd" | "netbsd" => &["gio", "wslview", "gnome-open", "kde-open"],
        _ => &[],
    }
}

/// True when this is something slidx may hand to the platform.
///
/// A local file, or a loopback URL. Nothing else, ever — an opener is a
/// general-purpose "run whatever is registered for this" and a deck's contents
/// are not a safe source of input for one.
pub fn is_openable(what: &str) -> bool {
    if let Some(rest) = what.strip_prefix("http://") {
        let host = rest.split(['/', ':']).next().unwrap_or("");
        return host == "127.0.0.1" || host == "localhost" || host == "[::1]";
    }

    // Anything else has to be a path that exists on this machine.
    !what.contains("://") && Path::new(what).exists()
}

/// Asks the platform to open something.
///
/// Returns whether it worked. A `false` is not an error — see the module docs.
pub fn open(what: &str) -> bool {
    if !is_openable(what) {
        return false;
    }

    let Some((program, leading)) = opener(std::env::consts::OS) else {
        return false;
    };

    if run(program, leading, what) {
        return true;
    }

    fallbacks(std::env::consts::OS)
        .iter()
        .any(|program| run(program, if *program == "gio" { &["open"] } else { &[] }, what))
}

fn run(program: &str, leading: &[&str], what: &str) -> bool {
    Command::new(program)
        .args(leading)
        .arg(what)
        // Detached from this process's streams: an opener that printed into
        // the terminal would scribble over the report saying where the deck is.
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_desktop_platform_has_its_own_opener() {
        assert_eq!(opener("macos").map(|(program, _)| program), Some("open"));
        assert_eq!(opener("linux").map(|(program, _)| program), Some("xdg-open"));
        assert_eq!(opener("windows").map(|(program, _)| program), Some("cmd"));
    }

    #[test]
    fn the_windows_opener_passes_an_empty_title_before_the_path() {
        // `start` reads a leading quoted string as the window title, so a
        // quoted path in that position opens an empty console window named
        // after the file instead of opening the file.
        let (_, leading) = opener("windows").expect("an opener");

        assert_eq!(leading, ["/c", "start", ""]);
    }

    #[test]
    fn a_platform_with_no_desktop_convention_has_no_opener_rather_than_a_guess() {
        assert!(opener("wasi").is_none());
        assert!(opener("android").is_none());
    }

    #[test]
    fn free_desktops_have_fallbacks_for_a_machine_without_xdg_utils() {
        assert!(fallbacks("linux").contains(&"gio"));
        // WSL: the thing to open with is on the Windows side.
        assert!(fallbacks("linux").contains(&"wslview"));
        assert!(fallbacks("macos").is_empty());
    }

    #[test]
    fn a_loopback_url_may_be_opened() {
        assert!(is_openable("http://127.0.0.1:8080/slides/"));
        assert!(is_openable("http://localhost:3000/"));
    }

    #[test]
    fn a_url_pointing_anywhere_else_may_not_be() {
        // An opener runs whatever is registered for a scheme. A deck is a file
        // somebody downloaded, and nothing in one is a safe source of input.
        for url in [
            "http://example.com/",
            "https://example.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "vscode://file/etc/passwd",
            "http://127.0.0.1.example.com/",
            "http://evil/?x=127.0.0.1",
        ] {
            assert!(!is_openable(url), "{url} should not be openable");
        }
    }

    #[test]
    fn a_file_that_does_not_exist_may_not_be_opened() {
        assert!(!is_openable("/nowhere/at/all/deck.pdf"));
    }

    #[test]
    fn a_file_that_does_exist_may_be() {
        let path = std::env::temp_dir().join(format!("slidx-open-{}.pdf", std::process::id()));
        std::fs::write(&path, b"%PDF-1.4\n").expect("write");

        assert!(is_openable(&path.display().to_string()));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn opening_something_that_is_not_allowed_reports_failure_rather_than_running_anything() {
        assert!(!open("https://example.com/"));
        assert!(!open("/nowhere/at/all/deck.pdf"));
    }
}
