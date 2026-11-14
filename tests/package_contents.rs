//! Exercise Cargo's real package selection, independent of Git ignore rules.
use std::{collections::BTreeSet, path::Path, process::Command};

const PUBLIC_FILES: &[(&str, &str)] = &[
    (
        "src/main.rs",
        "mod required; fn main() { required::run(); }\n",
    ),
    ("src/required.rs", "pub fn run() {}\n"),
    ("tests/smoke.rs", "#[test] fn smoke() {}\n"),
    ("tests/fixtures/nested/contract.json", "{}\n"),
    ("tests/fixtures/layout.html", "<p>fixture</p>\n"),
    ("tests/fixtures/README.md", "# Fixtures\n"),
    ("docs/nested/guide.md", "# Public documentation\n"),
    ("README.md", "# Package policy fixture\n"),
    ("SPEC.md", "# Specification\n"),
    ("RELEASE.md", "# Release process\n"),
    ("TESTING.md", "# Testing guide\n"),
    ("LICENSE", "MIT\n"),
];

const LOCAL_FILES: &[&str] = &[
    "review/private.md",
    "review/nested/results.json",
    "future-local-notes/notes.md",
    "future-local-notes/debug.rs",
    "local-note.md",
    "token.txt",
    "config.toml",
    "runtime.env",
    ".env",
    ".codex",
    ".github/workflows/local.yml",
    "AGENTS.md",
    "docker/local.env",
    "scripts/private.py",
    "skills/private.md",
    ".tmp/runtime.env",
    "src/.env",
    "src/.private.rs",
    "src/.tmp/private.rs",
    "docs/.private.md",
    "docs/.cache/private.md",
    "tests/fixtures/.env",
    "tests/fixtures/.private.json",
    "tests/fixtures/.local/private.html",
    "tests/fixtures/runtime.env",
    "src/debug.log",
    "docs/local.env",
];

fn write(root: &Path, name: &str, content: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn cargo(root: &Path, args: &[&str]) -> std::process::Output {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo {args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn fixture(git: bool) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    // Copy the actual policy, not a second implementation of its glob matching.
    // The fixture has no registry dependencies or access to local user files.
    let manifest: toml::Value = toml::from_str(include_str!("../Cargo.toml")).unwrap();
    let mut fixture:toml::Value=toml::from_str("[package]\nname='package-policy-fixture'\nversion='0.0.0'\nedition='2021'\nlicense='MIT'\nreadme='README.md'\n").unwrap();
    for field in ["include", "exclude"] {
        if let Some(policy) = manifest["package"].get(field) {
            fixture["package"]
                .as_table_mut()
                .unwrap()
                .insert(field.into(), policy.clone());
        }
    }
    write(root, "Cargo.toml", &toml::to_string(&fixture).unwrap());
    for (name, content) in PUBLIC_FILES {
        write(root, name, content);
    }
    cargo(root, &["generate-lockfile", "--offline"]);
    if git {
        let output = Command::new("git")
            .current_dir(root)
            .args(["init", "--quiet"])
            .output()
            .unwrap();
        assert!(output.status.success());
        // Public source must ship even if an ignore rule would hide it; local
        // material must stay excluded even when deliberately staged in Git.
        write(root, ".gitignore", "/src/required.rs\n/.tmp/\n/review/\n");
    }
    for name in LOCAL_FILES {
        write(root, name, "synthetic local data\n");
    }
    if git {
        let output = Command::new("git")
            .current_dir(root)
            .args(["add", "--force", "."])
            .output()
            .unwrap();
        assert!(output.status.success());
        let output = Command::new("git")
            .current_dir(root)
            .args(["rm", "--cached", "--quiet", "src/required.rs"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let output = Command::new("git")
            .current_dir(root)
            .args([
                "-c",
                "user.name=PackageFixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=disabled-hooks",
                "commit",
                "--quiet",
                "-m",
                "Package policy fixture",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new("git")
            .current_dir(root)
            .args(["check-ignore", "--quiet", "src/required.rs"])
            .output()
            .unwrap();
        assert!(output.status.success());
        write(
            root,
            "untracked-review/notes.md",
            "synthetic untracked data\n",
        );
        write(root, "review/ignored.md", "synthetic ignored data\n");
    }
    temp
}

fn assert_selection(root: &Path) {
    let listed = cargo(
        root,
        &[
            "package",
            "--list",
            "--locked",
            "--offline",
            "--allow-dirty",
        ],
    );
    let files: BTreeSet<_> = String::from_utf8(listed.stdout)
        .unwrap()
        .lines()
        .map(|path| path.replace('\\', "/"))
        .collect();
    for path in LOCAL_FILES {
        assert!(!files.contains(*path), "local material packaged: {path}");
    }
    for path in ["untracked-review/notes.md", "review/ignored.md"] {
        assert!(!files.contains(path), "local material packaged: {path}");
    }
    for (path, _) in PUBLIC_FILES {
        assert!(
            files.contains(*path),
            "missing published file {path}: {files:?}"
        );
    }
    for path in ["Cargo.toml", "Cargo.lock"] {
        assert!(files.contains(path), "missing {path}");
    }
    // Also build Cargo's extracted archive, catching an over-tight filter that
    // passes the negative checks but makes the published crate unusable.
    cargo(root, &["package", "--locked", "--offline", "--allow-dirty"]);
}

#[test]
fn package_policy_excludes_local_files_without_git_and_retains_build_inputs() {
    let fixture = fixture(false);
    assert_selection(fixture.path());
}

#[test]
fn package_policy_is_independent_of_tracked_untracked_and_ignored_local_files() {
    let fixture = fixture(true);
    assert_selection(fixture.path());
}
