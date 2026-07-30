//! Running the deck's own build.
//!
//! `slidx export` produces nothing a build did not, so the first half of it is
//! starting the build the author already has and waiting. This module is only
//! that: find the deck's Vite, run it, say what happened.
//!
//! ## The deck's Vite, never a package manager
//!
//! It runs `node_modules/.bin/vite` directly rather than `npm run build` or
//! `npx vite`. Two reasons, and the second is the one that matters:
//!
//! - A script called `build` is a convention, not a guarantee, and a deck whose
//!   script is called something else would be unexportable for no reason.
//! - `npm exec` **installs what it cannot find**. A build is supposed to make no
//!   network requests, and a command that could quietly fetch a package from a
//!   registry mid-export would break that promise on exactly the machine where
//!   it matters — a laptop on conference wifi the night before.
//!
//! So the binary is either already installed, or slidx says so and names what
//! to run. Nothing here reaches a network, and nothing can start doing so by
//! accident.
//!
//! ## Asking for more than the ordinary output
//!
//! The static pages are what every build writes. The PDF is off by default, and
//! per-slide documents and per-stop images are not written at all, because each
//! costs a browser launch nobody asked for. So the build is told what this
//! export needs through [`FRAME_VARIABLE`], which `@slidx/vite-plugin` reads —
//! see `packages/vite-plugin/src/frames.ts`. It is an environment variable
//! rather than an option because it is this command talking to that build, not
//! a setting an author maintains in a config file.

use std::path::{Path, PathBuf};
use std::process::Command;

use slidx_export::Frame;

/// What the plugin reads to find out which frames to render.
///
/// Named on both sides of the boundary: the value is [`Frame::as_token`], and
/// `packages/vite-plugin/src/frames.ts` is the only thing that reads it.
pub const FRAME_VARIABLE: &str = "SLIDX_EXPORT";

/// One build, as this command runs it.
#[derive(Debug, Clone, Copy)]
pub struct Build<'a> {
    /// The directory holding the deck's `vite.config.ts`.
    pub root: &'a Path,
    /// Where the output should land, when the caller named one.
    ///
    /// `None` leaves the project's own `build.outDir` alone. Passing `--outDir`
    /// unasked would override a deck that configured one and put the build
    /// somewhere its author does not expect.
    pub dist: Option<&'a Path>,
    pub frame: Option<Frame>,
}

/// Runs it, and waits.
///
/// The build's own output goes straight to the terminal rather than being
/// captured: it carries the linter's findings, and a contrast failure is worth
/// more to the person exporting than a tidier report would be. It also takes
/// tens of seconds on a deck with images, and a silent terminal for that long
/// reads as a hang.
pub fn run(build: &Build) -> Result<(), String> {
    let vite = find_vite(build.root).ok_or_else(|| no_vite(build.root))?;

    let mut command = Command::new(&vite);
    command.arg("build").current_dir(build.root);

    if let Some(dist) = build.dist {
        command.arg("--outDir").arg(dist);
        // Vite refuses to empty an output directory outside the project root
        // without this, and a directory that is not emptied leaves the previous
        // export's frames in it — which is how an export ends up holding slides
        // the deck no longer has.
        command.arg("--emptyOutDir");
    }

    if let Some(frame) = build.frame {
        command.env(FRAME_VARIABLE, frame.as_token());
    }

    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(failed(status.code())),
        Err(error) => Err(format!("Could not run {}: {error}\n", vite.display())),
    }
}

/// The deck's own Vite, if it is installed.
///
/// Walked up from the deck's project, the way Node resolves a binary: a deck
/// inside a workspace has its dependencies hoisted to the workspace root, and
/// one on its own has them beside it.
fn find_vite(root: &Path) -> Option<PathBuf> {
    // `.cmd` is what a package manager writes on Windows; the extensionless
    // file exists there too and is a shell script the OS cannot execute.
    let names: &[&str] = if cfg!(windows) { &["vite.cmd", "vite.CMD", "vite"] } else { &["vite"] };

    root.ancestors().find_map(|directory| {
        let bin = directory.join("node_modules").join(".bin");
        names.iter().map(|name| bin.join(name)).find(|candidate| candidate.is_file())
    })
}

fn no_vite(root: &Path) -> String {
    format!(
        "No Vite is installed for the deck in {}.\n\n\
         `slidx export` runs the deck's own build and packages what it wrote — it\n\
         does not render anything itself, and it will not install anything either.\n\
         In the deck's project:\n\n\
         \x20 vp add -D vite @slidx/vite-plugin\n\n\
         Or, if the build already ran somewhere else:\n\n\
         \x20 slidx export --target browser --no-build\n",
        root.display()
    )
}

fn failed(code: Option<i32>) -> String {
    let status = match code {
        Some(code) => format!("exited {code}"),
        None => "was stopped".to_string(),
    };

    format!(
        "The deck's build {status}, so there is nothing to package.\n\n\
         What it printed is above. A build stops on a blocking lint finding by\n\
         design, which is the last place one is cheap to fix.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A project tree, with or without the binary installed.
    struct Project(PathBuf);

    impl Project {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("slidx-build-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("deck")).expect("scratch");

            Self(root)
        }

        /// Installs a stub where a package manager would put the real one.
        fn with_vite(self, at: &str) -> Self {
            let bin = self.0.join(at).join("node_modules").join(".bin");
            fs::create_dir_all(&bin).expect("bin");
            let name = if cfg!(windows) { "vite.cmd" } else { "vite" };
            fs::write(bin.join(name), "#!/bin/sh\nexit 0\n").expect("stub");

            self
        }

        fn deck(&self) -> PathBuf {
            self.0.join("deck")
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_decks_own_vite_is_found_beside_it() {
        let project = Project::new("beside").with_vite("deck");

        assert!(find_vite(&project.deck()).is_some());
    }

    #[test]
    fn a_deck_in_a_workspace_finds_the_vite_hoisted_to_its_root() {
        // A monorepo installs once at the top. A resolver that only looked
        // beside the deck would report nothing installed in the layout most
        // repositories with more than one deck actually have.
        let project = Project::new("hoisted").with_vite("");

        assert!(find_vite(&project.deck()).is_some());
    }

    #[test]
    fn nothing_installed_is_reported_rather_than_reached_for_over_a_network() {
        // The whole reason this does not shell out to a package manager: `npm
        // exec` installs what it cannot find, and a build is supposed to make no
        // network requests at all.
        let project = Project::new("missing");

        assert!(find_vite(&project.deck()).is_none());
    }

    #[test]
    fn a_project_with_no_vite_names_what_to_install_and_the_way_round_it() {
        let message = no_vite(Path::new("/talks/vueconf"));

        assert!(message.contains("@slidx/vite-plugin"), "{message}");
        assert!(message.contains("--no-build"), "{message}");
        assert!(message.contains("does not render anything itself"), "{message}");
    }

    #[test]
    fn a_build_that_failed_says_the_output_above_is_the_reason() {
        // The build printed its own findings. Repeating them would be noise;
        // pretending they were not printed would be worse.
        let message = failed(Some(1));

        assert!(message.contains("exited 1"), "{message}");
        assert!(message.contains("nothing to package"), "{message}");
    }

    #[test]
    fn the_variable_the_plugin_reads_is_spelled_once() {
        // Two spellings of an environment variable is a build that renders
        // nothing and an export that reports missing frames, with no error
        // anywhere in between.
        assert_eq!(FRAME_VARIABLE, "SLIDX_EXPORT");
        assert_eq!(Frame::Png.as_token(), "png");
    }
}
