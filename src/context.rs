//! Context output for LLM tooling
//!
//! Collects direct dependencies across ecosystems and formats a prompt-friendly message.

use crate::cli::Ecosystem;
use crate::{go, node, python, ruby, rust, swift};

pub fn render_context() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let mut found_lockfile = false;
    let mut entries: Vec<(Ecosystem, Vec<String>)> = Vec::new();

    if let Some(deps) = collect_python_deps()? {
        found_lockfile = true;
        entries.push((Ecosystem::Python, deps));
    }
    if let Some(deps) = collect_node_deps()? {
        found_lockfile = true;
        entries.push((Ecosystem::Node, deps));
    }
    if let Some(deps) = collect_go_deps()? {
        found_lockfile = true;
        entries.push((Ecosystem::Go, deps));
    }
    if let Some(deps) = collect_rust_deps()? {
        found_lockfile = true;
        entries.push((Ecosystem::Rust, deps));
    }
    if let Some(deps) = collect_ruby_deps()? {
        found_lockfile = true;
        entries.push((Ecosystem::Ruby, deps));
    }
    if let Some(deps) = collect_swift_deps()? {
        found_lockfile = true;
        entries.push((Ecosystem::Swift, deps));
    }

    if !found_lockfile {
        return Ok(None);
    }

    Ok(Some(format_context(&entries)))
}

fn format_context(entries: &[(Ecosystem, Vec<String>)]) -> String {
    let mut output = String::new();
    output.push_str("## Dependency Source Code\n\n");
    output.push_str("Libraries in this project may have changed since your training. Before writing code that uses these dependencies, fetch their source to verify API details.\n\n");
    output.push_str("```bash\n");
    output.push_str("dotdeps add <ecosystem>:<package>\n");
    output.push_str("```\n\n");
    output.push_str("Source is cloned to `.deps/<ecosystem>/<package>/` for browsing.\n\n");
    output.push_str("**Available in this project:**\n\n");
    output.push_str("```bash\n");

    for (ecosystem, deps) in entries {
        for dep in deps {
            output.push_str(&format!("dotdeps add {}:{}\n", ecosystem, dep));
        }
    }

    output.push_str("```\n\n");
    output.push_str("After fetching, use a sub-agent to explore the source and answer specific questions about the implementation.\n");
    output
}

fn collect_python_deps() -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
    match python::find_lockfile_path() {
        Ok(path) => Ok(Some(python::list_direct_dependencies(&path)?)),
        Err(python::LockfileError::NotFound) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
}

fn collect_node_deps() -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
    match node::find_lockfile_path() {
        Ok(path) => Ok(Some(node::list_direct_dependencies(&path)?)),
        Err(node::LockfileError::NotFound) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
}

fn collect_go_deps() -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
    match go::find_lockfile_path() {
        Ok(path) => Ok(Some(go::list_direct_dependencies(&path)?)),
        Err(go::LockfileError::NotFound) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
}

fn collect_rust_deps() -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
    match rust::find_lockfile_path() {
        Ok(path) => Ok(Some(rust::list_direct_dependencies(&path)?)),
        Err(rust::LockfileError::NotFound) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
}

fn collect_ruby_deps() -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
    match ruby::find_lockfile_path() {
        Ok(path) => Ok(Some(ruby::list_direct_dependencies(&path)?)),
        Err(ruby::LockfileError::NotFound) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
}

fn collect_swift_deps() -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
    match swift::find_lockfile_path() {
        Ok(path) => Ok(Some(swift::list_direct_dependencies(&path)?)),
        Err(swift::LockfileError::NotFound) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_format_context_output_exact() {
        let entries = vec![(crate::cli::Ecosystem::Python, vec!["requests".to_string()])];
        let output = super::format_context(&entries);
        let expected = concat!(
            "## Dependency Source Code\n\n",
            "Libraries in this project may have changed since your training. Before writing code that uses these dependencies, fetch their source to verify API details.\n\n",
            "```bash\n",
            "dotdeps add <ecosystem>:<package>\n",
            "```\n\n",
            "Source is cloned to `.deps/<ecosystem>/<package>/` for browsing.\n\n",
            "**Available in this project:**\n\n",
            "```bash\n",
            "dotdeps add python:requests\n",
            "```\n\n",
            "After fetching, use a sub-agent to explore the source and answer specific questions about the implementation.\n",
        );
        assert_eq!(output, expected);
    }
}
