use crate::{paths, system};
use std::env;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Get shell name from args, parent process, or $SHELL
    let shell_name = if !args.is_empty() {
        args[0].clone()
    } else {
        detect_shell()
    };

    let prefix = paths::homebrew_prefix();
    let prefix_str = prefix.to_string_lossy().to_string();
    let cellar = paths::homebrew_cellar();
    let cellar_str = cellar.to_string_lossy().to_string();
    let repository = paths::homebrew_repository();
    let repository_str = repository.to_string_lossy().to_string();

    // Check if we should use path_helper (macOS 14.0+)
    // Also check if /usr/libexec/path_helper exists
    let macos_version_str = system::macos_version();
    let use_path_helper = if let Some(version_str) = macos_version_str {
        if let Some(version) = system::parse_macos_version(&version_str) {
            let version_ok = version.major >= 14 || (version.major == 10 && version.minor >= 10);
            version_ok && std::path::Path::new("/usr/libexec/path_helper").exists()
        } else {
            false
        }
    } else {
        false
    };

    // Ensure paths file exists on macOS 10.10+
    if use_path_helper {
        let paths_file = format!("{}/etc/paths", prefix_str);
        if !std::path::Path::new(&paths_file).exists() {
            if let Some(parent) = std::path::Path::new(&paths_file).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                &paths_file,
                format!("{}/bin\n{}/sbin\n", prefix_str, prefix_str),
            );
        }
    }

    match shell_name.as_str() {
        "fish" | "-fish" => {
            println!("set --global --export HOMEBREW_PREFIX \"{}\";", prefix_str);
            println!("set --global --export HOMEBREW_CELLAR \"{}\";", cellar_str);
            println!(
                "set --global --export HOMEBREW_REPOSITORY \"{}\";",
                repository_str
            );
            println!(
                "fish_add_path --global --move --path \"{}/bin\" \"{}/sbin\";",
                prefix_str, prefix_str
            );
            println!("if test -n \"$MANPATH[1]\"; set --global --export MANPATH '' $MANPATH; end;");
            println!(
                "if not contains \"{}/share/info\" $INFOPATH; set --global --export INFOPATH \"{}/share/info\" $INFOPATH; end;",
                prefix_str, prefix_str
            );
        }
        "csh" | "-csh" | "tcsh" | "-tcsh" => {
            println!("setenv HOMEBREW_PREFIX {};", prefix_str);
            println!("setenv HOMEBREW_CELLAR {};", cellar_str);
            println!("setenv HOMEBREW_REPOSITORY {};", repository_str);
            if use_path_helper {
                println!(
                    "eval `/usr/bin/env PATH_HELPER_ROOT=\"{}\" /usr/libexec/path_helper -c`;",
                    prefix_str
                );
            } else {
                println!("setenv PATH {}/bin:{}/sbin:$PATH;", prefix_str, prefix_str);
            }
            println!("test $?MANPATH -eq 1 && setenv MANPATH :${{MANPATH}};");
            println!(
                "setenv INFOPATH {}/share/info`test $?INFOPATH -eq 1 && echo :${{INFOPATH}}`;",
                prefix_str
            );
        }
        "pwsh" | "-pwsh" | "pwsh-preview" | "-pwsh-preview" => {
            println!(
                "[System.Environment]::SetEnvironmentVariable('HOMEBREW_PREFIX','{}' ,[System.EnvironmentVariableTarget]::Process)",
                prefix_str
            );
            println!(
                "[System.Environment]::SetEnvironmentVariable('HOMEBREW_CELLAR','{}' ,[System.EnvironmentVariableTarget]::Process)",
                cellar_str
            );
            println!(
                "[System.Environment]::SetEnvironmentVariable('HOMEBREW_REPOSITORY','{}' ,[System.EnvironmentVariableTarget]::Process)",
                repository_str
            );
            println!(
                "[System.Environment]::SetEnvironmentVariable('PATH',$('{}/bin:{}/sbin:'+$ENV:PATH),[System.EnvironmentVariableTarget]::Process)",
                prefix_str, prefix_str
            );
            println!(
                "[System.Environment]::SetEnvironmentVariable('MANPATH',('{}/share/man'+$(if(${{ENV:MANPATH}}){{':'+${{ENV:MANPATH}}}})+':'),[System.EnvironmentVariableTarget]::Process)",
                prefix_str
            );
            println!(
                "[System.Environment]::SetEnvironmentVariable('INFOPATH',('{}/share/info'+$(if(${{ENV:INFOPATH}}){{':'+${{ENV:INFOPATH}}}})),[System.EnvironmentVariableTarget]::Process)",
                prefix_str
            );
        }
        _ => {
            // Default: bash/zsh/sh
            println!("export HOMEBREW_PREFIX=\"{}\";", prefix_str);
            println!("export HOMEBREW_CELLAR=\"{}\";", cellar_str);
            println!("export HOMEBREW_REPOSITORY=\"{}\";", repository_str);

            // Add fpath for zsh
            if shell_name == "zsh" || shell_name == "-zsh" {
                println!("fpath[1,0]=\"{}/share/zsh/site-functions\";", prefix_str);
            }

            if use_path_helper {
                println!(
                    "eval \"$(/usr/bin/env PATH_HELPER_ROOT=\"{}\" /usr/libexec/path_helper -s)\"",
                    prefix_str
                );
            } else {
                println!(
                    "export PATH=\"{}/bin:{}/sbin${{PATH+:$PATH}}\";",
                    prefix_str, prefix_str
                );
            }

            println!("[ -z \"${{MANPATH-}}\" ] || export MANPATH=\":${{MANPATH#:}}\";");
            println!(
                "export INFOPATH=\"{}/share/info:${{INFOPATH:-}}\";",
                prefix_str
            );
        }
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
