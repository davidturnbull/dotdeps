#[path = "e2e/harness.rs"]
mod harness;
#[path = "e2e/scenarios.rs"]
mod scenarios;

#[test]
fn e2e_scenarios() {
    let ctx = harness::TestContext::new().expect("Failed to initialize test context");
    for scenario in scenarios::scenarios() {
        eprintln!("==> running scenario: {}", scenario.name);
        if let Err(err) = (scenario.run)(&ctx) {
            panic!("scenario '{}' failed: {}", scenario.name, err);
        }
    }
}
