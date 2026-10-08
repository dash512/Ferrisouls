use std::path::PathBuf;

use keyvalues_parser::{parse, Value};
use winreg::enums::*;
use winreg::RegKey;

pub fn steam_install_path() -> Option<PathBuf> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    for key_path in [
        r"SOFTWARE\Valve\Steam",
        r"SOFTWARE\WOW6432Node\Valve\Steam",
    ] {
        if let Ok(key) = hklm.open_subkey(key_path) {
            if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}


pub fn find_steam_game(app_id: u32) -> Option<PathBuf> {
    let steam_path = steam_install_path()?;

    let library_file = steam_path
        .join("steamapps")
        .join("libraryfolders.vdf");

    let text = std::fs::read_to_string(library_file).ok()?;
    let vdf = keyvalues_parser::parse(&text).ok()?;

    let root = vdf.value.unwrap_obj();

    let libraries = root
        .get("libraryfolders")?
        .first()?
        .get_obj()?;

    for values in libraries.values() {
        for library_value in values {
            let Value::Obj(library) = library_value else {
                continue;
            };

            let Some(path_value) = library
                .get("path")
                .and_then(|values| values.first())
            else {
                continue;
            };

            let Some(path_str) = path_value.get_str() else {
                continue;
            };

            let library_path = PathBuf::from(path_str);

            let manifest = library_path
                .join("steamapps")
                .join(format!("appmanifest_{app_id}.acf"));

            if !manifest.is_file() {
                continue;
            }

            let manifest_text = std::fs::read_to_string(&manifest).ok()?;
            let manifest_vdf = keyvalues_parser::parse(&manifest_text).ok()?;

            let manifest_root = manifest_vdf.value.unwrap_obj();

            let app_state = manifest_root
                .get("AppState")?
                .first()?
                .get_obj()?;

            let install_dir = app_state
                .get("installdir")?
                .first()?
                .get_str()?;

            return Some(
                library_path
                    .join("steamapps")
                    .join("common")
                    .join(install_dir),
            );
        }
    }

    None
}


pub fn find_install_dir(manifest: &std::path::Path, library_path: PathBuf) -> Option<PathBuf> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let vdf = parse(&text).ok()?;

    let root = vdf.value.unwrap_obj();

    let app_state = root
        .get("AppState")?
        .first()?
        .get_obj()?;

    let install_dir = app_state
        .get("installdir")?
        .first()?
        .get_str()?;

    Some(
        library_path
            .join("steamapps")
            .join("common")
            .join(install_dir),
    )
}

