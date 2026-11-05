//! Persistencia portable: un JSON junto al ejecutable (o, si esa carpeta no es
//! escribible, en `%LOCALAPPDATA%\d20calc`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "d20calc.json";

#[derive(Clone, Serialize, Deserialize)]
pub struct Saved {
    pub name: String,
    pub formula: String,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Stored {
    pub theme: usize,
    pub sfx: bool,
    pub saved: Vec<Saved>,
}

impl Default for Stored {
    fn default() -> Self {
        Stored { theme: 0, sfx: true, saved: Vec::new() }
    }
}

fn candidates() -> Vec<PathBuf> {
    // `D20CALC_DATA` fuerza otra ruta (lo usan las pruebas para no tocar los datos reales).
    if let Some(custom) = std::env::var_os("D20CALC_DATA") {
        return vec![PathBuf::from(custom)];
    }
    let mut out = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        out.push(dir.join(FILE_NAME));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(PathBuf::from(local).join("d20calc").join(FILE_NAME));
    }
    out
}

pub fn load() -> Stored {
    for path in candidates() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(stored) = serde_json::from_str(&text) {
                return stored;
            }
        }
    }
    Stored::default()
}

pub fn save(stored: &Stored) {
    let Ok(text) = serde_json::to_string_pretty(stored) else { return };
    for path in candidates() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&path, &text).is_ok() {
            return;
        }
    }
}
