use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub view: usize,
    pub terminal_theme: bool,
}
pub fn state_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
        .map(|p| p.join("kernwatch"))
}
impl Settings {
    pub fn load() -> Option<Self> {
        let path = state_dir()?.join("settings.json");
        if std::fs::metadata(&path).ok()?.len() > 4096 {
            return None;
        }
        let s: Self = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        (s.view < 13).then_some(s)
    }
    pub fn save(&self) -> std::io::Result<()> {
        let Some(dir) = state_dir() else {
            return Ok(());
        };
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("settings.json");
        let temporary = dir.join(format!("settings-{}.tmp", crate::recording::stamp()));
        std::fs::write(&temporary, serde_json::to_vec(self)?)?;
        std::fs::rename(temporary, path)
    }
}
