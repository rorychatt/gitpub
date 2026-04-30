use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "gitpub")]
#[command(about = "A CLI for interacting with gitpub repositories", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new gitpub repository
    Init {
        /// Repository name
        name: String,
    },
    /// Clone a repository from gitpub
    Clone {
        /// Repository URL
        url: String,
        /// Target directory
        #[arg(short, long)]
        directory: Option<String>,
    },
    /// Push changes to gitpub
    Push {
        /// Remote name
        #[arg(default_value = "origin")]
        remote: String,
        /// Branch name
        #[arg(default_value = "main")]
        branch: String,
    },
    /// Pull changes from gitpub
    Pull {
        /// Remote name
        #[arg(default_value = "origin")]
        remote: String,
        /// Branch name
        #[arg(default_value = "main")]
        branch: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            println!("Initializing repository: {}", name);
            // TODO: Implement repository initialization
            Ok(())
        }
        Commands::Clone { url, directory } => {
            let target = directory.unwrap_or_else(|| {
                url.split('/').last().unwrap_or("repo").to_string()
            });
            println!("Cloning {} to {}", url, target);
            // TODO: Implement repository cloning
            Ok(())
        }
        Commands::Push { remote, branch } => {
            println!("Pushing to {}/{}", remote, branch);
            // TODO: Implement push operation
            Ok(())
        }
        Commands::Pull { remote, branch } => {
            println!("Pulling from {}/{}", remote, branch);
            // TODO: Implement pull operation
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Basic test that CLI parsing works
        let result = std::panic::catch_unwind(|| {
            Cli::parse_from(&["gitpub", "init", "test-repo"]);
        });
        assert!(result.is_ok());
    }
}
