use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gitpub")]
#[command(about = "A CLI for interacting with gitpub repositories", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        name: String,
    },
    Clone {
        url: String,
        #[arg(short, long)]
        directory: Option<String>,
    },
    Push {
        #[arg(default_value = "origin")]
        remote: String,
        #[arg(default_value = "main")]
        branch: String,
    },
    Pull {
        #[arg(default_value = "origin")]
        remote: String,
        #[arg(default_value = "main")]
        branch: String,
    },
}

#[test]
fn test_cli_help() {
    let result = Cli::try_parse_from(["gitpub", "--help"]);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn test_cli_version() {
    let result = Cli::try_parse_from(["gitpub", "--version"]);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
}

#[test]
fn test_init_command_parsing() {
    let cli = Cli::parse_from(["gitpub", "init", "test-repo"]);

    match cli.command {
        Commands::Init { name } => {
            assert_eq!(name, "test-repo");
        }
        _ => panic!("Expected Init command"),
    }
}

#[test]
fn test_clone_command_parsing() {
    let cli = Cli::parse_from(["gitpub", "clone", "https://example.com/repo.git"]);

    match cli.command {
        Commands::Clone { url, directory } => {
            assert_eq!(url, "https://example.com/repo.git");
            assert!(directory.is_none());
        }
        _ => panic!("Expected Clone command"),
    }
}

#[test]
fn test_clone_command_with_directory() {
    let cli = Cli::parse_from([
        "gitpub",
        "clone",
        "https://example.com/repo.git",
        "-d",
        "my-dir",
    ]);

    match cli.command {
        Commands::Clone { url, directory } => {
            assert_eq!(url, "https://example.com/repo.git");
            assert_eq!(directory, Some("my-dir".to_string()));
        }
        _ => panic!("Expected Clone command"),
    }
}

#[test]
fn test_push_command_defaults() {
    let cli = Cli::parse_from(["gitpub", "push"]);

    match cli.command {
        Commands::Push { remote, branch } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "main");
        }
        _ => panic!("Expected Push command"),
    }
}

#[test]
fn test_push_command_with_args() {
    let cli = Cli::parse_from(["gitpub", "push", "upstream", "develop"]);

    match cli.command {
        Commands::Push { remote, branch } => {
            assert_eq!(remote, "upstream");
            assert_eq!(branch, "develop");
        }
        _ => panic!("Expected Push command"),
    }
}

#[test]
fn test_pull_command_defaults() {
    let cli = Cli::parse_from(["gitpub", "pull"]);

    match cli.command {
        Commands::Pull { remote, branch } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "main");
        }
        _ => panic!("Expected Pull command"),
    }
}

#[test]
fn test_pull_command_with_args() {
    let cli = Cli::parse_from(["gitpub", "pull", "upstream", "develop"]);

    match cli.command {
        Commands::Pull { remote, branch } => {
            assert_eq!(remote, "upstream");
            assert_eq!(branch, "develop");
        }
        _ => panic!("Expected Pull command"),
    }
}

#[test]
fn test_invalid_command() {
    let result = Cli::try_parse_from(["gitpub", "invalid"]);
    assert!(result.is_err());
}

#[test]
fn test_init_requires_name() {
    let result = Cli::try_parse_from(["gitpub", "init"]);
    assert!(result.is_err());
}

#[test]
fn test_clone_requires_url() {
    let result = Cli::try_parse_from(["gitpub", "clone"]);
    assert!(result.is_err());
}
