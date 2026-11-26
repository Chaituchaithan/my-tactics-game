//! Save file location and (de)serialization.

use crate::logic::campaign::Campaign;

fn path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "android")]
    {
        // App-private internal storage; no permissions needed.
        Some(std::path::PathBuf::from("/data/data/com.emberfall.tactics/files/save.json"))
    }
    #[cfg(not(target_os = "android"))]
    {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        Some(std::path::PathBuf::from(home).join(".emberfall").join("save.json"))
    }
}

pub fn save(c: &Campaign) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(c) {
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}

pub fn load() -> Option<Campaign> {
    let data = std::fs::read_to_string(path()?).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn delete() {
    if let Some(p) = path() {
        let _ = std::fs::remove_file(p);
    }
}

fn profile_path() -> Option<std::path::PathBuf> {
    path().map(|p| p.with_file_name("profile.json"))
}

pub fn save_profile(p: &crate::logic::profile::Profile) {
    let Some(path) = profile_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(p) {
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn load_profile() -> crate::logic::profile::Profile {
    profile_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
