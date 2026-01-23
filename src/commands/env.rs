use crate::commands::{Command, CommandResult};
use crate::paths;
use crate::system;
use std::collections::HashMap;
use std::env;
use std::io::IsTerminal;

pub struct EnvCommand;

impl Command for EnvCommand {
    fn run(&self, args: &[String]) -> CommandResult {
        let mut plain = false;
        let mut shell: Option<String> = None;
        let mut formulae: Vec<String> = Vec::new();

        // Parse arguments
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--plain" {
                plain = true;
            } else if arg.starts_with("--shell=") {
                shell = Some(arg.trim_start_matches("--shell=").to_string());
            } else if arg == "--shell" && i + 1 < args.len() {
                i += 1;
                shell = Some(args[i].clone());
            } else if !arg.starts_with('-') {
                formulae.push(arg.clone());
            }
            i += 1;
        }

        // Determine output format
        let output_shell = if plain {
            None
        } else if let Some(s) = shell {
            if s == "auto" {
                Some(detect_shell())
            } else {
                Some(s)
            }
        } else if !std::io::stdout().is_terminal() {
            // When piped and no shell specified, default to bash
            Some("bash".to_string())
        } else {
            None
        };

        // Collect environment variables
        let env_vars = collect_build_environment(&formulae)?;

        // Output in appropriate format
        if let Some(shell_type) = output_shell {
            print_shell_export(&env_vars, &shell_type);
        } else {
            print_plain_format(&env_vars);
        }

        Ok(())
    }
}

fn collect_build_environment(
    formulae: &[String],
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut env = HashMap::new();

    // Compiler settings
    let cc = "clang";
    let cxx = "clang++";

    env.insert("CC".to_string(), cc.to_string());
    env.insert("CXX".to_string(), cxx.to_string());
    env.insert("OBJC".to_string(), cc.to_string());
    env.insert("OBJCXX".to_string(), cxx.to_string());
    env.insert("HOMEBREW_CC".to_string(), cc.to_string());
    env.insert("HOMEBREW_CXX".to_string(), cxx.to_string());

    // Make settings
    let num_cpus = num_cpus::get();
    env.insert("MAKEFLAGS".to_string(), format!("-j{}", num_cpus));
    env.insert("HOMEBREW_MAKE_JOBS".to_string(), num_cpus.to_string());

    // CMake settings
    let prefix = paths::homebrew_prefix();
    env.insert(
        "CMAKE_PREFIX_PATH".to_string(),
        prefix.display().to_string(),
    );

    // Get SDK path
    if let Some(sdkroot) = system::macos_sdk_path() {
        env.insert("HOMEBREW_SDKROOT".to_string(), sdkroot.clone());

        // CMake paths for OpenGL
        let opengl_headers = format!(
            "{}/System/Library/Frameworks/OpenGL.framework/Versions/Current/Headers",
            sdkroot
        );
        let opengl_libs = format!(
            "{}/System/Library/Frameworks/OpenGL.framework/Versions/Current/Libraries",
            sdkroot
        );
        env.insert("CMAKE_INCLUDE_PATH".to_string(), opengl_headers);
        env.insert("CMAKE_LIBRARY_PATH".to_string(), opengl_libs);
    }

    // PKG_CONFIG settings
    if let Some(version_str) = system::macos_version()
        && let Some(parsed_version) = system::parse_macos_version(&version_str)
    {
        let pkg_config = format!(
            "/usr/lib/pkgconfig:{}/Library/Homebrew/os/mac/pkgconfig/{}",
            prefix.display(),
            parsed_version.major
        );
        env.insert("PKG_CONFIG_LIBDIR".to_string(), pkg_config);
    }

    // Git
    env.insert("HOMEBREW_GIT".to_string(), "git".to_string());

    // ACLOCAL_PATH
    env.insert(
        "ACLOCAL_PATH".to_string(),
        format!("{}/share/aclocal", prefix.display()),
    );

    // PATH - build the superenv shims path
    let mut path_components = vec![format!(
        "{}/Library/Homebrew/shims/mac/super",
        prefix.display()
    )];

    // Add formula bin directories if specified
    for formula in formulae {
        let formula_bin = format!("{}/opt/{}/bin", prefix.display(), formula);
        path_components.push(formula_bin);
    }

    // Add standard paths
    path_components.extend_from_slice(&[
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ]);

    env.insert("PATH".to_string(), path_components.join(":"));

    Ok(env)
}

fn print_plain_format(env: &HashMap<String, String>) {
    // Order matters - match Homebrew's KEYS order from build_environment.rb
    let key_order = [
        "HOMEBREW_CC",
        "HOMEBREW_CXX",
        "MAKEFLAGS",
        "CMAKE_PREFIX_PATH",
        "CMAKE_INCLUDE_PATH",
        "CMAKE_LIBRARY_PATH",
        "PKG_CONFIG_LIBDIR",
        "HOMEBREW_MAKE_JOBS",
        "HOMEBREW_GIT",
        "HOMEBREW_SDKROOT",
        "ACLOCAL_PATH",
        "PATH",
    ];

    for key in &key_order {
        if let Some(value) = env.get(*key) {
            println!("{}: {}", key, value);
        }
    }
}

fn print_shell_export(env: &HashMap<String, String>, shell: &str) {
    // Order matters - output all variables in consistent order
    let key_order = [
        "CC",
        "CXX",
        "OBJC",
        "OBJCXX",
        "HOMEBREW_CC",
        "HOMEBREW_CXX",
        "MAKEFLAGS",
        "CMAKE_PREFIX_PATH",
        "CMAKE_INCLUDE_PATH",
        "CMAKE_LIBRARY_PATH",
        "PKG_CONFIG_LIBDIR",
        "HOMEBREW_MAKE_JOBS",
        "HOMEBREW_GIT",
        "HOMEBREW_SDKROOT",
        "ACLOCAL_PATH",
        "PATH",
    ];

    for key in &key_order {
        if let Some(value) = env.get(*key) {
            println!("{}", format_export(key, value, shell));
        }
    }
}

fn format_export(key: &str, value: &str, shell: &str) -> String {
    match shell {
        "bash" | "sh" | "zsh" | "ksh" | "mksh" => {
            format!("export {}=\"{}\"", key, shell_escape(value))
        }
        "fish" => {
            format!("set -gx {} \"{}\"", key, shell_escape(value))
        }
        "csh" | "tcsh" => {
            format!("setenv {} {};", key, csh_escape(value))
        }
        "rc" => {
            format!("{}=({})", key, shell_escape(value))
        }
        _ => {
            // Default to bash format
            format!("export {}=\"{}\"", key, shell_escape(value))
        }
    }
}

fn shell_escape(s: &str) -> String {
    // Escape special characters for shell
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

fn csh_escape(s: &str) -> String {
    // For csh/tcsh, use different escaping rules
    let mut result = String::new();
    for c in s.chars() {
        match c {
            ' ' | '\t' | '\n' | '\'' | '"' | '\\' | '&' | ';' | '<' | '>' | '(' | ')' | '$'
            | '`' | '|' | '*' | '?' | '[' | ']' | '#' | '~' | '=' | '%' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

fn detect_shell() -> String {
    // Try to detect shell from SHELL environment variable
    if let Ok(shell_path) = env::var("SHELL")
        && let Some(shell_name) = shell_path.split('/').next_back()
    {
        return shell_name.to_string();
    }

    // Default to bash
    "bash".to_string()
}
