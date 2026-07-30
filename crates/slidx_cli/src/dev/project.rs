//! Finding the Vite project whose dev server `slidx dev` is going to start.
//!
//! `slidx dev` starts somebody else's server, so the first thing it has to know
//! is whether there is one. A Vite project is identified by its config file and
//! nothing else: that is the file `slidx()` is registered in, and without it
//! there is no plugin, no editor route, and nothing for this command to drive.
//!
//! ## Why it walks up
//!
//! An author editing `slides/0004.md` is in `slides/`, and the config is one
//! directory above. Refusing there would make the command work only from the
//! one directory somebody happens not to be in, so the search climbs to the
//! filesystem root the same way `.slidx-version` is found.
//!
//! ## Why a config without slidx in it is a note rather than a refusal
//!
//! The plugin can be registered from a file the config imports, and reading
//! TypeScript to find out would mean evaluating it. So a config that does not
//! mention slidx anywhere obvious is started anyway, with a line saying the
//! editor may not be there — which is the truth, and better than either
//! refusing a project that works or promising an editor that is missing.

use std::fs;
use std::path::{Path, PathBuf};

/// The config file names Vite itself looks for, in the order it looks.
///
/// Copied from Vite rather than guessed at: a project whose config is
/// `vite.config.mts` has a working dev server, and a slidx that could not find
/// it would look broken for a reason the author cannot see.
pub const CONFIG_NAMES: &[&str] =
    &["vite.config.ts", "vite.config.mts", "vite.config.cts", "vite.config.js", "vite.config.mjs"];

/// Where the plugin's name appears when a project depends on it.
const PLUGIN: &str = "@ubugeeei/slidx-vite-plugin";

/// A Vite project on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// The directory holding the config. Vite's root, and the child's cwd.
    pub root: PathBuf,
    /// The config file itself, so a message can name it.
    pub config: PathBuf,
}

impl Project {
    /// The nearest Vite project at or above `from`.
    pub fn find(from: &Path) -> Option<Self> {
        let start = from.canonicalize().unwrap_or_else(|_| from.to_path_buf());
        let mut directory = start.as_path();

        loop {
            if let Some(config) = config_in(directory) {
                return Some(Self { root: directory.to_path_buf(), config });
            }

            directory = directory.parent()?;
        }
    }

    /// True when this project says anywhere in writing that it uses slidx.
    ///
    /// The config text and the manifest, which between them cover every project
    /// laid out the way the README describes. A `false` is not evidence of
    /// absence — see the module docs — so nothing refuses on it.
    pub fn mentions_slidx(&self) -> bool {
        let manifest = fs::read_to_string(self.root.join("package.json")).unwrap_or_default();
        if manifest.contains(PLUGIN) {
            return true;
        }

        fs::read_to_string(&self.config).unwrap_or_default().contains("slidx")
    }
}

fn config_in(directory: &Path) -> Option<PathBuf> {
    CONFIG_NAMES.iter().map(|name| directory.join(name)).find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-project-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn write(&self, relative: &str, body: &str) -> PathBuf {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(&path, body).expect("write");
            path
        }

        fn dir(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(&path).expect("directory");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_project_is_the_directory_holding_the_vite_config() {
        let scratch = Scratch::new("here");
        scratch.write("vite.config.ts", "export default {};");

        let found = Project::find(&scratch.0).expect("a project");

        assert_eq!(found.config.file_name().unwrap(), "vite.config.ts");
    }

    #[test]
    fn a_project_is_found_from_inside_the_slides_directory() {
        // An author who has just been editing 0004.md is in `slides/`, and
        // refusing there would make the command work only from the one
        // directory they are not in.
        let scratch = Scratch::new("climb");
        scratch.write("vite.config.ts", "export default {};");
        let slides = scratch.dir("slides");

        assert_eq!(
            Project::find(&slides).expect("a project").root.canonicalize().unwrap(),
            scratch.0.canonicalize().unwrap()
        );
    }

    #[test]
    fn a_config_under_any_of_vites_own_extensions_counts() {
        for name in CONFIG_NAMES {
            let scratch = Scratch::new(&name.replace('.', "-"));
            scratch.write(name, "export default {};");

            assert!(Project::find(&scratch.0).is_some(), "{name} was not found");
        }
    }

    #[test]
    fn a_directory_with_no_vite_config_above_it_is_not_a_project() {
        // The root of a scratch tree has no config, and neither does anything
        // above it that a test can rely on. Nothing is found rather than the
        // nearest unrelated project being started.
        let scratch = Scratch::new("none");
        let deep = scratch.dir("a/b/c");

        let found = Project::find(&deep);

        assert!(
            found.as_ref().is_none_or(|project| !project.root.starts_with(&scratch.0)),
            "{found:?}"
        );
    }

    #[test]
    fn a_project_that_depends_on_the_plugin_says_so_through_its_manifest() {
        let scratch = Scratch::new("manifest");
        scratch.write("vite.config.ts", "import config from \"./vite/deck\";");
        scratch.write(
            "package.json",
            "{ \"devDependencies\": { \"@ubugeeei/slidx-vite-plugin\": \"^0\" } }",
        );

        assert!(Project::find(&scratch.0).expect("a project").mentions_slidx());
    }

    #[test]
    fn a_project_that_registers_the_plugin_in_its_config_says_so_there() {
        let scratch = Scratch::new("config");
        scratch.write("vite.config.ts", "import { slidx } from \"@ubugeeei/slidx-vite-plugin\";");

        assert!(Project::find(&scratch.0).expect("a project").mentions_slidx());
    }

    #[test]
    fn an_unrelated_vite_project_mentions_no_slidx_and_is_still_a_project() {
        // Started anyway, with a line saying the editor may not be there. The
        // plugin can be registered from a file the config imports, and reading
        // TypeScript to find out would mean evaluating it.
        let scratch = Scratch::new("unrelated");
        scratch.write("vite.config.ts", "export default { plugins: [] };");
        scratch.write("package.json", "{ \"devDependencies\": { \"vite\": \"^7\" } }");

        assert!(!Project::find(&scratch.0).expect("a project").mentions_slidx());
    }
}
