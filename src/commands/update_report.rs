pub fn run(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Error: update-report should not be called directly!");
    std::process::exit(1);
}
