//! Plugin ("addons") system for the PatchedModrinth fork.
//!
//! Plugins live in `<settings_dir>/plugins/<id>/` as a folder containing a
//! `manifest.json` plus the referenced `.js`/`.css` files. Enabled state is
//! persisted separately in `<settings_dir>/plugins/enabled.json` so upgrading
//! the built-in plugins never clobbers the user's on/off choices.
//!
//! The frontend loader (`apps/app-frontend/src/plugins/plugin-loader.js`) calls
//! [`read_plugins`] once on startup and injects each enabled plugin's CSS/JS.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::plugin::TauriPlugin;
use tokio::sync::OnceCell;

pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("addons")
        .invoke_handler(tauri::generate_handler![
            read_plugins,
            set_plugin_enabled,
            get_plugins_dir,
        ])
        .build()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    js: Option<String>,
    #[serde(default)]
    css: Option<String>,
    #[serde(default)]
    enabled_by_default: bool,
    #[serde(default)]
    builtin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginData {
    id: String,
    name: String,
    description: String,
    version: String,
    author: String,
    enabled: bool,
    builtin: bool,
    js: Option<String>,
    css: Option<String>,
}

async fn plugins_dir() -> crate::api::Result<PathBuf> {
    let state = theseus::State::get().await?;
    let dir = state.directories.settings_dir.join("plugins");
    tokio::fs::create_dir_all(&dir).await?;
    Ok(dir)
}

async fn read_enabled_map() -> crate::api::Result<HashMap<String, bool>> {
    let path = plugins_dir().await?.join("enabled.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(contents) => Ok(serde_json::from_str(&contents).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(e) => Err(e.into()),
    }
}

async fn write_enabled_map(map: &HashMap<String, bool>) -> crate::api::Result<()> {
    let path = plugins_dir().await?.join("enabled.json");
    let json = serde_json::to_string_pretty(map)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    tokio::fs::write(&path, json).await?;
    Ok(())
}

#[tauri::command]
pub async fn read_plugins() -> crate::api::Result<Vec<PluginData>> {
    ensure_seeded().await;

    let dir = plugins_dir().await?;
    let enabled_map = read_enabled_map().await?;

    let mut out = Vec::new();
    let mut entries = tokio::fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let path = entry.path();
        let manifest_str = match tokio::fs::read_to_string(path.join("manifest.json")).await {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let manifest: Manifest = match serde_json::from_str(&manifest_str) {
            Ok(manifest) => manifest,
            Err(e) => {
                tracing::warn!("Skipping invalid plugin manifest in {path:?}: {e}");
                continue;
            }
        };

        let js = match &manifest.js {
            Some(file) if !file.is_empty() => {
                tokio::fs::read_to_string(path.join(file)).await.ok()
            }
            _ => None,
        };
        let css = match &manifest.css {
            Some(file) if !file.is_empty() => {
                tokio::fs::read_to_string(path.join(file)).await.ok()
            }
            _ => None,
        };

        let enabled = enabled_map
            .get(&manifest.id)
            .copied()
            .unwrap_or(manifest.enabled_by_default);

        out.push(PluginData {
            name: if manifest.name.is_empty() {
                manifest.id.clone()
            } else {
                manifest.name.clone()
            },
            id: manifest.id,
            description: manifest.description,
            version: manifest.version,
            author: manifest.author,
            enabled,
            builtin: manifest.builtin,
            js,
            css,
        });
    }

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub async fn set_plugin_enabled(id: String, enabled: bool) -> crate::api::Result<()> {
    let mut map = read_enabled_map().await?;
    map.insert(id, enabled);
    write_enabled_map(&map).await
}

#[tauri::command]
pub async fn get_plugins_dir() -> crate::api::Result<String> {
    Ok(plugins_dir().await?.to_string_lossy().to_string())
}

/// Whether a plugin is currently enabled. Used natively (e.g. by the ads
/// module) to honor plugin toggles without a frontend round-trip. Only the
/// ads module calls this, and that module is not built on Linux.
#[cfg(not(target_os = "linux"))]
pub async fn is_plugin_enabled(id: &str) -> bool {
    ensure_seeded().await;

    if let Ok(map) = read_enabled_map().await {
        if let Some(&value) = map.get(id) {
            return value;
        }
    }

    let Ok(dir) = plugins_dir().await else {
        return false;
    };
    match tokio::fs::read_to_string(dir.join(id).join("manifest.json")).await {
        Ok(contents) => serde_json::from_str::<Manifest>(&contents)
            .map(|manifest| manifest.enabled_by_default)
            .unwrap_or(false),
        Err(_) => false,
    }
}

struct BuiltinFile {
    name: &'static str,
    content: &'static str,
    /// Never overwrite once the user has one (e.g. their custom CSS).
    preserve: bool,
}

struct Builtin {
    id: &'static str,
    files: &'static [BuiltinFile],
}

const BUILTINS: &[Builtin] = &[
    Builtin {
        id: "hide-ads",
        files: &[
            BuiltinFile {
                name: "manifest.json",
                content: include_str!("builtin_plugins/hide-ads/manifest.json"),
                preserve: false,
            },
            BuiltinFile {
                name: "styles.css",
                content: include_str!("builtin_plugins/hide-ads/styles.css"),
                preserve: false,
            },
        ],
    },
    Builtin {
        id: "multi-launch",
        files: &[
            BuiltinFile {
                name: "manifest.json",
                content: include_str!("builtin_plugins/multi-launch/manifest.json"),
                preserve: false,
            },
            BuiltinFile {
                name: "index.js",
                content: include_str!("builtin_plugins/multi-launch/index.js"),
                preserve: false,
            },
        ],
    },
    Builtin {
        id: "custom-css",
        files: &[
            BuiltinFile {
                name: "manifest.json",
                content: include_str!("builtin_plugins/custom-css/manifest.json"),
                preserve: false,
            },
            BuiltinFile {
                name: "user.css",
                content: include_str!("builtin_plugins/custom-css/user.css"),
                preserve: true,
            },
        ],
    },
];

static SEEDED: OnceCell<()> = OnceCell::const_new();

async fn ensure_seeded() {
    // get_or_try_init makes every concurrent caller await the same seeding
    // future, and only marks the cell initialized on success (so a failed seed
    // is retried on the next call). This avoids reading a half-written dir.
    let result = SEEDED
        .get_or_try_init(|| async { seed_builtin_plugins().await })
        .await;
    if let Err(e) = result {
        tracing::warn!("Failed to seed built-in plugins: {e}");
    }
}

async fn seed_builtin_plugins() -> crate::api::Result<()> {
    let dir = plugins_dir().await?;
    for builtin in BUILTINS {
        let plugin_dir = dir.join(builtin.id);
        tokio::fs::create_dir_all(&plugin_dir).await?;
        for file in builtin.files {
            let file_path = plugin_dir.join(file.name);
            if file.preserve && tokio::fs::try_exists(&file_path).await.unwrap_or(false) {
                continue;
            }
            tokio::fs::write(&file_path, file.content).await?;
        }
    }
    Ok(())
}
