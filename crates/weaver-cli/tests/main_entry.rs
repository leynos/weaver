use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn capabilities_probe_succeeds() {
    let mut command = Command::cargo_bin("weaver-cli").expect("resolve binary");
    command.arg("--capabilities");
    command.assert().success();
}

#[test]
fn missing_operation_exits_with_failure() {
    let mut command = Command::cargo_bin("weaver-cli").expect("resolve binary");
    command.arg("observe");
    command
        .assert()
        .failure()
        .stderr(contains("command operation must be provided"));
}
