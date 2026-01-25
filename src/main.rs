mod cli;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    // Handle --clean flag (mutually exclusive with subcommands)
    if cli.clean {
        if cli.command.is_some() {
            eprintln!("Error: --clean cannot be used with a subcommand");
            std::process::exit(1);
        }
        println!("Cleaning .deps/ directory...");
        // TODO: Implement clean
        return;
    }

    match cli.command {
        Some(Command::Add { spec }) => {
            println!("Adding dependency: {}", spec);
            // TODO: Implement add
        }
        Some(Command::Remove { spec }) => {
            println!("Removing dependency: {}", spec);
            // TODO: Implement remove
        }
        Some(Command::List) => {
            println!("Listing dependencies...");
            // TODO: Implement list
        }
        None => {
            // No command and no --clean flag, show help
            eprintln!("No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}
