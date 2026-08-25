//! `slidx theme add` — a package name into `package.json`, and nothing fetched.
//!
//! The four builtins stay four because the projector audit is a closed set.
//! `@slidxjs/theme-*` packages already exist; this command is how a deck names
//! one without slidx growing a network stack. It writes `devDependencies` with
//! `*`, or prints `vp add -D`. It does not run `vp`, `npm`, or anything else.
//!
//! A positional `add` is a branch of the `theme` leaf, not a subcommand. A
//! directory named `add` is still a path, spelled `./add`. The second word is a
//! package name. A theme id (`minimal`) and a path (`./theme.json`) are
//! refused: inventing either as a package is how a deck depends on something
//! that is not one.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::report;
use crate::style::{Ink, Style};
use crate::Outcome;

use slidx_theme::builtin;

/// The version written for a package slidx has not fetched.
///
/// A number would be invented. `*` is the honest one: whatever the author's
/// package manager resolves, when they run it.
const STAR: &str = "*";

pub fn run(package: Option<&str>, from: &Path, style: &Style) -> Outcome {
    let Some(package) = package.filter(|name| !name.is_empty()) else {
        return Outcome::misuse(needs_a_package());
    };

    if looks_like_a_path(package) {
        return Outcome::misuse(a_path_is_not_a_package(package));
    }

    if is_builtin_id(package) {
        return Outcome::misuse(a_theme_id_is_not_a_package(package));
    }

    if !is_package_name(package) {
        return Outcome::misuse(not_a_package(package));
    }

    match nearest_manifest(from) {
        Some(path) => write_dependency(&path, package, style),
        None => Outcome::out(print_the_install(package, from, style)),
    }
}

/// Walks from `from` towards the filesystem root for a `package.json`.
fn nearest_manifest(from: &Path) -> Option<PathBuf> {
    let start = if from.is_file() { from.parent().unwrap_or(from) } else { from };

    start.ancestors().map(|directory| directory.join("package.json")).find(|path| path.is_file())
}

fn write_dependency(path: &Path, package: &str, style: &Style) -> Outcome {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            return Outcome::misuse(format!("Could not read {}: {error}\n", path.display()))
        }
    };

    let mut manifest: Value = match serde_json::from_str(&source) {
        Ok(Value::Object(map)) => Value::Object(map),
        Ok(_) => {
            return Outcome::misuse(format!(
                "{} is not a JSON object, so a dependency cannot be added to it.\n",
                path.display()
            ))
        }
        Err(error) => {
            return Outcome::misuse(format!("Could not parse {}: {error}\n", path.display()))
        }
    };

    if named_in(&manifest, package) {
        return Outcome::out(already_named(package, style));
    }

    if let Err(message) = insert_star(&mut manifest, package) {
        return Outcome::misuse(message);
    }

    let mut written = match serde_json::to_string_pretty(&manifest) {
        Ok(json) => json,
        Err(error) => {
            return Outcome::misuse(format!("Could not write {}: {error}\n", path.display()))
        }
    };
    if !written.ends_with('\n') {
        written.push('\n');
    }

    if let Err(error) = fs::write(path, written) {
        return Outcome::misuse(format!("Could not write {}: {error}\n", path.display()));
    }

    Outcome::out(wrote(package, style))
}

fn named_in(manifest: &Value, package: &str) -> bool {
    ["devDependencies", "dependencies"]
        .iter()
        .any(|field| manifest.get(*field).and_then(|deps| deps.get(package)).is_some())
}

fn insert_star(manifest: &mut Value, package: &str) -> Result<(), String> {
    let object = manifest.as_object_mut().ok_or_else(|| {
        "package.json is not an object, so a dependency cannot be added to it.\n".to_string()
    })?;
    let deps = object.entry("devDependencies").or_insert_with(|| json!({}));
    let deps = deps.as_object_mut().ok_or_else(|| {
        "package.json has a devDependencies that is not an object, so a package cannot be added to it.\n"
            .to_string()
    })?;
    deps.insert(package.to_string(), json!(STAR));
    Ok(())
}

fn is_builtin_id(value: &str) -> bool {
    builtin::all().iter().any(|theme| theme.id == value)
}

/// npm's name grammar, narrowed to what a person types for a theme package.
///
/// Scoped (`@slidxjs/theme-workshop`) or a single name part. A second `/` is a
/// path. A leading `.` is a path. Uppercase is allowed because the registry
/// is, and this command must not invent a different spelling.
fn is_package_name(value: &str) -> bool {
    if let Some(rest) = value.strip_prefix('@') {
        let Some((scope, name)) = rest.split_once('/') else {
            return false;
        };
        return is_name_part(scope) && is_name_part(name);
    }

    is_name_part(value)
}

fn is_name_part(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn looks_like_a_path(value: &str) -> bool {
    value.starts_with('.')
        || value.starts_with('/')
        || value.starts_with('~')
        || value.contains('\\')
        || (value.contains('/') && !value.starts_with('@'))
}

fn needs_a_package() -> String {
    "`slidx theme add` needs a package name.\n\n\
     \x20 slidx theme add @slidxjs/theme-workshop\n\n\
     A theme id (`minimal`) is not a package, and a path is still a path:\n\
     `slidx theme ./theme.json` checks a document. This writes a name into\n\
     package.json and does not fetch it.\n"
        .to_string()
}

fn a_path_is_not_a_package(value: &str) -> String {
    format!(
        "`{value}` looks like a path. `slidx theme add` writes a package name\n\
         into package.json; it does not install from a directory.\n\n\
         \x20 slidx theme add @slidxjs/theme-workshop\n\n\
         To check a theme document:\n\n\
         \x20 slidx theme {value}\n"
    )
}

fn a_theme_id_is_not_a_package(value: &str) -> String {
    format!(
        "`{value}` is a theme slidx already ships. `slidx theme add` takes a\n\
         package name — what `vp add` would accept — not a built-in id.\n\n\
         \x20 slidx theme add @slidxjs/theme-workshop\n"
    )
}

fn not_a_package(value: &str) -> String {
    format!(
        "`{value}` is not a package name.\n\n\
         \x20 slidx theme add @slidxjs/theme-workshop\n"
    )
}

fn print_the_install(package: &str, from: &Path, style: &Style) -> String {
    format!(
        "{}\n\n\
         `slidx theme add` does not run a package manager, and it will not\n\
         invent a project either. In {}:\n\n\
         \x20 vp add -D {}\n",
        style.paint(Ink::Strong, "No package.json is here to write into."),
        from.display(),
        report::shell_arg(package),
    )
}

fn already_named(package: &str, style: &Style) -> String {
    format!(
        "  {}  {}\n  {}\n",
        style.pad(Ink::Pass, "named", report::STATUS_WIDTH),
        style.paint(Ink::Strong, package),
        style.paint(Ink::Faint, "package.json already lists it."),
    )
}

fn wrote(package: &str, style: &Style) -> String {
    format!(
        "  {}  {}\n  {}\n",
        style.pad(Ink::Pass, "wrote", report::STATUS_WIDTH),
        style.paint(Ink::Strong, package),
        style.paint(Ink::Faint, "devDependency * — slidx did not fetch it."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;
    use crate::{MISUSE, OK};

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "slidx-theme-add-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a temporary directory");
        directory
    }

    fn add(package: &str, from: &Path) -> Outcome {
        run(Some(package), from, &Style::plain())
    }

    #[test]
    fn a_missing_package_name_is_a_misuse() {
        let outcome = run(None, Path::new("."), &Style::plain());

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stderr.contains("package name"), "{}", outcome.stderr);
        assert!(outcome.stderr.contains("slidx theme add"), "{}", outcome.stderr);
    }

    #[test]
    fn a_path_is_refused_rather_than_written_as_a_package() {
        let directory = scratch("path");
        fs::write(directory.join("package.json"), "{}\n").unwrap();

        let outcome = add("./theme.json", &directory);

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stderr.contains("path"), "{}", outcome.stderr);
        let manifest = fs::read_to_string(directory.join("package.json")).unwrap();
        assert_eq!(manifest, "{}\n");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_builtin_theme_id_is_not_a_package() {
        let directory = scratch("builtin");
        fs::write(directory.join("package.json"), "{}\n").unwrap();

        let outcome = add("minimal", &directory);

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stderr.contains("minimal"), "{}", outcome.stderr);
        assert!(outcome.stderr.contains("already ships"), "{}", outcome.stderr);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_manifest_is_given_a_star_dev_dependency() {
        let directory = scratch("write");
        fs::write(
            directory.join("package.json"),
            "{\n  \"name\": \"talk\",\n  \"private\": true\n}\n",
        )
        .unwrap();

        let outcome = add("@slidxjs/theme-workshop", &directory);

        assert_eq!(outcome.code, OK, "{}", outcome.stderr);
        assert!(outcome.stdout.contains("@slidxjs/theme-workshop"), "{}", outcome.stdout);

        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(directory.join("package.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["devDependencies"]["@slidxjs/theme-workshop"], "*");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn adding_the_same_package_twice_does_not_rewrite_the_file() {
        let directory = scratch("twice");
        let path = directory.join("package.json");
        fs::write(&path, "{\n  \"devDependencies\": {\n    \"vite\": \"^7\"\n  }\n}\n").unwrap();

        assert_eq!(add("@slidxjs/theme-workshop", &directory).code, OK);
        let after_first = fs::read_to_string(&path).unwrap();
        let outcome = add("@slidxjs/theme-workshop", &directory);

        assert_eq!(outcome.code, OK);
        assert!(outcome.stdout.contains("already lists"), "{}", outcome.stdout);
        assert_eq!(fs::read_to_string(&path).unwrap(), after_first);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_package_already_in_dependencies_is_left_alone() {
        let directory = scratch("prod");
        let original =
            "{\n  \"dependencies\": {\n    \"@slidxjs/theme-workshop\": \"0.6.0\"\n  }\n}\n";
        fs::write(directory.join("package.json"), original).unwrap();

        let outcome = add("@slidxjs/theme-workshop", &directory);

        assert_eq!(outcome.code, OK);
        assert_eq!(fs::read_to_string(directory.join("package.json")).unwrap(), original);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn without_a_manifest_the_vp_line_is_printed_and_nothing_is_created() {
        let directory = scratch("none");

        let outcome = add("@slidxjs/theme-workshop", &directory);

        assert_eq!(outcome.code, OK, "{}", outcome.stderr);
        assert!(outcome.stdout.contains("vp add -D @slidxjs/theme-workshop"), "{}", outcome.stdout);
        assert!(!directory.join("package.json").exists());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_manifest_above_the_working_directory_is_the_one_that_is_written() {
        let directory = scratch("walk");
        fs::write(directory.join("package.json"), "{}\n").unwrap();
        let nested = directory.join("slides");
        fs::create_dir(&nested).unwrap();

        assert_eq!(add("@slidxjs/theme-workshop", &nested).code, OK);

        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(directory.join("package.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["devDependencies"]["@slidxjs/theme-workshop"], "*");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn an_unreadable_manifest_is_a_misuse_rather_than_a_silent_skip() {
        let directory = scratch("bad");
        fs::write(directory.join("package.json"), "not json\n").unwrap();

        let outcome = add("@slidxjs/theme-workshop", &directory);

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stderr.contains("parse"), "{}", outcome.stderr);
        let _ = fs::remove_dir_all(&directory);
    }
}
