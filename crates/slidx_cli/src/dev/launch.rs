//! Which program starts the project's own dev server.
//!
//! `slidx dev` runs Vite. It does not contain a server, does not proxy one, and
//! does not know how a slide becomes HTML — that is `@slidxjs/vite-plugin`, and a
//! second implementation of it here would be two answers to one question. So
//! the only decision in this module is *how to reach the Vite the project
//! already installed*, which is a question about the project's package manager
//! and about nothing else.
//!
//! ## Why the lockfile decides
//!
//! A lockfile is the one artifact that says which package manager actually
//! installed `node_modules`. A manifest's `packageManager` field says what
//! somebody intended, and a `node_modules/.bin` says nothing about who put it
//! there. Reaching Vite through the wrong manager is how a command fails with
//! "vite: not found" in a project where Vite is plainly installed.
//!
//! Vite+ is checked before any lockfile, because a project that has it does not
//! merely have a package manager that can reach Vite — it has its own dev
//! command, and that is the one its author runs.

use std::path::Path;

/// The program and its leading arguments, before any Vite flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
}

/// How this project reaches the Vite it installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    /// Vite+, which owns Vite and has its own `dev`.
    VitePlus,
    Pnpm,
    Bun,
    Yarn,
    /// The fallback, and the right answer for a project with no lockfile
    /// committed at all.
    Npm,
}

impl Runner {
    /// What the reader would have typed themselves, for the ready line.
    pub fn typed(self) -> &'static str {
        match self {
            Self::VitePlus => "vp dev",
            Self::Pnpm => "pnpm exec vite",
            Self::Bun => "bun x vite",
            Self::Yarn => "yarn vite",
            Self::Npm => "npm exec -- vite",
        }
    }

    /// The program, and everything before the Vite flags.
    fn prefix(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::VitePlus => ("vp", &["dev"]),
            Self::Pnpm => ("pnpm", &["exec", "vite"]),
            Self::Bun => ("bun", &["x", "vite"]),
            // No `exec`: yarn 1 does not have one, and both yarn 1 and berry
            // run a workspace binary named directly after the command.
            Self::Yarn => ("yarn", &["vite"]),
            // The `--` is what keeps `--port` from being read as npm's own.
            Self::Npm => ("npm", &["exec", "--", "vite"]),
        }
    }
}

/// The lockfile each manager writes, in the order they are checked.
///
/// A repository holding two of these has been migrated and not cleaned up. The
/// order is the one that is most likely to be the live install, which is the
/// only tie-break available without asking somebody.
const LOCKFILES: &[(&str, Runner)] = &[
    ("pnpm-lock.yaml", Runner::Pnpm),
    ("bun.lock", Runner::Bun),
    ("bun.lockb", Runner::Bun),
    ("yarn.lock", Runner::Yarn),
    ("package-lock.json", Runner::Npm),
];

/// How to reach Vite in this project.
///
/// Looks in the project and then above it, because a deck in a monorepo has its
/// lockfile at the repository root and its `vite.config.ts` in its own package.
pub fn runner_for(root: &Path) -> Runner {
    let mut directory = Some(root);

    while let Some(here) = directory {
        if uses_vite_plus(here) {
            return Runner::VitePlus;
        }

        if let Some((_, runner)) = LOCKFILES.iter().find(|(name, _)| here.join(name).is_file()) {
            return *runner;
        }

        directory = here.parent();
    }

    Runner::Npm
}

/// True when this directory's manifest depends on Vite+.
fn uses_vite_plus(directory: &Path) -> bool {
    std::fs::read_to_string(directory.join("package.json"))
        .unwrap_or_default()
        .contains("\"vite-plus\"")
}

/// The command line to spawn: the runner's prefix, then the Vite flags.
pub fn plan(runner: Runner, flags: &[String]) -> Launch {
    let (program, leading) = runner.prefix();

    Launch {
        program: program.to_string(),
        args: leading
            .iter()
            .map(|argument| (*argument).to_string())
            .chain(flags.to_vec())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-launch-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn write(&self, relative: &str, body: &str) -> &Self {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(path, body).expect("write");
            self
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn flags(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn a_project_with_a_pnpm_lockfile_reaches_vite_through_pnpm() {
        let scratch = Scratch::new("pnpm");
        scratch.write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

        assert_eq!(runner_for(&scratch.0), Runner::Pnpm);
    }

    #[test]
    fn every_manager_is_recognised_by_the_lockfile_it_writes() {
        for (name, expected) in LOCKFILES {
            let scratch = Scratch::new(&name.replace('.', "-"));
            scratch.write(name, "");

            assert_eq!(runner_for(&scratch.0), *expected, "{name}");
        }
    }

    #[test]
    fn a_project_that_uses_vite_plus_is_started_with_its_own_dev_command() {
        // Vite+ does not merely have a package manager that can reach Vite; it
        // has the dev command this project's author runs.
        let scratch = Scratch::new("viteplus");
        scratch
            .write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n")
            .write("package.json", "{ \"devDependencies\": { \"vite-plus\": \"0.2.6\" } }");

        assert_eq!(runner_for(&scratch.0), Runner::VitePlus);
    }

    #[test]
    fn a_lockfile_above_the_deck_still_decides_because_a_monorepo_keeps_it_at_the_root() {
        let scratch = Scratch::new("monorepo");
        scratch.write("pnpm-lock.yaml", "").write("talks/vueconf/vite.config.ts", "");

        assert_eq!(runner_for(&scratch.0.join("talks/vueconf")), Runner::Pnpm);
    }

    #[test]
    fn a_project_with_no_lockfile_anywhere_falls_back_to_npm() {
        // npm is the one manager every machine with Node already has, so it is
        // the only fallback that is not a guess about somebody's setup.
        assert_eq!(runner_for(Path::new("/nowhere/at/all")), Runner::Npm);
    }

    #[test]
    fn the_flags_slidx_chose_come_after_the_runners_own_arguments() {
        let planned = plan(Runner::Pnpm, &flags(["--open", "/__slidx/"].as_slice()));

        assert_eq!(planned.program, "pnpm");
        assert_eq!(planned.args, ["exec", "vite", "--open", "/__slidx/"]);
    }

    #[test]
    fn npm_gets_a_double_dash_so_a_port_is_not_read_as_its_own_option() {
        let planned = plan(Runner::Npm, &flags(["--port", "5173"].as_slice()));

        assert_eq!(planned.args, ["exec", "--", "vite", "--port", "5173"]);
    }

    #[test]
    fn every_runner_can_be_named_the_way_somebody_would_have_typed_it() {
        // The ready line says what slidx is running, so an author can run it
        // themselves the day they stop wanting slidx in the middle.
        for runner in [Runner::VitePlus, Runner::Pnpm, Runner::Bun, Runner::Yarn, Runner::Npm] {
            let typed = runner.typed();
            let planned = plan(runner, &[]);

            assert!(typed.starts_with(&planned.program), "{typed} does not start with the program");
        }
    }
}
