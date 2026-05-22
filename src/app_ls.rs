use color_eyre::Result;
use color_eyre::eyre::eyre;
use std::{fs::TryLockError, path::PathBuf};
fn is_file_locked(path: &PathBuf) -> Result<bool> {
    let file = std::fs::OpenOptions::new().read(true).open(path)?;
    match file.try_lock() {
        Ok(_) => {
            file.unlock()?;
            Ok(false)
        }
        Err(e) => match e {
            TryLockError::WouldBlock => Ok(true),
            _ => Err(eyre!("Failed to check file lock: {}", e)),
        },
    }
}

fn get_all_pid_files(base_path: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut pid_files = vec![];
    for entry in std::fs::read_dir(base_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("pid") {
            pid_files.push(path);
        }
    }
    Ok(pid_files)
}

pub fn get_all_sessions(base_path: &PathBuf) -> Result<Vec<String>> {
    let sessions = get_all_pid_files(base_path)?
        .into_iter()
        .filter_map(|path| match is_file_locked(&path) {
            Ok(true) => Some(Ok(path.file_stem()?.to_str()?.to_string())),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<String>>>()?;
    Ok(sessions)
}
