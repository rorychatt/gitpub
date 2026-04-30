use std::process::Command;

fn gitpub_cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gitpub-cli"))
}

#[test]
fn test_cli_no_args_shows_error() {
    let output = gitpub_cli().output().expect("failed to execute");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage") || stderr.contains("gitpub"));
}

#[test]
fn test_cli_help_flag() {
    let output = gitpub_cli()
        .arg("--help")
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gitpub"));
}

#[test]
fn test_cli_init_command() {
    let output = gitpub_cli()
        .args(["init", "test-repo"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Initializing repository: test-repo"));
}

#[test]
fn test_cli_init_missing_name() {
    let output = gitpub_cli()
        .arg("init")
        .output()
        .expect("failed to execute");
    assert!(!output.status.success());
}

#[test]
fn test_cli_clone_command() {
    let output = gitpub_cli()
        .args(["clone", "https://gitpub.example.com/user/repo"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cloning"));
}

#[test]
fn test_cli_push_command_defaults() {
    let output = gitpub_cli()
        .arg("push")
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Pushing to origin/main"));
}

#[test]
fn test_cli_push_command_custom_remote() {
    let output = gitpub_cli()
        .args(["push", "upstream", "develop"])
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Pushing to upstream/develop"));
}

#[test]
fn test_cli_pull_command_defaults() {
    let output = gitpub_cli()
        .arg("pull")
        .output()
        .expect("failed to execute");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Pulling from origin/main"));
}

#[test]
fn test_cli_invalid_subcommand() {
    let output = gitpub_cli()
        .arg("invalid-command")
        .output()
        .expect("failed to execute");
    assert!(!output.status.success());
}
