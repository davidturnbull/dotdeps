use crate::settings;
use std::io::{self, Write};

pub fn execute(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let subcommand = args.first().map(|s| s.as_str());

    match subcommand {
        None | Some("state") => {
            // Display current analytics state
            if settings::analytics_disabled() {
                println!("InfluxDB analytics are disabled.");
            } else {
                println!("InfluxDB analytics are enabled.");
            }
            println!("Google Analytics were destroyed.");
        }
        Some("on") => {
            // Enable analytics
            settings::write("analyticsdisabled", "false")?;
            settings::delete("analyticsuuid")?;
            settings::write("analyticsmessage", "true")?;
            settings::write("caskanalyticsmessage", "true")?;
            settings::write("influxanalyticsmessage", "true")?;
        }
        Some("off") => {
            // Disable analytics
            settings::write("analyticsdisabled", "true")?;
            settings::delete("analyticsuuid")?;
        }
        Some("regenerate-uuid") => {
            // Regenerate UUID (deprecated)
            settings::delete("analyticsuuid")?;
            writeln!(
                io::stderr(),
                "Warning: Homebrew no longer uses an analytics UUID so this has been deleted!"
            )?;
            println!("brew analytics regenerate-uuid is no longer necessary.");
        }
        Some(unknown) => {
            eprintln!("Usage: brew analytics [subcommand]");
            eprintln!();
            eprintln!(
                "Control Homebrew's anonymous aggregate user behaviour analytics. Read more at"
            );
            eprintln!("https://docs.brew.sh/Analytics.");
            eprintln!();
            eprintln!("brew analytics [state]:");
            eprintln!("    Display the current state of Homebrew's analytics.");
            eprintln!();
            eprintln!("brew analytics (on|off):");
            eprintln!("    Turn Homebrew's analytics on or off respectively.");
            eprintln!();
            return Err(format!("Invalid usage: unknown subcommand: {}", unknown).into());
        }
    }

    Ok(())
}
