//! What is attached, and whether it is showing the same thing twice.
//!
//! Three tools that agree on nothing: `system_profiler` prints an indented
//! outline, `xrandr` prints one line per output, and Windows has to be asked
//! through .NET. The mirroring answer is the one that differs most, and the
//! differences are facts about the platforms rather than gaps in the parsing:
//!
//! - macOS says it outright, per display, in a `Mirror:` line.
//! - X11 does not say it, but it *shows* it: mirrored outputs are placed at the
//!   same origin, which is precisely what `xrandr --same-as` does.
//! - Windows will not say. Duplicated monitors collapse into one logical
//!   screen, so a single screen there could equally be a laptop on its own —
//!   and that is reported as "cannot tell" rather than guessed either way.
//!
//! Every branch is dispatched on a [`Platform`] value and every tool comes
//! through [`Tools`], so all three parse on all three runners.

use crate::environment::{Display, Displays, Platform, Reading, Resolution};
use crate::probe::tools::{self, Tools};

/// Asks Windows through .NET, because nothing on the command line will say.
///
/// `Screen.AllScreens` is the logical desktop rather than the monitor list,
/// which is the whole reason the mirroring answer from Windows is a shrug: in
/// duplicate mode several monitors share one entry here.
const WINDOWS_SCREENS: &str = "Add-Type -AssemblyName System.Windows.Forms; \
     [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { \
     '{0} {1} {2} {3}' -f $_.DeviceName, $_.Bounds.Width, $_.Bounds.Height, $_.Primary }";

pub fn read(platform: Platform, tools: &Tools) -> Reading<Displays> {
    match platform {
        Platform::MacOs => tools::parsed(
            tools.output("system_profiler", &["SPDisplaysDataType"]),
            parse_system_profiler,
            "`system_profiler` described no display this build can read",
        ),
        Platform::Linux => tools::parsed(
            tools.output("xrandr", &["--query"]),
            parse_xrandr,
            "`xrandr` listed no connected output this build can read",
        ),
        Platform::Windows => tools::parsed(
            tools.output(
                "powershell",
                &["-NoProfile", "-NonInteractive", "-Command", WINDOWS_SCREENS],
            ),
            parse_windows_screens,
            "Windows described no screen this build can read",
        ),
        Platform::Unknown => {
            Reading::unavailable("slidx has no way to list the displays on this platform")
        }
    }
}

/// Parses the `Displays:` section of `system_profiler SPDisplaysDataType`.
///
/// The outline is two spaces per level, so a display's name sits exactly two
/// columns inside the `Displays:` heading and its fields further in again.
/// Anchoring on that rather than on the field names means a machine with two
/// graphics cards, each with its own `Displays:` block, reads as one list.
///
/// A `Mirror:` line that is missing entirely is not the same as one reading
/// `Off`: the first means this output shape is not one slidx knows, and the
/// mirroring answer stays absent rather than becoming a cheerful no.
fn parse_system_profiler(output: &str) -> Option<Displays> {
    let mut screens: Vec<Display> = Vec::new();
    let mut mirror_seen = false;
    let mut mirroring = false;
    let mut section: Option<usize> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();

        // Anything at or outside the heading's own column has left the block.
        if section.is_some_and(|open| indent <= open) {
            section = None;
        }

        if trimmed == "Displays:" {
            section = Some(indent);
            continue;
        }

        let Some(open) = section else { continue };

        if indent == open + 2 {
            // A name and a colon, and nothing after it: a new display.
            if let Some(name) = trimmed.strip_suffix(':') {
                screens.push(Display::new(0, 0).named(name));
            }
            continue;
        }

        let Some((key, value)) = trimmed.split_once(": ") else { continue };
        let Some(screen) = screens.last_mut() else { continue };

        match key {
            "Resolution" => {
                if let Some(size) = dimensions(value) {
                    screen.pixels = size;
                }
            }
            // What the desktop is actually drawn at on a scaled panel. Absent
            // when macOS is using the display's default, which is why `points`
            // stays None rather than being filled in from the pixels.
            "UI Looks like" => screen.points = dimensions(value),
            "Main Display" => screen.primary = value.trim() == "Yes",
            "Mirror" => {
                mirror_seen = true;
                mirroring |= value.trim() == "On";
            }
            _ => {}
        }
    }

    screens.retain(|screen| !screen.pixels.is_empty());
    if screens.is_empty() {
        return None;
    }

    let displays = Displays::new(screens);
    Some(match (mirror_seen, mirroring) {
        (true, true) => displays.mirrored(),
        (true, false) => displays.extended(),
        (false, _) => displays,
    })
}

/// Parses `xrandr --query`, taking only the connected outputs.
///
/// ```text
/// eDP-1 connected primary 1920x1080+0+0 (normal left inverted right) 344mm x 193mm
/// HDMI-1 connected 1920x1080+1920+0 (normal left inverted right) 509mm x 286mm
/// ```
///
/// Two outputs sharing an origin are mirrored. That is not an inference about
/// what the user meant — placing both at the same position is exactly what
/// `xrandr --same-as` does, and it is the only thing "mirrored" means to X.
fn parse_xrandr(output: &str) -> Option<Displays> {
    let mut screens = Vec::new();
    let mut origins: Vec<(i64, i64)> = Vec::new();

    for line in output.lines() {
        // Mode lists are indented under their output; only the output lines
        // start at the left margin.
        if line.starts_with(char::is_whitespace) {
            continue;
        }

        let mut tokens = line.split_whitespace();
        let Some(name) = tokens.next() else { continue };
        if tokens.next() != Some("connected") {
            continue;
        }

        let mut rest = tokens.peekable();
        let primary = rest.peek() == Some(&"primary");
        if primary {
            rest.next();
        }

        let Some((size, origin)) = rest.next().and_then(geometry) else { continue };

        origins.push(origin);
        let mut screen = Display::new(size.width, size.height).named(name);
        screen.primary = primary;
        screens.push(screen);
    }

    if screens.is_empty() {
        return None;
    }

    let mut distinct = origins.clone();
    distinct.sort_unstable();
    distinct.dedup();

    let displays = Displays::new(screens);
    Some(if distinct.len() < origins.len() { displays.mirrored() } else { displays.extended() })
}

/// Parses one `1920x1080+1920+0` geometry into a size and a position.
fn geometry(token: &str) -> Option<(Resolution, (i64, i64))> {
    let (size, position) = token.split_once('+')?;
    let (left, top) = position.split_once('+')?;
    let dimensions = dimensions(&size.replace('x', " x "))?;

    Some((dimensions, (left.parse().ok()?, top.parse().ok()?)))
}

/// Parses `DeviceName Width Height Primary`, one line per logical screen.
///
/// The mirroring answer is deliberately left absent on one screen and only
/// claimed on two or more. Windows duplicate mode presents its monitors as a
/// single logical screen, so one entry could be a duplicated pair or a laptop
/// on its own — and those are different things to tell a speaker.
fn parse_windows_screens(output: &str) -> Option<Displays> {
    let mut screens = Vec::new();

    for line in output.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [name, width, height, primary] = fields[..] else { continue };

        let (Ok(width), Ok(height)) = (width.parse::<u32>(), height.parse::<u32>()) else {
            continue;
        };

        let mut screen = Display::new(width, height).named(name);
        screen.primary = primary.eq_ignore_ascii_case("true");
        screens.push(screen);
    }

    if screens.is_empty() {
        return None;
    }

    let several = screens.len() > 1;
    let displays = Displays::new(screens);

    Some(if several { displays.extended() } else { displays })
}

/// The first `1234 x 567` in a value, whatever surrounds it.
fn dimensions(value: &str) -> Option<Resolution> {
    let tokens: Vec<&str> = value.split_whitespace().collect();

    tokens.windows(3).find_map(|window| match window {
        [width, "x", height] => Some(Resolution::new(width.parse().ok()?, height.parse().ok()?)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MACOS: &str = "Graphics/Displays:

    Apple M3 Pro:

      Chipset Model: Apple M3 Pro
      Displays:
        Color LCD:
          Display Type: Built-in Liquid Retina XDR Display
          Resolution: 3024 x 1964 Retina
          UI Looks like: 1512 x 982 @ 120.00Hz
          Main Display: Yes
          Mirror: Off
          Online: Yes
        LG HDR WFHD:
          Resolution: 2560 x 1080 (UW-UXGA - Ultra Wide)
          UI Looks like: 2560 x 1080 @ 75.00Hz
          Mirror: Off
          Online: Yes
";

    const XRANDR: &str = "Screen 0: minimum 320 x 200, current 3840 x 1080, maximum 16384 x 16384
eDP-1 connected primary 1920x1080+0+0 (normal left inverted right) 344mm x 193mm
   1920x1080     60.01*+  59.97
   1680x1050     59.95
HDMI-1 connected 1920x1080+1920+0 (normal left inverted right) 509mm x 286mm
   1920x1080     60.00*+
DP-1 disconnected (normal left inverted right x axis y axis)
";

    #[test]
    fn every_platforms_display_reading_is_parsed_on_every_platform() {
        // The seam's whole reason for existing. Each branch is driven here by
        // its own canned output, on whichever runner happens to be running —
        // so a macOS parser that stopped working fails on Linux CI too.
        let cases = [
            (Platform::MacOs, MACOS),
            (Platform::Linux, XRANDR),
            (Platform::Windows, "\\\\.\\DISPLAY1 1920 1080 True\n"),
        ];

        for (platform, output) in cases {
            let tools = Tools::answering(move |_, _| Ok(output.to_string()));
            let reading = read(platform, &tools);

            assert!(reading.is_known(), "{platform:?} did not parse its own output");
        }
    }

    #[test]
    fn a_platform_slidx_cannot_ask_says_so_rather_than_reporting_no_displays() {
        // An empty display list and an unaskable platform look identical to a
        // check unless the reading itself keeps them apart.
        let reading = read(Platform::Unknown, &Tools::absent());

        assert!(!reading.is_known());
        assert!(reading.reason().is_some_and(|why| why.contains("no way")));
    }

    #[test]
    fn a_tool_that_is_not_installed_becomes_the_reason_the_line_reads_unknown() {
        let reading = read(Platform::Linux, &Tools::absent());

        assert!(reading.reason().is_some_and(|why| why.contains("xrandr")), "{reading:?}");
    }

    #[test]
    fn the_mac_outline_yields_both_screens_with_their_scaled_sizes() {
        let displays = parse_system_profiler(MACOS).unwrap_or_default();

        assert_eq!(displays.len(), 2);
        assert_eq!(displays.screens()[0].name.as_deref(), Some("Color LCD"));
        assert_eq!(displays.screens()[0].drawn_size(), Resolution::new(1512, 982));
        assert_eq!(displays.screens()[0].scale_percent(), Some(200));
        assert!(displays.screens()[0].primary);
        assert!(!displays.screens()[1].primary);
    }

    #[test]
    fn a_mac_reporting_mirror_on_for_any_screen_is_a_mirrored_arrangement() {
        let mirrored = MACOS.replace("Mirror: Off\n          Online: Yes\n        LG", "Mirror: On\n          Online: Yes\n        LG");
        let displays = parse_system_profiler(&mirrored).unwrap_or_default();

        assert_eq!(displays.is_mirrored(), Some(true));
    }

    #[test]
    fn a_mac_outline_with_no_mirror_line_at_all_claims_no_arrangement() {
        // An output shape this build does not recognise. Reading the absence as
        // "not mirrored" would be the cheerful guess this crate exists to
        // refuse.
        let quiet = MACOS.replace("          Mirror: Off\n", "");
        let displays = parse_system_profiler(&quiet).unwrap_or_default();

        assert_eq!(displays.len(), 2);
        assert_eq!(displays.is_mirrored(), None);
    }

    #[test]
    fn a_retina_panel_that_names_no_scaled_size_reports_its_pixels_and_no_scale() {
        // macOS omits `UI Looks like` when the display is at its default. The
        // pixels are still a real reading; the scale is not.
        let native = MACOS.replace("          UI Looks like: 1512 x 982 @ 120.00Hz\n", "");
        let displays = parse_system_profiler(&native).unwrap_or_default();

        assert_eq!(displays.screens()[0].points, None);
        assert_eq!(displays.screens()[0].drawn_size(), Resolution::new(3024, 1964));
    }

    #[test]
    fn a_mac_section_that_lists_no_display_is_unreadable_rather_than_empty() {
        assert!(parse_system_profiler("Graphics/Displays:\n\n    Apple M3 Pro:\n").is_none());
        assert!(parse_system_profiler("").is_none());
    }

    #[test]
    fn xrandr_yields_only_the_connected_outputs_and_skips_their_mode_lists() {
        // The mode list under each output is indented, and a disconnected
        // output is a line that must not become a screen.
        let displays = parse_xrandr(XRANDR).unwrap_or_default();

        assert_eq!(displays.labels(), ["eDP-1 1920x1080", "HDMI-1 1920x1080"]);
        assert!(displays.screens()[0].primary);
    }

    #[test]
    fn two_x_outputs_at_the_same_origin_are_mirrored_because_that_is_what_mirroring_is() {
        // `xrandr --same-as` places both outputs at one position. There is no
        // other thing that placement could mean.
        let same = XRANDR.replace("1920x1080+1920+0", "1920x1080+0+0");

        assert_eq!(parse_xrandr(&same).unwrap_or_default().is_mirrored(), Some(true));
        assert_eq!(parse_xrandr(XRANDR).unwrap_or_default().is_mirrored(), Some(false));
    }

    #[test]
    fn one_connected_x_output_is_an_extended_arrangement_with_nothing_to_mirror() {
        let alone = "eDP-1 connected primary 2560x1600+0+0 (normal) 344mm x 215mm\n";
        let displays = parse_xrandr(alone).unwrap_or_default();

        assert_eq!(displays.len(), 1);
        assert_eq!(displays.is_mirrored(), Some(false));
    }

    #[test]
    fn an_xrandr_run_that_found_nothing_connected_is_unreadable() {
        assert!(parse_xrandr("Screen 0: minimum 320 x 200\nDP-1 disconnected\n").is_none());
    }

    #[test]
    fn windows_reports_its_logical_screens_with_their_device_names() {
        let output = "\\\\.\\DISPLAY1 2560 1440 True\n\\\\.\\DISPLAY2 1920 1080 False\n";
        let displays = parse_windows_screens(output).unwrap_or_default();

        assert_eq!(displays.len(), 2);
        assert!(displays.screens()[0].primary);
        assert_eq!(displays.screens()[1].pixels, Resolution::new(1920, 1080));
    }

    #[test]
    fn one_windows_screen_will_not_say_whether_it_is_a_duplicated_pair() {
        // Duplicate mode collapses several monitors into one logical screen, so
        // a single entry is either a laptop on its own or a projector showing
        // the same thing. Those are different sentences to a speaker.
        let single = parse_windows_screens("\\\\.\\DISPLAY1 1920 1080 True\n").unwrap_or_default();
        let pair = parse_windows_screens(
            "\\\\.\\DISPLAY1 1920 1080 True\n\\\\.\\DISPLAY2 1920 1080 False\n",
        )
        .unwrap_or_default();

        assert_eq!(single.is_mirrored(), None);
        assert_eq!(pair.is_mirrored(), Some(false));
    }

    #[test]
    fn windows_output_that_is_not_four_fields_is_ignored_rather_than_misread() {
        // PowerShell prints its errors to the same place, and a line of prose
        // read as a screen would put nonsense on the report.
        assert!(parse_windows_screens("Add-Type : Cannot find the type.").is_none());
        assert!(parse_windows_screens("").is_none());
    }

    #[test]
    fn a_size_is_found_wherever_it_sits_in_a_platforms_own_wording() {
        assert_eq!(dimensions("3024 x 1964 Retina"), Some(Resolution::new(3024, 1964)));
        assert_eq!(dimensions("2560 x 1080 @ 75.00Hz"), Some(Resolution::new(2560, 1080)));
        assert_eq!(dimensions("Retina"), None);
    }

    #[test]
    fn a_geometry_yields_the_size_and_the_position_that_decides_mirroring() {
        assert_eq!(geometry("1920x1080+1920+0"), Some((Resolution::new(1920, 1080), (1920, 0))));
        assert_eq!(geometry("1920x1080"), None);
    }

    #[test]
    fn reading_this_machine_answers_one_way_or_the_other() {
        // The one call that touches the operating system. What it says depends
        // on whether anything is plugged into the runner, which is exactly why
        // every assertion above uses the seam instead.
        let reading = read(Platform::host(), &Tools::on_this_machine(std::time::Duration::from_secs(5)));

        let _ = format!("{reading:?}");
    }
}
