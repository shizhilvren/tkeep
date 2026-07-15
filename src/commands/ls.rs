use color_eyre::Result;
use std::time::Duration;

use crate::{app_ls, tool};

pub fn run() -> Result<()> {
    let mut sessions = app_ls::get_all_sessions(&tool::CFG.dir())?;
    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    if sessions.is_empty() {
        println!("No active sessions found.");
    } else {
        println!("Active sessions:");
        for session in sessions {
            println!(
                "- {} (uptime: {})",
                session.name,
                format_duration(session.uptime)
            );
        }
    }
    Ok(())
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let minutes = (total % 3_600) / 60;
    let seconds = total % 60;
    if days > 0 {
        format!("{}d {:02}h {:02}m {:02}s", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{:02}h {:02}m {:02}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{:02}m {:02}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}
