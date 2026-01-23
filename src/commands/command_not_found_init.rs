use std::env;
use std::io::IsTerminal;

const HANDLER_SH: &str = r#"#
# Homebrew command-not-found script for macOS
#
# Usage: Source it somewhere in your .bashrc (bash) or .zshrc (zsh)
#
# Author: Baptiste Fontaine
# License: MIT
# The license text can be found in Library/Homebrew/command-not-found/LICENSE

if ! command -v brew >/dev/null; then return; fi

homebrew_command_not_found_handle() {
  local cmd="$1"

  if [[ -n "${ZSH_VERSION}" ]]
  then
    autoload is-at-least
  fi

  # The code below is based off this Linux Journal article:
  #   http://www.linuxjournal.com/content/bash-command-not-found

  # do not run when inside Midnight Commander or within a Pipe, except if CI
  # HOMEBREW_COMMAND_NOT_FOUND_CI is defined in the CI environment
  # MC_SID is defined when running inside Midnight Commander.
  # shellcheck disable=SC2154
  if test -z "${HOMEBREW_COMMAND_NOT_FOUND_CI}" && test -n "${MC_SID}" -o ! -t 1
  then
    [[ -n "${BASH_VERSION}" ]] &&
      TEXTDOMAIN=command-not-found echo $"${cmd}: command not found"
    # Zsh versions 5.3 and above don't print this for us.
    [[ -n "${ZSH_VERSION}" ]] && is-at-least "5.2" "${ZSH_VERSION}" &&
      echo "zsh: command not found: ${cmd}" >&2
    return 127
  fi

  if [[ "${cmd}" != "-h" ]] && [[ "${cmd}" != "--help" ]] && [[ "${cmd}" != "--usage" ]] && [[ "${cmd}" != "-?" ]]
  then
    local txt
    txt="$(brew which-formula --skip-update --explain "${cmd}" 2>/dev/null)"
  fi

  if [[ -z "${txt}" ]]
  then
    [[ -n "${BASH_VERSION}" ]] &&
      TEXTDOMAIN=command-not-found echo $"${cmd}: command not found"

    # Zsh versions 5.3 and above don't print this for us.
    [[ -n "${ZSH_VERSION}" ]] && is-at-least "5.2" "${ZSH_VERSION}" &&
      echo "zsh: command not found: ${cmd}" >&2
  else
    echo "${txt}"
  fi

  return 127
}

if [[ -n "${BASH_VERSION}" ]]
then
  command_not_found_handle() {
    homebrew_command_not_found_handle "$*"
    return $?
  }
elif [[ -n "${ZSH_VERSION}" ]]
then
  command_not_found_handler() {
    homebrew_command_not_found_handle "$*"
    return $?
  }
fi
"#;

const HANDLER_FISH: &str = r#"# See https://docs.brew.sh/Command-Not-Found for current setup instructions
# License: MIT
# The license text can be found in Library/Homebrew/command-not-found/LICENSE

function fish_command_not_found
    set -l cmd $argv[1]
    set -l txt

    if not contains -- "$cmd" "-h" "--help" "--usage" "-?"
        set txt (brew which-formula --skip-update --explain $cmd 2> /dev/null)
    end

    if test -z "$txt"
        __fish_default_command_not_found_handler $cmd
    else
        string collect $txt
    end
end

function __fish_command_not_found_handler --on-event fish_command_not_found
    fish_command_not_found $argv
end
"#;

pub fn execute(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let is_tty = std::io::stdout().is_terminal();
    let shell = detect_shell();

    if is_tty {
        print_help(&shell);
    } else {
        print_handler(&shell);
    }

    Ok(())
}

fn detect_shell() -> String {
    // Try parent process name first (may fail in sandboxes)
    if let Ok(ppid) = env::var("PPID")
        && let Ok(output) = std::process::Command::new("/bin/ps")
            .args(["-p", &ppid, "-c", "-o", "comm="])
            .output()
        && output.status.success()
    {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }

    // Fall back to $SHELL
    if let Ok(shell) = env::var("SHELL")
        && let Some(name) = shell.split('/').next_back()
    {
        return name.to_string();
    }

    // Default to bash
    "bash".to_string()
}

fn print_handler(shell: &str) {
    match shell {
        "fish" | "-fish" => {
            print!("{}", HANDLER_FISH);
        }
        _ => {
            // Default to bash/zsh handler for all other shells
            print!("{}", HANDLER_SH);
        }
    }
}

fn print_help(shell: &str) {
    match shell {
        "fish" | "-fish" => {
            println!("# To enable command-not-found");
            println!("# Add the following line to ~/.config/fish/config.fish");
            println!();
            println!(
                "set HOMEBREW_COMMAND_NOT_FOUND_HANDLER (brew --repository)/Library/Homebrew/command-not-found/handler.fish"
            );
            println!("if test -f $HOMEBREW_COMMAND_NOT_FOUND_HANDLER");
            println!("  source $HOMEBREW_COMMAND_NOT_FOUND_HANDLER");
            println!("end");
        }
        "bash" | "-bash" => {
            println!("# To enable command-not-found");
            println!("# Add the following lines to ~/.bashrc");
            println!();
            println!(
                "HOMEBREW_COMMAND_NOT_FOUND_HANDLER=\"$(brew --repository)/Library/Homebrew/command-not-found/handler.sh\""
            );
            println!("if [ -f \"$HOMEBREW_COMMAND_NOT_FOUND_HANDLER\" ]; then");
            println!("  source \"$HOMEBREW_COMMAND_NOT_FOUND_HANDLER\";");
            println!("fi");
        }
        _ => {
            // Default to zsh for all other shells
            println!("# To enable command-not-found");
            println!("# Add the following lines to ~/.zshrc");
            println!();
            println!(
                "HOMEBREW_COMMAND_NOT_FOUND_HANDLER=\"$(brew --repository)/Library/Homebrew/command-not-found/handler.sh\""
            );
            println!("if [ -f \"$HOMEBREW_COMMAND_NOT_FOUND_HANDLER\" ]; then");
            println!("  source \"$HOMEBREW_COMMAND_NOT_FOUND_HANDLER\";");
            println!("fi");
        }
    }
}
