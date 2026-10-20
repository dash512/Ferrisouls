use std::path::{Path, PathBuf};

use keyvalues_parser::{parse, Value};

#[cfg(windows)]
pub fn steam_install_path() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"]
        .iter()
        .find_map(|key_path| {
            hklm.open_subkey(key_path)
                .ok()?
                .get_value::<String, _>("InstallPath")
                .ok()
                .map(PathBuf::from)
        })
}

pub fn steam_library_paths() -> Vec<PathBuf> {
    let Some(steam_path) = steam_install_path() else {
        return Vec::new();
    };

    let Ok(text) = std::fs::read_to_string(steam_path.join("steamapps").join("libraryfolders.vdf"))
    else {
        return vec![steam_path]; // at least check the main install
    };
    let Ok(vdf) = parse(&text) else {
        return vec![steam_path];
    };

    let mut paths = vec![steam_path];

    if let Some(root) = vdf.value.get_obj() {
        for values in root.values() {
            for value in values {
                if let Value::Obj(lib) = value {
                    if let Some(p) = lib
                        .get("path")
                        .and_then(|v| v.first())
                        .and_then(|v| v.get_str())
                    {
                        let p = PathBuf::from(p);
                        if !paths.contains(&p) {
                            paths.push(p);
                        }
                    }
                }
            }
        }
    }

    paths
}

pub fn find_install_dir(manifest: &Path, library_path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let vdf = parse(&text).ok()?;

    let install_dir = vdf
        .value
        .get_obj()?
        .get("installdir")?
        .first()?
        .get_str()?;

    Some(library_path.join("steamapps").join("common").join(install_dir))
}

pub fn find_steam_game(app_id: u32) -> Option<PathBuf> {
    steam_library_paths().into_iter().find_map(|library| {
        let manifest = library
            .join("steamapps")
            .join(format!("appmanifest_{app_id}.acf"));

        if !manifest.is_file() {
            return None;
        }
        find_install_dir(&manifest, &library)
    })
}

