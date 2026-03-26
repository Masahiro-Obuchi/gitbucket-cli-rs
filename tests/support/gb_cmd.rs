pub fn gb_command() -> std::process::Command {
    assert_cmd::cargo::CommandCargoExt::cargo_bin("gb").unwrap()
}
