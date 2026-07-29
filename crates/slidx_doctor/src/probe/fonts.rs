//! Which font families this machine can resolve.
//!
//! Two strategies. `fc-list` knows real family names and is the right answer
//! wherever it exists; everywhere else the font directories are listed and the
//! file name is used as a stand-in for the family name.
//!
//! The file-name approach is approximate — `HelveticaNeue.ttc` is not spelled
//! the way the family is — and that is handled by comparing on letters and
//! digits alone rather than by pretending the two agree. Approximate in the
//! forgiving direction is the right bias here: a check that wrongly reports a
//! font missing sends a speaker chasing a problem they can see is not real,
//! and after the first time they stop reading the line.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::environment::{InstalledFonts, Reading};
use crate::probe::command;

/// Extensions worth treating as a font. Anything else in these directories —
/// caches, licence files, `.DS_Store` — is not a family.
const FONT_EXTENSIONS: &[&str] =
    &["ttf", "otf", "ttc", "otc", "woff", "woff2", "dfont", "pfb", "pfa", "bdf", "pcf"];

pub fn read(timeout: Duration) -> Reading<InstalledFonts> {
    // `fc-list` is the only source here that knows what a family is actually
    // called, so it is preferred wherever it exists rather than only on Linux.
    if let Some(output) = command::try_output("fc-list", &[":", "family"], timeout) {
        let families = parse_fc_list(&output);

        if !families.is_empty() {
            return Reading::known(families.into_iter().collect());
        }
    }

    let directories = font_directories();
    if directories.is_empty() {
        return Reading::unavailable("slidx does not know where fonts live on this platform");
    }

    let families = scan(&directories);
    if families.is_empty() {
        return Reading::unavailable("no font directory on this machine could be listed");
    }

    Reading::known(families.into_iter().collect())
}

/// Where each platform keeps its fonts.
///
/// Missing directories are harmless: they are filtered out when scanned, so
/// the list can be generous rather than exactly right.
fn font_directories() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);

    #[cfg(target_os = "macos")]
    {
        let mut directories = vec![
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts/Supplemental"),
            PathBuf::from("/Library/Fonts"),
        ];
        directories.extend(home.map(|home| home.join("Library/Fonts")));
        directories
    }

    #[cfg(target_os = "linux")]
    {
        let mut directories =
            vec![PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/local/share/fonts")];

        if let Some(home) = home {
            // `.fonts` is the older spelling and is still in use.
            directories.push(home.join(".local/share/fonts"));
            directories.push(home.join(".fonts"));
        }

        directories
    }

    #[cfg(windows)]
    {
        let _ = home;
        let mut directories = Vec::new();
        directories
            .extend(std::env::var_os("WINDIR").map(|windir| PathBuf::from(windir).join("Fonts")));
        directories.extend(
            std::env::var_os("LOCALAPPDATA")
                .map(|local| PathBuf::from(local).join("Microsoft/Windows/Fonts")),
        );
        directories
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = home;
        Vec::new()
    }
}

/// Collects family names from a set of directories, one level of nesting deep.
///
/// One level, not a full walk: Linux nests fonts by vendor, and a recursive
/// walk of `/usr/share/fonts` on a machine with a full language pack visits
/// thousands of files for a reading nobody is waiting on.
fn scan(directories: &[PathBuf]) -> BTreeSet<String> {
    let mut families = BTreeSet::new();

    for directory in directories {
        collect(directory, &mut families);

        let Ok(entries) = fs::read_dir(directory) else { continue };
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                collect(&entry.path(), &mut families);
            }
        }
    }

    families
}

fn collect(directory: &Path, families: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(directory) else { return };

    for entry in entries.flatten() {
        if let Some(family) = family_from_file_name(&entry.file_name().to_string_lossy()) {
            families.insert(family);
        }
    }
}

/// Turns `HelveticaNeue.ttc` into `HelveticaNeue`, and rejects anything that is
/// not a font file.
fn family_from_file_name(name: &str) -> Option<String> {
    let (stem, extension) = name.rsplit_once('.')?;

    if !FONT_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) {
        return None;
    }

    // `Inter-Bold` and `Inter-Regular` are both the Inter family. Keeping the
    // weight would make a machine that has Inter look like it does not.
    let family = stem.split(['-', '_']).next().unwrap_or(stem).trim();

    (!family.is_empty()).then(|| family.to_string())
}

/// Parses `fc-list : family`, whose lines carry a family and its aliases,
/// comma separated.
fn parse_fc_list(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|family| !family.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn fc_list_output_yields_every_family_and_alias() {
        // A line names one font under several spellings; all of them are worth
        // matching against, since a theme may name any of them.
        let output = "DejaVu Sans,DejaVu Sans Book\nInter\nNoto Sans JP,Noto Sans JP Regular\n";
        let families = parse_fc_list(output);

        assert!(families.contains("DejaVu Sans"));
        assert!(families.contains("DejaVu Sans Book"));
        assert!(families.contains("Inter"));
        assert_eq!(families.len(), 5);
    }

    #[test]
    fn empty_fc_list_output_yields_nothing_rather_than_an_empty_name() {
        // An empty family name would match every lookup and turn the fonts
        // check permanently green.
        assert!(parse_fc_list("").is_empty());
        assert!(parse_fc_list("\n,\n , \n").is_empty());
    }

    #[test]
    fn a_font_file_name_is_read_as_a_family() {
        assert_eq!(family_from_file_name("HelveticaNeue.ttc").as_deref(), Some("HelveticaNeue"));
        assert_eq!(family_from_file_name("Inter.otf").as_deref(), Some("Inter"));
    }

    #[test]
    fn a_weight_suffix_is_dropped_so_the_family_still_matches() {
        // A machine with `Inter-Regular.ttf` has Inter. Keeping the suffix
        // would report the font as missing on exactly the machines that have it.
        assert_eq!(family_from_file_name("Inter-Regular.ttf").as_deref(), Some("Inter"));
        assert_eq!(family_from_file_name("IBMPlexMono_Bold.otf").as_deref(), Some("IBMPlexMono"));
    }

    #[test]
    fn a_file_that_is_not_a_font_is_ignored() {
        // Font directories are full of caches and metadata.
        assert!(family_from_file_name(".DS_Store").is_none());
        assert!(family_from_file_name("fonts.dir").is_none());
        assert!(family_from_file_name("README").is_none());
    }

    #[test]
    fn a_font_extension_in_capitals_is_still_a_font() {
        // Windows font directories are full of them.
        assert_eq!(family_from_file_name("ARIAL.TTF").as_deref(), Some("ARIAL"));
    }

    #[test]
    fn a_directory_that_does_not_exist_contributes_nothing_and_does_not_fail() {
        // The directory list is deliberately generous; missing entries are the
        // normal case, not an error.
        let families = scan(&[PathBuf::from("/slidx-no-such-font-directory")]);

        assert!(families.is_empty());
    }

    #[test]
    fn fonts_one_directory_deep_are_found() {
        // Linux nests by vendor: /usr/share/fonts/truetype/dejavu. A flat scan
        // would find nothing at all on a Linux machine.
        let root = std::env::temp_dir().join("slidx-doctor-fonts");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("truetype/dejavu")).unwrap();
        fs::write(root.join("Inter.otf"), "").unwrap();
        fs::write(root.join("truetype/Roboto.ttf"), "").unwrap();
        // Two levels down, which the scan deliberately does not reach.
        fs::write(root.join("truetype/dejavu/DejaVuSans.ttf"), "").unwrap();

        let families = scan(std::slice::from_ref(&root));
        let _ = fs::remove_dir_all(&root);

        assert!(families.contains("Inter"));
        assert!(families.contains("Roboto"));
        assert!(!families.contains("DejaVuSans"), "the scan should stop one level down");
    }

    #[test]
    fn reading_this_machine_either_lists_fonts_or_says_why_not() {
        // Never a silently empty list: an empty `InstalledFonts` would make
        // every named family look missing.
        let reading = read(Duration::from_secs(10));

        match reading.value() {
            Some(fonts) => assert!(!fonts.is_empty()),
            None => assert!(reading.reason().is_some()),
        }
    }
}
