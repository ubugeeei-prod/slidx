//! What the output is set to.
//!
//! macOS answers both halves in one call and Linux answers them through
//! whichever sound server is installed. Windows answers neither: there is no
//! documented command-line reading of the output level, and getting one means
//! shipping native code against the audio endpoint interface. That is a real
//! platform limit rather than a gap in this module, so the Windows reading is
//! unavailable and says which is which.
//!
//! Nothing here reaches for the output *device*. No platform hands that over
//! in the same call as the level, and a pre-flight that spends an extra
//! subprocess per reading is one a speaker abandons — so the audio check's
//! remedy names the device instead, which is the honest trade rather than a
//! silent one.

use crate::environment::{Audio, Platform, Reading};
use crate::probe::tools::{self, Tools};

pub fn read(platform: Platform, tools: &Tools) -> Reading<Audio> {
    match platform {
        Platform::MacOs => tools::parsed(
            tools.output("osascript", &["-e", "get volume settings"]),
            parse_volume_settings,
            "this Mac reported a volume setting this build cannot read",
        ),
        Platform::Linux => read_linux(tools),
        // Not "slidx has not got round to it": there is no documented reading
        // of the output level from a Windows command line, and the interface
        // that has one needs native code compiled against it.
        Platform::Windows => Reading::unavailable(
            "Windows exposes no output level a command line can read",
        ),
        Platform::Unknown => {
            Reading::unavailable("slidx has no way to read the output level on this platform")
        }
    }
}

/// Asks PipeWire first, then PulseAudio.
///
/// `wpctl` answers both halves at once and is what a current desktop has.
/// `pactl` needs two calls, because the level and the mute state are two
/// commands — which is why it is the fallback rather than the first ask.
fn read_linux(tools: &Tools) -> Reading<Audio> {
    let pipewire = tools.output("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]);

    if let Some(audio) = pipewire.ok().as_deref().and_then(parse_wpctl) {
        return Reading::known(audio);
    }

    let level = tools
        .output("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])
        .ok()
        .as_deref()
        .and_then(parse_pactl_volume);

    let muted = tools
        .output("pactl", &["get-sink-mute", "@DEFAULT_SINK@"])
        .ok()
        .as_deref()
        .and_then(parse_pactl_mute);

    let audio = Audio { level_percent: level, muted };
    if audio.says_nothing() {
        return Reading::unavailable("no sound server on this machine answered slidx");
    }

    Reading::known(audio)
}

/// Parses `osascript -e "get volume settings"`.
///
/// ```text
/// output volume:44, input volume:100, alert volume:100, output muted:false
/// ```
///
/// The output volume is the literal `missing value` on a device whose level
/// lives on the hardware — HDMI to a projector does this — and that is a real
/// answer rather than a failure, so the mute half still comes back.
fn parse_volume_settings(output: &str) -> Option<Audio> {
    let level = field(output, "output volume:")
        .and_then(|value| value.parse::<u32>().ok())
        .map(|percent| percent.min(100) as u8);

    let muted = field(output, "output muted:").and_then(|value| match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    });

    let audio = Audio { level_percent: level, muted };

    (!audio.says_nothing()).then_some(audio)
}

/// One comma-separated field of the volume settings line.
fn field<'a>(output: &'a str, key: &str) -> Option<&'a str> {
    let rest = output.split_once(key)?.1;

    Some(rest.split(',').next().unwrap_or(rest).trim())
}

/// Parses `wpctl get-volume @DEFAULT_AUDIO_SINK@`.
///
/// ```text
/// Volume: 0.65 [MUTED]
/// ```
///
/// The level is a fraction and PipeWire will happily report one above 1.0 for a
/// boosted sink, so it is clamped rather than trusted into a percentage that
/// cannot exist.
fn parse_wpctl(output: &str) -> Option<Audio> {
    let rest = output.split_once("Volume:")?.1;
    let fraction: f64 = rest.split_whitespace().next()?.parse().ok()?;
    let percent = (fraction * 100.0).round().clamp(0.0, 100.0) as u8;

    Some(Audio { level_percent: Some(percent), muted: Some(rest.contains("[MUTED]")) })
}

/// Parses the first percentage out of `pactl get-sink-volume @DEFAULT_SINK@`.
///
/// Each channel is reported separately and they are the same on any machine
/// nobody has deliberately unbalanced, so the first is the reading. A speaker
/// whose left channel is quieter than their right has a problem no pre-flight
/// was going to catch.
fn parse_pactl_volume(output: &str) -> Option<u8> {
    output.split_whitespace().find_map(|token| {
        let digits = token.strip_suffix('%')?;

        digits.parse::<u32>().ok().map(|percent| percent.min(100) as u8)
    })
}

/// Parses `pactl get-sink-mute @DEFAULT_SINK@`, which answers `Mute: yes`.
fn parse_pactl_mute(output: &str) -> Option<bool> {
    match output.split_once("Mute:")?.1.trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MACOS: &str = "output volume:44, input volume:100, alert volume:100, output muted:false";

    #[test]
    fn both_platforms_that_answer_are_parsed_on_every_platform() {
        // The seam. macOS and Linux each read their own tool's exact output on
        // whichever runner is running, so neither parser can quietly rot.
        let mac = Tools::answering(|_, _| Ok(MACOS.to_string()));
        let linux = Tools::answering(|_, _| Ok("Volume: 0.65\n".to_string()));

        assert_eq!(read(Platform::MacOs, &mac).value(), Some(&Audio::playing_at(44)));
        assert_eq!(read(Platform::Linux, &linux).value(), Some(&Audio::playing_at(65)));
    }

    #[test]
    fn windows_says_it_cannot_read_this_rather_than_reporting_a_level() {
        // The honest answer, and the reason it is worth stating: a green audio
        // line on Windows would be a claim nobody measured.
        let reading = read(Platform::Windows, &Tools::absent());

        assert!(!reading.is_known());
        assert!(reading.reason().is_some_and(|why| why.contains("Windows exposes no")));
    }

    #[test]
    fn a_platform_slidx_cannot_ask_says_so() {
        assert!(read(Platform::Unknown, &Tools::absent()).reason().is_some());
    }

    #[test]
    fn the_mac_volume_line_gives_the_level_and_the_mute_state_together() {
        let audio = parse_volume_settings(MACOS).unwrap_or(Audio::level_only(0));

        assert_eq!(audio.level_percent, Some(44));
        assert_eq!(audio.muted, Some(false));
    }

    #[test]
    fn a_mac_output_with_no_software_volume_still_reports_whether_it_is_muted() {
        // HDMI to a projector answers `missing value` for the level, because
        // the knob really is on the other end of the cable. Half a reading is
        // worth having when the half is the mute state.
        let hdmi = "output volume:missing value, input volume:75, output muted:true";
        let audio = parse_volume_settings(hdmi).unwrap_or(Audio::level_only(0));

        assert_eq!(audio.level_percent, None);
        assert_eq!(audio.muted, Some(true));
    }

    #[test]
    fn a_mac_answer_with_neither_half_in_it_is_no_reading_at_all() {
        assert!(parse_volume_settings("").is_none());
        assert!(parse_volume_settings("something else entirely").is_none());
    }

    #[test]
    fn pipewire_reports_a_muted_sink_on_the_same_line_as_its_level() {
        let audio = parse_wpctl("Volume: 0.70 [MUTED]\n").unwrap_or(Audio::level_only(0));

        assert_eq!(audio, Audio::muted_at(70));
    }

    #[test]
    fn a_boosted_pipewire_sink_is_clamped_rather_than_reported_above_a_hundred() {
        // PipeWire will go past 1.0. A percentage that cannot exist makes the
        // line look broken at the moment it needs to be believed.
        assert_eq!(parse_wpctl("Volume: 1.53\n").unwrap_or(Audio::level_only(0)).level_percent, Some(100));
    }

    #[test]
    fn a_pipewire_answer_with_no_number_in_it_is_no_reading() {
        assert!(parse_wpctl("Volume:\n").is_none());
        assert!(parse_wpctl("Node not found").is_none());
    }

    #[test]
    fn pulseaudio_is_asked_when_pipewire_is_not_installed() {
        // The fallback that makes the Linux reading worth having on a machine
        // that never moved to PipeWire.
        let pulse = Tools::answering(|program, args| match (program, args.first()) {
            ("wpctl", _) => Err("`wpctl` could not be run".to_string()),
            (_, Some(&"get-sink-volume")) => {
                Ok("Volume: front-left: 42281 /  65% / -11.14 dB,   front-right: 42281 /  65%"
                    .to_string())
            }
            _ => Ok("Mute: yes\n".to_string()),
        });

        assert_eq!(read_linux(&pulse).value(), Some(&Audio::muted_at(65)));
    }

    #[test]
    fn a_pulseaudio_percentage_is_taken_from_the_first_channel() {
        let output = "Volume: front-left: 42281 /  65% / -11.14 dB,   front-right: 42281 /  65%";

        assert_eq!(parse_pactl_volume(output), Some(65));
        assert_eq!(parse_pactl_volume("Volume: front-left: 98304 / 150%"), Some(100));
        assert_eq!(parse_pactl_volume("Sink not found"), None);
    }

    #[test]
    fn pulseaudio_answers_its_mute_state_in_words() {
        assert_eq!(parse_pactl_mute("Mute: yes\n"), Some(true));
        assert_eq!(parse_pactl_mute("Mute: no\n"), Some(false));
        assert_eq!(parse_pactl_mute("Failure: No such entity"), None);
    }

    #[test]
    fn a_linux_machine_with_no_sound_server_at_all_reports_unknown() {
        // A container, or a machine with no audio stack. Neither is a machine
        // whose output is fine.
        let reading = read_linux(&Tools::absent());

        assert!(!reading.is_known());
        assert!(reading.reason().is_some_and(|why| why.contains("no sound server")));
    }

    #[test]
    fn reading_this_machine_answers_one_way_or_the_other() {
        // The one call that touches the operating system. What it says depends
        // on the runner's own audio stack, which is why everything above goes
        // through the seam instead.
        let reading =
            read(Platform::host(), &Tools::on_this_machine(std::time::Duration::from_secs(5)));

        let _ = format!("{reading:?}");
    }
}
