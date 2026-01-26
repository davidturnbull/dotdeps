use std::path::Path;
use std::process::Command;

use super::harness::{
    CommandOutput, TestContext, TestEnv, ensure_dir, parse_json, read_file, symlink_dir, write_file,
};

pub struct Scenario {
    pub name: &'static str,
    pub run: fn(&TestContext) -> Result<(), String>,
}

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "help_output",
            run: scenario_help,
        },
        Scenario {
            name: "no_args_error",
            run: scenario_no_args,
        },
        Scenario {
            name: "invalid_spec",
            run: scenario_invalid_spec,
        },
        Scenario {
            name: "invalid_ecosystem",
            run: scenario_invalid_ecosystem,
        },
        Scenario {
            name: "clean_with_subcommand",
            run: scenario_clean_with_subcommand,
        },
        Scenario {
            name: "context_empty_dir",
            run: scenario_context_empty_dir,
        },
        Scenario {
            name: "list_empty_dir",
            run: scenario_list_empty_dir,
        },
        Scenario {
            name: "list_json_empty_dir",
            run: scenario_list_json_empty_dir,
        },
        Scenario {
            name: "symlink_list_remove_clean",
            run: scenario_symlink_list_remove_clean,
        },
        Scenario {
            name: "init_fresh_directory",
            run: scenario_init_fresh,
        },
        Scenario {
            name: "init_already_initialized",
            run: scenario_init_already_done,
        },
        Scenario {
            name: "init_with_existing_claude_md",
            run: scenario_init_with_claude_md,
        },
        Scenario {
            name: "init_with_existing_agents_md",
            run: scenario_init_with_agents_md,
        },
        Scenario {
            name: "init_dry_run",
            run: scenario_init_dry_run,
        },
        Scenario {
            name: "init_json_output",
            run: scenario_init_json,
        },
        Scenario {
            name: "init_skip_flags",
            run: scenario_init_skip_flags,
        },
        Scenario {
            name: "init_gitignore_already_has_deps",
            run: scenario_init_gitignore_has_deps,
        },
        Scenario {
            name: "python_requirements",
            run: scenario_python_requirements,
        },
        Scenario {
            name: "python_pyproject",
            run: scenario_python_pyproject,
        },
        Scenario {
            name: "python_poetry_lock",
            run: scenario_python_poetry_lock,
        },
        Scenario {
            name: "python_uv_lock",
            run: scenario_python_uv_lock,
        },
        Scenario {
            name: "node_npm_lock",
            run: scenario_node_npm_lock,
        },
        Scenario {
            name: "node_pnpm_lock",
            run: scenario_node_pnpm_lock,
        },
        Scenario {
            name: "node_bun_lock",
            run: scenario_node_bun_lock,
        },
        Scenario {
            name: "node_yarn_lock",
            run: scenario_node_yarn_lock,
        },
        Scenario {
            name: "go_mod_sum",
            run: scenario_go_mod_sum,
        },
        Scenario {
            name: "rust_cargo",
            run: scenario_rust_cargo,
        },
        Scenario {
            name: "ruby_bundle",
            run: scenario_ruby_bundle,
        },
        Scenario {
            name: "swift_package",
            run: scenario_swift_package,
        },
        Scenario {
            name: "multi_ecosystem_context",
            run: scenario_multi_ecosystem_context,
        },
    ]
}

fn scenario_help(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("help")?;
    let output = ctx.run_dotdeps(&env, &["--help"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains("context")?;
    Ok(())
}

fn scenario_no_args(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("no-args")?;
    let output = ctx.run_dotdeps(&env, &[], &env.root)?;
    output.assert_failure()?;
    output.assert_stderr_contains("No command specified")?;
    Ok(())
}

fn scenario_invalid_spec(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("invalid-spec")?;
    let output = ctx.run_dotdeps(&env, &["add", "pythonrequests"], &env.root)?;
    output.assert_failure()?;
    output.assert_stderr_contains("Invalid format")?;
    Ok(())
}

fn scenario_invalid_ecosystem(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("invalid-ecosystem")?;
    let output = ctx.run_dotdeps(&env, &["add", "java:foo"], &env.root)?;
    output.assert_failure()?;
    output.assert_stderr_contains("Unknown ecosystem")?;
    Ok(())
}

fn scenario_clean_with_subcommand(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("clean-subcommand")?;
    let output = ctx.run_dotdeps(&env, &["--clean", "list"], &env.root)?;
    output.assert_failure()?;
    output.assert_stderr_contains("unexpected argument '--clean'")?;
    Ok(())
}

fn scenario_context_empty_dir(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("context-empty")?;
    let output = ctx.run_dotdeps(&env, &["context"], &env.root)?;
    output.assert_success()?;
    if !output.stdout.trim().is_empty() {
        return Err(format!(
            "Expected empty context output, got: {}",
            output.stdout
        ));
    }
    Ok(())
}

fn scenario_list_empty_dir(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("list-empty")?;
    let output = ctx.run_dotdeps(&env, &["list"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains("No dependencies in .deps/")?;
    Ok(())
}

fn scenario_list_json_empty_dir(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("list-json-empty")?;
    let output = ctx.run_dotdeps(&env, &["list", "--json"], &env.root)?;
    output.assert_success()?;
    let value = parse_json(&output.stdout)?;
    if value
        .get("dependencies")
        .and_then(|v| v.as_array())
        .map(|arr| arr.is_empty())
        != Some(true)
    {
        return Err("Expected empty dependencies array".to_string());
    }
    Ok(())
}

fn scenario_symlink_list_remove_clean(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("symlink-list-remove")?;
    let cache_path = env
        .xdg_cache
        .join("dotdeps")
        .join("python")
        .join("requests")
        .join("2.31.0");
    ensure_dir(&cache_path)?;
    write_file(&cache_path.join("README.md"), "fake")?;

    let deps_path = env.root.join(".deps").join("python").join("requests");
    ensure_dir(deps_path.parent().unwrap())?;
    symlink_dir(&cache_path, &deps_path)?;

    let list = ctx.run_dotdeps(&env, &["list"], &env.root)?;
    list.assert_success()?;
    list.assert_stdout_contains("python:requests@2.31.0")?;

    let remove = ctx.run_dotdeps(&env, &["remove", "python:requests"], &env.root)?;
    remove.assert_success()?;

    let list_after = ctx.run_dotdeps(&env, &["list"], &env.root)?;
    list_after.assert_success()?;
    list_after.assert_stdout_contains("No dependencies in .deps/")?;

    let clean = ctx.run_dotdeps(&env, &["clean"], &env.root)?;
    clean.assert_success()?;
    Ok(())
}

// =============================================================================
// Init command scenarios
// =============================================================================

fn scenario_init_fresh(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-fresh")?;

    let output = ctx.run_dotdeps(&env, &["init"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains("Created .deps/")?;
    output.assert_stdout_contains("Added \".deps/\" to .gitignore")?;
    output.assert_stdout_contains("Created AGENTS.md with dotdeps instructions")?;

    // Verify files created
    if !env.root.join(".deps").is_dir() {
        return Err(".deps directory was not created".to_string());
    }
    if !env.root.join(".gitignore").exists() {
        return Err(".gitignore was not created".to_string());
    }
    if !env.root.join("AGENTS.md").exists() {
        return Err("AGENTS.md was not created".to_string());
    }

    let gitignore = read_file(&env.root.join(".gitignore"))?;
    if !gitignore.contains(".deps/") {
        return Err(".gitignore does not contain .deps/".to_string());
    }

    let agents_md = read_file(&env.root.join("AGENTS.md"))?;
    if !agents_md.contains("<!-- dotdeps:instructions -->") {
        return Err("AGENTS.md does not contain marker comment".to_string());
    }

    Ok(())
}

fn scenario_init_already_done(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-already")?;

    // First init
    ctx.run_dotdeps(&env, &["init"], &env.root)?
        .assert_success()?;

    // Second init should skip everything
    let output = ctx.run_dotdeps(&env, &["init"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains("already initialized")?;

    Ok(())
}

fn scenario_init_with_claude_md(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-claude-md")?;
    write_file(
        &env.root.join("CLAUDE.md"),
        "# Existing Instructions\n\nSome content.",
    )?;

    let output = ctx.run_dotdeps(&env, &["init"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains("Added dotdeps instructions to CLAUDE.md")?;

    let content = read_file(&env.root.join("CLAUDE.md"))?;
    if !content.contains("# Existing Instructions") {
        return Err("CLAUDE.md lost existing content".to_string());
    }
    if !content.contains("<!-- dotdeps:instructions -->") {
        return Err("CLAUDE.md does not contain marker comment".to_string());
    }

    Ok(())
}

fn scenario_init_with_agents_md(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-agents-md")?;
    write_file(&env.root.join("AGENTS.md"), "# Agent Instructions\n")?;

    let output = ctx.run_dotdeps(&env, &["init"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains("Added dotdeps instructions to AGENTS.md")?;

    // Verify existing content preserved
    let content = read_file(&env.root.join("AGENTS.md"))?;
    if !content.contains("# Agent Instructions") {
        return Err("AGENTS.md lost existing content".to_string());
    }

    Ok(())
}

fn scenario_init_dry_run(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-dry-run")?;

    let output = ctx.run_dotdeps(&env, &["init", "--dry-run"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains("[dry-run]")?;

    // Nothing should be created
    if env.root.join(".deps").exists() {
        return Err(".deps was created in dry-run mode".to_string());
    }
    if env.root.join(".gitignore").exists() {
        return Err(".gitignore was created in dry-run mode".to_string());
    }
    if env.root.join("AGENTS.md").exists() {
        return Err("AGENTS.md was created in dry-run mode".to_string());
    }

    Ok(())
}

fn scenario_init_json(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-json")?;

    let output = ctx.run_dotdeps(&env, &["init", "--json"], &env.root)?;
    output.assert_success()?;

    let json = parse_json(&output.stdout)?;
    if json.get("initialized") != Some(&serde_json::Value::Bool(true)) {
        return Err("JSON output missing initialized: true".to_string());
    }
    let actions = json.get("actions").and_then(|v| v.as_array());
    if actions.map(|a| a.len()) != Some(3) {
        return Err(format!("Expected 3 actions, got {:?}", actions));
    }

    Ok(())
}

fn scenario_init_skip_flags(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-skip")?;

    let output = ctx.run_dotdeps(
        &env,
        &["init", "--skip-gitignore", "--skip-instructions"],
        &env.root,
    )?;
    output.assert_success()?;

    // Only .deps/ should be created
    if !env.root.join(".deps").is_dir() {
        return Err(".deps directory was not created".to_string());
    }
    if env.root.join(".gitignore").exists() {
        return Err(".gitignore was created despite --skip-gitignore".to_string());
    }
    if env.root.join("AGENTS.md").exists() {
        return Err("AGENTS.md was created despite --skip-instructions".to_string());
    }

    Ok(())
}

fn scenario_init_gitignore_has_deps(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("init-gitignore")?;
    write_file(&env.root.join(".gitignore"), "node_modules/\n.deps/\n")?;

    let output = ctx.run_dotdeps(&env, &["init"], &env.root)?;
    output.assert_success()?;
    output.assert_stdout_contains(".gitignore already includes .deps/")?;

    // .gitignore should be unchanged
    let content = read_file(&env.root.join(".gitignore"))?;
    if content != "node_modules/\n.deps/\n" {
        return Err(format!(".gitignore was modified: {:?}", content));
    }

    Ok(())
}

fn scenario_python_requirements(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "python3")?;
    require_cmd(ctx, "pip3")?;

    let env = ctx.create_env("python-req")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(ctx, &env, "python3", &["-m", "venv", ".venv"], &proj)?;
    let pip = proj.join(".venv").join("bin").join("pip");
    run_cmd_raw(&env, &pip, &["install", "-q", "requests==2.31.0"], &proj)?;

    let output = Command::new(&pip)
        .args(["freeze"])
        .current_dir(&proj)
        .output()
        .map_err(|e| format!("Failed to run pip freeze: {}", e))?;
    if !output.status.success() {
        return Err("pip freeze failed".to_string());
    }
    write_file(
        &proj.join("requirements.txt"),
        &String::from_utf8_lossy(&output.stdout),
    )?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add python:requests")?;

    let add = ctx.run_dotdeps(&env, &["add", "python:requests", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching requests 2.31.0")?;
    Ok(())
}

fn scenario_python_pyproject(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("python-pyproject")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;
    let pyproject = r#"[tool.poetry.dependencies]
python = "^3.11"
flask = "^2.3.0"
local = { path = "../local" }

[project]
dependencies = ["requests>=2.31.0", "SQLAlchemy==2.0.0"]
"#;
    write_file(&proj.join("pyproject.toml"), pyproject)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add python:flask")?;
    context.assert_stdout_contains("dotdeps add python:requests")?;
    context.assert_stdout_contains("dotdeps add python:sqlalchemy")?;
    context.assert_stdout_not_contains("dotdeps add python:python")?;
    context.assert_stdout_not_contains("dotdeps add python:local")?;

    let add = ctx.run_dotdeps(&env, &["add", "python:flask", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching flask 2.3.0")?;
    Ok(())
}

fn scenario_python_poetry_lock(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("python-poetry")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;
    let lock = r#"[[package]]
name = "requests"
version = "2.31.0"

[[package]]
name = "local-pkg"
version = "0.1.0"

[package.source]
type = "directory"
url = "../local"
"#;
    write_file(&proj.join("poetry.lock"), lock)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add python:requests")?;
    context.assert_stdout_not_contains("dotdeps add python:local-pkg")?;

    let add = ctx.run_dotdeps(&env, &["add", "python:requests", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching requests 2.31.0")?;
    Ok(())
}

fn scenario_python_uv_lock(ctx: &TestContext) -> Result<(), String> {
    let env = ctx.create_env("python-uv")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;
    let lock = r#"[[package]]
name = "flask"
version = "2.3.0"
"#;
    write_file(&proj.join("uv.lock"), lock)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add python:flask")?;

    let add = ctx.run_dotdeps(&env, &["add", "python:flask", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching flask 2.3.0")?;
    Ok(())
}

fn scenario_node_npm_lock(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "npm")?;
    let env = ctx.create_env("node-npm")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(ctx, &env, "npm", &["init", "-y"], &proj)?;
    run_cmd(
        ctx,
        &env,
        "npm",
        &["install", "-s", "lodash@4.17.21"],
        &proj,
    )?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add node:lodash")?;

    let add = ctx.run_dotdeps(&env, &["add", "node:lodash", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching lodash 4.17.21")?;
    Ok(())
}

fn scenario_node_pnpm_lock(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "pnpm")?;
    let env = ctx.create_env("node-pnpm")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;
    let local = env.root.join("localpkg");
    ensure_dir(&local)?;
    write_file(
        &local.join("package.json"),
        r#"{"name": "localpkg", "version": "1.0.0"}"#,
    )?;

    write_file(
        &proj.join("package.json"),
        r#"{"name": "pnpm-test", "version": "1.0.0"}"#,
    )?;
    run_cmd(ctx, &env, "pnpm", &["add", "lodash@4.17.21"], &proj)?;
    let local_spec = format!("file:{}", local.display());
    run_cmd_vec(ctx, &env, "pnpm", &[String::from("add"), local_spec], &proj)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add node:lodash")?;
    context.assert_stdout_not_contains("dotdeps add node:localpkg")?;

    let add = ctx.run_dotdeps(&env, &["add", "node:lodash", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching lodash 4.17.21")?;
    Ok(())
}

fn scenario_node_bun_lock(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "bun")?;
    let env = ctx.create_env("node-bun")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    write_file(
        &proj.join("package.json"),
        r#"{"name": "bun-test", "version": "1.0.0"}"#,
    )?;
    run_cmd(ctx, &env, "bun", &["add", "lodash@4.17.21"], &proj)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add node:lodash")?;

    let add = ctx.run_dotdeps(&env, &["add", "node:lodash", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching lodash 4.17.21")?;
    Ok(())
}

fn scenario_node_yarn_lock(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "npx")?;
    require_cmd(ctx, "npm")?;
    let env = ctx.create_env("node-yarn")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(ctx, &env, "npm", &["init", "-y"], &proj)?;
    run_cmd(
        ctx,
        &env,
        "npx",
        &[
            "-y",
            "yarn@1.22.22",
            "add",
            "lodash@4.17.21",
            "--ignore-scripts",
        ],
        &proj,
    )?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add node:lodash")?;

    let add = ctx.run_dotdeps(&env, &["add", "node:lodash", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching lodash 4.17.21")?;
    Ok(())
}

fn scenario_go_mod_sum(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "go")?;
    let env = ctx.create_env("go-mod")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(
        ctx,
        &env,
        "go",
        &["mod", "init", "example.com/dotdeps-test"],
        &proj,
    )?;
    write_file(
        &proj.join("main.go"),
        "package main\nimport \"github.com/gin-gonic/gin\"\nfunc main() { _ = gin.Default() }\n",
    )?;
    run_cmd(ctx, &env, "go", &["mod", "tidy"], &proj)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add go:github.com/gin-gonic/gin")?;

    let add = ctx.run_dotdeps(
        &env,
        &["add", "go:github.com/gin-gonic/gin", "--dry-run"],
        &proj,
    )?;
    add.assert_success()?;
    add.assert_stdout_contains("github.com/gin-gonic/gin")?;
    Ok(())
}

fn scenario_rust_cargo(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "cargo")?;
    let env = ctx.create_env("rust-cargo")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(
        ctx,
        &env,
        "cargo",
        &["init", "--lib", "--name", "dotdeps_test"],
        &proj,
    )?;
    let cargo_toml = read_file(&proj.join("Cargo.toml"))?;
    let updated = cargo_toml.replace("[dependencies]", "[dependencies]\nserde = \"1.0\"");
    write_file(&proj.join("Cargo.toml"), &updated)?;
    run_cmd(ctx, &env, "cargo", &["build"], &proj)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add rust:serde")?;

    let add = ctx.run_dotdeps(&env, &["add", "rust:serde", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("serde")?;
    Ok(())
}

fn scenario_ruby_bundle(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "bundle")?;
    let env = ctx.create_env("ruby-bundle")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(ctx, &env, "bundle", &["init"], &proj)?;
    let gemfile_path = proj.join("Gemfile");
    let gemfile = read_file(&gemfile_path)?;
    let updated = format!("{}\n\ngem \"rack\"\n", gemfile);
    write_file(&gemfile_path, &updated)?;
    run_cmd(ctx, &env, "bundle", &["lock"], &proj)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add ruby:rack")?;

    let add = ctx.run_dotdeps(&env, &["add", "ruby:rack", "--dry-run"], &proj)?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching rack")?;
    Ok(())
}

fn scenario_swift_package(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "swift")?;
    let env = ctx.create_env("swift-package")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(
        ctx,
        &env,
        "swift",
        &["package", "init", "--type", "executable"],
        &proj,
    )?;
    let package_swift = r#"// swift-tools-version: 5.7
import PackageDescription

let package = Package(
    name: "DotdepsSwift",
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser.git", from: "1.5.0")
    ],
    targets: [
        .executableTarget(
            name: "DotdepsSwift",
            dependencies: [
                .product(name: "ArgumentParser", package: "swift-argument-parser")
            ]
        )
    ]
)
"#;
    write_file(&proj.join("Package.swift"), package_swift)?;
    run_cmd(ctx, &env, "swift", &["package", "resolve"], &proj)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add swift:swift-argument-parser")?;

    let add = ctx.run_dotdeps(
        &env,
        &["add", "swift:swift-argument-parser", "--dry-run"],
        &proj,
    )?;
    add.assert_success()?;
    add.assert_stdout_contains("Fetching swift-argument-parser")?;
    Ok(())
}

fn scenario_multi_ecosystem_context(ctx: &TestContext) -> Result<(), String> {
    require_cmd(ctx, "npm")?;
    require_cmd(ctx, "python3")?;
    let env = ctx.create_env("multi-ecosystem")?;
    let proj = env.root.join("proj");
    ensure_dir(&proj)?;

    run_cmd(ctx, &env, "npm", &["init", "-y"], &proj)?;
    run_cmd(
        ctx,
        &env,
        "npm",
        &["install", "-s", "lodash@4.17.21"],
        &proj,
    )?;

    let pyproject = r#"[project]
dependencies = ["requests==2.31.0"]
"#;
    write_file(&proj.join("pyproject.toml"), pyproject)?;

    let context = ctx.run_dotdeps(&env, &["context"], &proj)?;
    context.assert_success()?;
    context.assert_stdout_contains("dotdeps add node:lodash")?;
    context.assert_stdout_contains("dotdeps add python:requests")?;
    Ok(())
}

fn require_cmd(ctx: &TestContext, cmd: &str) -> Result<(), String> {
    if ctx.command_available(cmd) {
        Ok(())
    } else {
        Err(format!("Required command not available: {}", cmd))
    }
}

fn run_cmd(
    ctx: &TestContext,
    env: &TestEnv,
    cmd: &str,
    args: &[&str],
    cwd: &Path,
) -> Result<(), String> {
    let _ = env;
    let output = ctx.run_system_command(cmd, args, cwd)?;
    output.assert_success()?;
    Ok(())
}

fn run_cmd_vec(
    ctx: &TestContext,
    env: &TestEnv,
    cmd: &str,
    args: &[String],
    cwd: &Path,
) -> Result<(), String> {
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let _ = env;
    let output = ctx.run_system_command(cmd, &arg_refs, cwd)?;
    output.assert_success()?;
    Ok(())
}

fn run_cmd_raw(env: &TestEnv, cmd: &Path, args: &[&str], cwd: &Path) -> Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .env("HOME", &env.home)
        .env("XDG_CACHE_HOME", &env.xdg_cache)
        .env("XDG_CONFIG_HOME", &env.xdg_config)
        .output()
        .map_err(|e| format!("Failed to run command: {}", e))?;
    let result = CommandOutput::from_output(output);
    result.assert_success()?;
    Ok(())
}
