use crate::commands::{Command, CommandResult};

pub struct Help;

impl Command for Help {
    fn run(&self, args: &[String]) -> CommandResult {
        // If a command is specified, show help for that command
        if !args.is_empty() {
            let cmd = &args[0];
            // TODO: Show command-specific help
            eprintln!("Help for command '{cmd}' not yet implemented.");
            return Ok(());
        }

        // Show general help (matching brew help output format)
        println!(
            r#"Example usage:
  brew search TEXT|/REGEX/
  brew info [FORMULA|CASK...]
  brew install FORMULA|CASK...
  brew update
  brew upgrade [FORMULA|CASK...]
  brew uninstall FORMULA|CASK...
  brew list [FORMULA|CASK...]

Troubleshooting:
  brew config
  brew doctor
  brew install --verbose --debug FORMULA|CASK

Contributing:
  brew create URL [--no-fetch]
  brew edit [FORMULA|CASK...]

Further help:
  brew commands
  brew help [COMMAND]
  man brew
  https://docs.brew.sh"#
        );

        Ok(())
    }
}
