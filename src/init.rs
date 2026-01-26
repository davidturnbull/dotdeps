//! Initialize dotdeps in the current directory
//!
//! Creates .deps/ directory, updates .gitignore, and adds usage instructions
//! to CLAUDE.md or AGENTS.md.

use std::fs;
use std::io;
use std::path::Path;

/// Marker comment to detect existing dotdeps instructions
const MARKER_COMMENT: &str = "<!-- dotdeps:instructions -->";

/// Instructions to add to CLAUDE.md/AGENTS.md
const INSTRUCTIONS: &str = r#"<!-- dotdeps:instructions -->
## Dependency Source Code

Before writing code that uses external libraries, fetch and read their source to ensure accuracy:

```bash
dotdeps add <ecosystem>:<package>
```

This clones the library into `.deps/<ecosystem>/<package>/` where you can browse the actual implementation.

**When to use:**
- Implementing features with a dependency's API
- Debugging behavior that involves external code
- Verifying your assumptions about how a library works

**Ecosystems:** python, node, rust, go, ruby, swift

After fetching, read the relevant source files. The implementation is the truth—don't rely on training data for API details.
"#;

/// Configuration for init command
pub struct InitConfig {
    pub skip_gitignore: bool,
    pub skip_instructions: bool,
    pub dry_run: bool,
}

/// Result of an individual init action
#[derive(Debug, Clone)]
pub enum ActionResult {
    /// Action was performed successfully
    Created(String),
    /// Action was skipped because it was already done
    AlreadyExists(String),
    /// Action was skipped due to configuration
    Skipped,
}

/// Result of the init command
pub struct InitResult {
    pub deps_dir: ActionResult,
    pub gitignore: ActionResult,
    pub instructions: ActionResult,
    /// Which file received instructions (if any)
    pub instructions_file: Option<String>,
}

impl InitResult {
    /// Returns true if everything was already initialized
    pub fn already_initialized(&self) -> bool {
        matches!(
            (&self.deps_dir, &self.gitignore, &self.instructions),
            (
                ActionResult::AlreadyExists(_),
                ActionResult::AlreadyExists(_) | ActionResult::Skipped,
                ActionResult::AlreadyExists(_) | ActionResult::Skipped
            )
        )
    }
}

/// Run the init command
pub fn run_init(config: InitConfig) -> Result<InitResult, io::Error> {
    let deps_dir = init_deps_dir(config.dry_run)?;
    let gitignore = if config.skip_gitignore {
        ActionResult::Skipped
    } else {
        init_gitignore(config.dry_run)?
    };
    let (instructions, instructions_file) = if config.skip_instructions {
        (ActionResult::Skipped, None)
    } else {
        init_instructions(config.dry_run)?
    };

    Ok(InitResult {
        deps_dir,
        gitignore,
        instructions,
        instructions_file,
    })
}

/// Create .deps/ directory if it doesn't exist
fn init_deps_dir(dry_run: bool) -> Result<ActionResult, io::Error> {
    let deps_path = Path::new(".deps");

    if deps_path.exists() {
        return Ok(ActionResult::AlreadyExists(
            ".deps/ already exists".to_string(),
        ));
    }

    if !dry_run {
        fs::create_dir(deps_path)?;
    }

    Ok(ActionResult::Created("Created .deps/".to_string()))
}

/// Add .deps/ to .gitignore if not already present
fn init_gitignore(dry_run: bool) -> Result<ActionResult, io::Error> {
    let gitignore_path = Path::new(".gitignore");

    // Read existing content or start fresh
    let existing_content = if gitignore_path.exists() {
        fs::read_to_string(gitignore_path)?
    } else {
        String::new()
    };

    // Check if .deps/ or .deps is already in gitignore
    if gitignore_has_deps(&existing_content) {
        return Ok(ActionResult::AlreadyExists(
            ".gitignore already includes .deps/".to_string(),
        ));
    }

    if !dry_run {
        // Append .deps/ to gitignore, handling trailing newline
        let new_content = if existing_content.is_empty() {
            ".deps/\n".to_string()
        } else if existing_content.ends_with('\n') {
            format!("{}.deps/\n", existing_content)
        } else {
            format!("{}\n.deps/\n", existing_content)
        };

        fs::write(gitignore_path, new_content)?;
    }

    Ok(ActionResult::Created(
        "Added \".deps/\" to .gitignore".to_string(),
    ))
}

/// Check if gitignore already has .deps/ or .deps pattern
fn gitignore_has_deps(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        // Match .deps, .deps/, or patterns like /.deps, /.deps/
        if trimmed == ".deps" || trimmed == ".deps/" || trimmed == "/.deps" || trimmed == "/.deps/"
        {
            return true;
        }
    }
    false
}

/// Add usage instructions to AGENTS.md or CLAUDE.md
fn init_instructions(dry_run: bool) -> Result<(ActionResult, Option<String>), io::Error> {
    let agents_md = Path::new("AGENTS.md");
    let claude_md = Path::new("CLAUDE.md");

    // Priority: AGENTS.md > CLAUDE.md > create AGENTS.md
    let (target_path, existing_content) = if agents_md.exists() {
        let content = fs::read_to_string(agents_md)?;
        (agents_md, Some(content))
    } else if claude_md.exists() {
        let content = fs::read_to_string(claude_md)?;
        (claude_md, Some(content))
    } else {
        (agents_md, None)
    };

    let target_name = target_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Check if instructions already present
    if let Some(ref content) = existing_content
        && content.contains(MARKER_COMMENT)
    {
        return Ok((
            ActionResult::AlreadyExists(format!(
                "{} already has dotdeps instructions",
                target_name
            )),
            Some(target_name),
        ));
    }

    let had_existing_content = existing_content.is_some();

    if !dry_run {
        let new_content = match existing_content {
            Some(content) => {
                // Append to existing file, handling trailing newline
                if content.is_empty() {
                    INSTRUCTIONS.to_string()
                } else if content.ends_with('\n') {
                    format!("{}\n{}", content, INSTRUCTIONS)
                } else {
                    format!("{}\n\n{}", content, INSTRUCTIONS)
                }
            }
            None => {
                // Create new file with instructions
                INSTRUCTIONS.to_string()
            }
        };

        fs::write(target_path, new_content)?;
    }

    let action_msg = if had_existing_content {
        format!("Added dotdeps instructions to {}", target_name)
    } else {
        format!("Created {} with dotdeps instructions", target_name)
    };

    Ok((ActionResult::Created(action_msg), Some(target_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitignore_has_deps_exact_match() {
        assert!(gitignore_has_deps(".deps/"));
        assert!(gitignore_has_deps(".deps"));
        assert!(gitignore_has_deps("/.deps/"));
        assert!(gitignore_has_deps("/.deps"));
    }

    #[test]
    fn test_gitignore_has_deps_with_other_content() {
        assert!(gitignore_has_deps("node_modules/\n.deps/\n"));
        assert!(gitignore_has_deps("# comment\n.deps\n"));
    }

    #[test]
    fn test_gitignore_has_deps_no_match() {
        assert!(!gitignore_has_deps(""));
        assert!(!gitignore_has_deps("node_modules/"));
        assert!(!gitignore_has_deps(".dependencies/"));
        assert!(!gitignore_has_deps("# .deps/"));
    }

    #[test]
    fn test_gitignore_has_deps_whitespace() {
        assert!(gitignore_has_deps("  .deps/  "));
        assert!(gitignore_has_deps("\t.deps\t"));
    }
}
