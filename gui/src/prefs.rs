// Persisted GUI preferences (~/.config/crucible/gui.json).
//
// Qt selects the RHI graphics backend once at startup, so the OpenGL/Vulkan
// toggle works by persisting the choice here and applying it as
// QSG_RHI_BACKEND on the next launch (see main.rs + Controller::restart).

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Prefs {
    pub dark: bool,
    /// "auto" | "opengl" | "vulkan" — applied as QSG_RHI_BACKEND at launch.
    pub render_backend: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: true,
            render_backend: "auto".to_string(),
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("crucible").join("gui.json"))
}

/// Load preferences; any missing/invalid field falls back to its default.
pub fn load() -> Prefs {
    let mut p = Prefs::default();
    let Some(file) = path() else { return p };
    let Ok(text) = std::fs::read_to_string(file) else { return p };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return p };
    if let Some(d) = v.get("dark").and_then(|x| x.as_bool()) {
        p.dark = d;
    }
    if let Some(b) = v.get("render_backend").and_then(|x| x.as_str()) {
        if matches!(b, "auto" | "opengl" | "vulkan") {
            p.render_backend = b.to_string();
        }
    }
    p
}

/// Write preferences via temp-file + rename so a crash can't truncate them.
pub fn save(p: &Prefs) -> Result<(), String> {
    let Some(file) = path() else {
        return Err("no config directory (HOME unset)".to_string());
    };
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let body = serde_json::json!({
        "dark": p.dark,
        "render_backend": p.render_backend,
    })
    .to_string();
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &file).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_validation() {
        let dir = std::env::temp_dir().join(format!("crux-prefs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        // defaults when no file exists
        let p = load();
        assert!(p.dark);
        assert_eq!(p.render_backend, "auto");

        // save → load roundtrip
        save(&Prefs { dark: false, render_backend: "vulkan".into() }).unwrap();
        let p = load();
        assert!(!p.dark);
        assert_eq!(p.render_backend, "vulkan");

        // invalid backend in the file falls back to default
        let file = dir.join("crucible").join("gui.json");
        std::fs::write(&file, r#"{"dark":false,"render_backend":"directx"}"#).unwrap();
        assert_eq!(load().render_backend, "auto");

        // corrupt file falls back to all defaults
        std::fs::write(&file, "not json").unwrap();
        let p = load();
        assert!(p.dark);
        assert_eq!(p.render_backend, "auto");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
