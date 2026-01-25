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
    output.push_str("[dotdeps] Your training data may be outdated or incomplete for the libraries in this project. When you're unsure how a library works, fetch and read the actual source code instead of guessing.\n\n");
    output.push_str("Fetch a dependency:\n");
    output.push_str("  dotdeps add <ecosystem>:<package>\n\n");
    output.push_str(
        "This clones the repository into .deps/<ecosystem>/<package>/ for direct browsing.\n\n",
    );
    output.push_str("Available in this project:\n");

    for (ecosystem, deps) in entries {
        for dep in deps {
            output.push_str(&format!("  dotdeps add {}:{}\n", ecosystem, dep));
        }
    }

    output.push_str("\nAfter fetching, spawn a sub-agent to explore the implementation:\n");
    output.push_str(
        "  Task: Read .deps/python/requests/src/ to understand how Session handles retries\n\n",
    );
    output.push_str("The source code is the truth. Use it.\n");
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
            "[dotdeps] Your training data may be outdated or incomplete for the libraries in this project. When you're unsure how a library works, fetch and read the actual source code instead of guessing.\n\n",
            "Fetch a dependency:\n",
            "  dotdeps add <ecosystem>:<package>\n\n",
            "This clones the repository into .deps/<ecosystem>/<package>/ for direct browsing.\n\n",
            "Available in this project:\n",
            "  dotdeps add python:requests\n",
            "\n",
            "After fetching, spawn a sub-agent to explore the implementation:\n",
            "  Task: Read .deps/python/requests/src/ to understand how Session handles retries\n\n",
            "The source code is the truth. Use it.\n",
        );
        assert_eq!(output, expected);
    }
}
