//! Dependency resolution module.
//!
//! This module handles:
//! - Building dependency graphs from formulae
//! - Detecting circular dependencies
//! - Topological sorting for correct installation order
//! - Handling different dependency types (runtime, build, optional)

use std::collections::{HashMap, HashSet, VecDeque};

use crate::api;

/// Dependency graph for formula installation order resolution.
#[derive(Debug)]
pub struct DependencyGraph {
    /// Map of formula name to its dependencies
    graph: HashMap<String, Vec<String>>,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    /// Create a new empty dependency graph.
    pub fn new() -> Self {
        Self {
            graph: HashMap::new(),
        }
    }

    /// Build the dependency graph for a formula and its dependencies.
    ///
    /// This recursively processes all dependencies and adds them to the graph.
    /// By default, only runtime dependencies are included (not build or test dependencies).
    pub fn build_for_formula(
        &mut self,
        formula_name: &str,
        include_build: bool,
    ) -> Result<(), String> {
        let mut visited = HashSet::new();
        self.build_recursive(formula_name, include_build, &mut visited)
    }

    /// Recursively build the dependency graph.
    fn build_recursive(
        &mut self,
        formula_name: &str,
        include_build: bool,
        visited: &mut HashSet<String>,
    ) -> Result<(), String> {
        // Skip if already visited
        if visited.contains(formula_name) {
            return Ok(());
        }
        visited.insert(formula_name.to_string());

        // Load formula info
        let info = api::get_formula(formula_name)
            .map_err(|e| format!("Formula not found: {}: {}", formula_name, e))?;

        // Get dependencies (runtime + optionally build)
        let mut deps = info.dependencies.clone();
        if include_build {
            deps.extend(info.build_dependencies.clone());
        }

        // Add to graph
        self.graph.insert(formula_name.to_string(), deps.clone());

        // Recursively process dependencies
        for dep in &deps {
            self.build_recursive(dep, include_build, visited)?;
        }

        Ok(())
    }

    /// Detect if there are any circular dependencies in the graph.
    ///
    /// Returns Some(cycle) if a cycle is detected, None otherwise.
    pub fn detect_cycle(&self) -> Option<Vec<String>> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for node in self.graph.keys() {
            if !visited.contains(node)
                && let Some(cycle) =
                    self.detect_cycle_util(node, &mut visited, &mut rec_stack, &mut path)
            {
                return Some(cycle);
            }
        }

        None
    }

    /// Utility function for cycle detection using DFS.
    fn detect_cycle_util(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = self.graph.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    if let Some(cycle) = self.detect_cycle_util(neighbor, visited, rec_stack, path)
                    {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(neighbor) {
                    // Found cycle - extract it from path
                    let cycle_start = path.iter().position(|n| n == neighbor).unwrap();
                    let mut cycle = path[cycle_start..].to_vec();
                    cycle.push(neighbor.to_string());
                    return Some(cycle);
                }
            }
        }

        rec_stack.remove(node);
        path.pop();
        None
    }

    /// Perform topological sort on the dependency graph.
    ///
    /// Returns a vector of formula names in the order they should be installed.
    /// Dependencies come before the formulae that depend on them.
    ///
    /// Returns an error if a cycle is detected.
    pub fn topological_sort(&self) -> Result<Vec<String>, String> {
        // Check for cycles first
        if let Some(cycle) = self.detect_cycle() {
            return Err(format!(
                "Circular dependency detected: {}",
                cycle.join(" -> ")
            ));
        }

        // Calculate in-degree for each node
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node in self.graph.keys() {
            in_degree.entry(node.clone()).or_insert(0);
        }
        for deps in self.graph.values() {
            for dep in deps {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        // Queue of nodes with in-degree 0
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(node, _)| node.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            // Reduce in-degree for neighbors
            if let Some(neighbors) = self.graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        // If result doesn't contain all nodes, there's a cycle (shouldn't happen due to earlier check)
        if result.len() != self.graph.len() {
            return Err("Cycle detected in dependency graph".to_string());
        }

        // Reverse to get dependencies first
        result.reverse();

        Ok(result)
    }

    /// Get all dependencies for a formula (flattened list).
    pub fn get_all_dependencies(&self, formula_name: &str) -> Vec<String> {
        let mut deps = HashSet::new();
        let mut to_visit = VecDeque::new();
        to_visit.push_back(formula_name.to_string());

        while let Some(node) = to_visit.pop_front() {
            if let Some(neighbors) = self.graph.get(&node) {
                for neighbor in neighbors {
                    if deps.insert(neighbor.clone()) {
                        to_visit.push_back(neighbor.clone());
                    }
                }
            }
        }

        let mut result: Vec<String> = deps.into_iter().collect();
        result.sort();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = DependencyGraph::new();
        let result = graph.topological_sort().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_simple_chain() {
        let mut graph = DependencyGraph::new();
        graph.graph.insert("a".to_string(), vec!["b".to_string()]);
        graph.graph.insert("b".to_string(), vec!["c".to_string()]);
        graph.graph.insert("c".to_string(), vec![]);

        let result = graph.topological_sort().unwrap();
        // c should come first (no deps), then b, then a
        assert_eq!(result, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_diamond_dependency() {
        let mut graph = DependencyGraph::new();
        graph
            .graph
            .insert("a".to_string(), vec!["b".to_string(), "c".to_string()]);
        graph.graph.insert("b".to_string(), vec!["d".to_string()]);
        graph.graph.insert("c".to_string(), vec!["d".to_string()]);
        graph.graph.insert("d".to_string(), vec![]);

        let result = graph.topological_sort().unwrap();
        // d should come first
        assert_eq!(result[0], "d");
        // a should come last
        assert_eq!(result[result.len() - 1], "a");
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DependencyGraph::new();
        graph.graph.insert("a".to_string(), vec!["b".to_string()]);
        graph.graph.insert("b".to_string(), vec!["c".to_string()]);
        graph.graph.insert("c".to_string(), vec!["a".to_string()]);

        let cycle = graph.detect_cycle();
        assert!(cycle.is_some());
        let cycle = cycle.unwrap();
        // Cycle should contain all three nodes
        assert!(cycle.contains(&"a".to_string()));
        assert!(cycle.contains(&"b".to_string()));
        assert!(cycle.contains(&"c".to_string()));
    }

    #[test]
    fn test_get_all_dependencies() {
        let mut graph = DependencyGraph::new();
        graph
            .graph
            .insert("a".to_string(), vec!["b".to_string(), "c".to_string()]);
        graph.graph.insert("b".to_string(), vec!["d".to_string()]);
        graph.graph.insert("c".to_string(), vec!["d".to_string()]);
        graph.graph.insert("d".to_string(), vec![]);

        let deps = graph.get_all_dependencies("a");
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&"b".to_string()));
        assert!(deps.contains(&"c".to_string()));
        assert!(deps.contains(&"d".to_string()));
    }
}
