use crate::commands::{Command, CommandResult};

pub struct Help;

impl Command for Help {
    fn run(&self, args: &[String]) -> CommandResult {
        // If a command is specified, show help for that command
        if !args.is_empty() {
            let cmd = &args[0];
            if let Some(help_text) = get_command_help(cmd) {
                println!("{help_text}");
                return Ok(());
            }
            // Unknown command - show generic help then error (matching brew behavior)
            println!("{GENERAL_HELP}");
            println!();
            eprintln!("Error: Invalid usage: Unknown command: brew {cmd}");
            // Exit with failure but don't print another error from main
            std::process::exit(1);
        }

        // Show general help (matching brew help output format)
        println!("{GENERAL_HELP}");

        Ok(())
    }
}

const GENERAL_HELP: &str = r#"Example usage:
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
  https://docs.brew.sh"#;

/// Get help text for a specific command.
fn get_command_help(cmd: &str) -> Option<&'static str> {
    // Normalize command name (handle aliases)
    let cmd = match cmd {
        "-v" => "--version",
        "--repo" => "--repository",
        _ => cmd,
    };

    match cmd {
        "--version" => Some(HELP_VERSION),
        "--prefix" => Some(HELP_PREFIX),
        "--cellar" => Some(HELP_CELLAR),
        "--cache" => Some(HELP_CACHE),
        "--repository" => Some(HELP_REPOSITORY),
        "--caskroom" => Some(HELP_CASKROOM),
        "--taps" => Some(HELP_TAPS),
        "commands" => Some(HELP_COMMANDS),
        "config" => Some(HELP_CONFIG),
        "help" => Some(HELP_HELP),
        _ => None,
    }
}

const HELP_VERSION: &str = r#"Usage: brew --version, -v

Print the version numbers of Homebrew, Homebrew/homebrew-core and
Homebrew/homebrew-cask (if tapped) to standard output.

  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_PREFIX: &str = r#"Usage: brew --prefix [--unbrewed] [--installed] [formula ...]

Display Homebrew's install path. Default:

  - macOS ARM: /opt/homebrew
  - macOS Intel: /usr/local
  - Linux: /home/linuxbrew/.linuxbrew

If formula is provided, display the location where formula is or would be
installed.

      --unbrewed                   List files in Homebrew's prefix not installed
                                   by Homebrew.
      --installed                  Outputs nothing and returns a failing status
                                   code if formula is not installed.
  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_CELLAR: &str = r#"Usage: brew --cellar [formula ...]

Display Homebrew's Cellar path. Default: $(brew --prefix)/Cellar, or if that
directory doesn't exist, $(brew --repository)/Cellar.

If formula is provided, display the location in the Cellar where formula
would be installed, without any sort of versioned directory as the last path.

  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_CACHE: &str = r#"Usage: brew --cache [options] [formula|cask ...]

Display Homebrew's download cache. See also $HOMEBREW_CACHE.

If a formula or cask is provided, display the file or directory used to
cache it.

      --os                         Show cache file for the given operating
                                   system. (Pass all to show cache files for
                                   all operating systems.)
      --arch                       Show cache file for the given CPU
                                   architecture. (Pass all to show cache files
                                   for all architectures.)
  -s, --build-from-source          Show the cache file used when building from
                                   source.
      --force-bottle               Show the cache file used when pouring a
                                   bottle.
      --bottle-tag                 Show the cache file used when pouring a
                                   bottle for the given tag.
      --HEAD                       Show the cache file used when building from
                                   HEAD.
      --formula, --formulae        Only show cache files for formulae.
      --cask, --casks              Only show cache files for casks.
  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_REPOSITORY: &str = r#"Usage: brew --repository, --repo [tap ...]

Display where Homebrew's Git repository is located.

If user/repo are provided, display where tap user/repo's directory
is located.

  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_CASKROOM: &str = r#"Usage: brew --caskroom [cask ...]

Display Homebrew's Caskroom path.

If cask is provided, display the location in the Caskroom where cask would
be installed, without any sort of versioned directory as the last path.

  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_TAPS: &str = r#"Usage: brew --taps

List all installed taps.

  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_COMMANDS: &str = r#"Usage: brew commands [--quiet] [--include-aliases]

Show lists of built-in and external commands.

  -q, --quiet                      List only the names of commands without
                                   category headers.
      --include-aliases            Include aliases of internal commands.
  -d, --debug                      Display any debugging information.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_CONFIG: &str = r#"Usage: brew config, --config

Show Homebrew and system configuration info useful for debugging. If you file a
bug report, you will be required to provide this information.

  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;

const HELP_HELP: &str = r#"Usage: brew help [command]

Outputs the usage instructions for brew command.
Equivalent to brew --help command.

  -d, --debug                      Display any debugging information.
  -q, --quiet                      Make some output more quiet.
  -v, --verbose                    Make some output more verbose.
  -h, --help                       Show this message."#;
