use color_eyre::Result;
use color_eyre::eyre::eyre;
use std::process::Command;

use crate::tool;

pub fn run(name: String, force: bool) -> Result<()> {
    let pid_path = tool::CFG.pid(&name);
    if !pid_path.exists() {
        return Err(eyre!("Session '{}' does not exist.", name));
    }

    let pid_text = std::fs::read_to_string(&pid_path)
        .map_err(|e| eyre!("Failed to read pid file {:?}: {}", pid_path, e))?;
    let pid: i32 = pid_text
        .trim()
        .parse()
        .map_err(|e| eyre!("Invalid pid in {:?}: {}", pid_path, e))?;

    let signal_name = if force { "KILL" } else { "TERM" };

    let status = Command::new("kill")
        .arg(format!("-{}", signal_name))
        .arg(pid.to_string())
        .status()
        .map_err(|e| eyre!("Failed to invoke 'kill': {}", e))?;

    if !status.success() {
        return Err(eyre!(
            "Failed to send SIG{} to session '{}' (pid {}): kill exited with {}",
            signal_name,
            name,
            pid,
            status
        ));
    }

    println!("Sent SIG{} to session '{}' (pid {}).", signal_name, name, pid);
    Ok(())
}
