use crate::commands::{Command, CommandResult};
use crate::tap;

pub struct Taps;

impl Command for Taps {
    fn run(&self, _args: &[String]) -> CommandResult {
        let taps = tap::list_installed();

        for tap in taps {
            println!("{}", tap.name());
        }

        Ok(())
    }
}
