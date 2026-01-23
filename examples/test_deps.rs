use homebrew_rust::deps::DependencyGraph;

fn main() {
    println!("Testing dependency resolution...\n");

    // Test 1: wget (simple dependencies)
    println!("=== Testing wget dependencies ===");
    let mut graph = DependencyGraph::new();
    match graph.build_for_formula("wget", false) {
        Ok(_) => {
            println!("✓ Built dependency graph for wget");

            // Get all dependencies
            let deps = graph.get_all_dependencies("wget");
            println!("All dependencies: {:?}", deps);

            // Get topological order
            match graph.topological_sort() {
                Ok(order) => {
                    println!("Install order:");
                    for (i, formula) in order.iter().enumerate() {
                        println!("  {}. {}", i + 1, formula);
                    }
                }
                Err(e) => println!("✗ Error in topological sort: {}", e),
            }
        }
        Err(e) => println!("✗ Error building graph: {}", e),
    }

    println!("\n=== Testing curl dependencies ===");
    let mut graph2 = DependencyGraph::new();
    match graph2.build_for_formula("curl", false) {
        Ok(_) => {
            println!("✓ Built dependency graph for curl");

            // Get all dependencies
            let deps = graph2.get_all_dependencies("curl");
            println!("All dependencies ({} total): {:?}", deps.len(), deps);

            // Get topological order
            match graph2.topological_sort() {
                Ok(order) => {
                    println!("Install order:");
                    for (i, formula) in order.iter().enumerate() {
                        println!("  {}. {}", i + 1, formula);
                    }
                }
                Err(e) => println!("✗ Error in topological sort: {}", e),
            }
        }
        Err(e) => println!("✗ Error building graph: {}", e),
    }

    println!("\n=== Testing with build dependencies ===");
    let mut graph3 = DependencyGraph::new();
    match graph3.build_for_formula("curl", true) {
        Ok(_) => {
            println!("✓ Built dependency graph for curl (with build deps)");

            // Get all dependencies
            let deps = graph3.get_all_dependencies("curl");
            println!("All dependencies ({} total): {:?}", deps.len(), deps);
        }
        Err(e) => println!("✗ Error building graph: {}", e),
    }
}
