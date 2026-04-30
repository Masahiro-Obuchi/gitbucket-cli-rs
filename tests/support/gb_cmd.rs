#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

pub fn gb_command() -> std::process::Command {
    assert_cmd::cargo::CommandCargoExt::cargo_bin("gb").unwrap()
}

pub struct GbTestEnv {
    temp: TempDir,
}

impl GbTestEnv {
    pub fn new() -> Self {
        Self {
            temp: tempfile::tempdir().unwrap(),
        }
    }

    pub fn path(&self) -> &Path {
        self.temp.path()
    }

    pub fn command(&self) -> Command {
        let mut command = gb_command();
        command
            .current_dir(self.path())
            .env("GB_CONFIG_DIR", self.path())
            .env("NO_COLOR", "1");
        command
    }

    pub fn api_command(&self, host: impl AsRef<str>) -> Command {
        let mut command = self.command();
        command
            .env("GB_HOST", host.as_ref())
            .env("GB_TOKEN", "test-token")
            .env("GB_PROTOCOL", "http");
        command
    }

    pub fn repo_api_command(&self, host: impl AsRef<str>, repo: impl AsRef<str>) -> Command {
        let mut command = self.api_command(host);
        command.env("GB_REPO", repo.as_ref());
        command
    }
}

pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
