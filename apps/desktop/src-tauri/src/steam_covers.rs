//! Read Steam's library artwork locally, without fetching URLs or changing profiles.
use std::{collections::HashSet, path::{Path, PathBuf}};
use base64::Engine;
use gtk::gdk_pixbuf::Pixbuf;

#[derive(serde::Serialize)]
pub struct SteamCover { pub app_id: String, pub image: String }

fn game_id(id: &str) -> Option<&str> {
    let game = id.strip_prefix("steam:")?;
    if game.is_empty() || game.len() > 10 || game.starts_with('0') || !game.bytes().all(|b| b.is_ascii_digit()) { return None; }
    game.parse::<u32>().ok().filter(|n| *n > 0).map(|_| game)
}
fn candidates(root: &Path, game: &str) -> Vec<PathBuf> {
    let cache = root.join("appcache/librarycache");
    let mut paths = vec![];
    for extension in ["jpg", "png"] {
        paths.push(cache.join(format!("{game}_library_600x900.{extension}")));
        paths.push(cache.join(format!("{game}_library_capsule.{extension}")));
    }
    let directory = cache.join(game);
    let mut directories = vec![directory.clone()];
    if let Ok(entries) = std::fs::read_dir(directory) {
        let mut children: Vec<_> = entries.flatten().take(128).map(|e| e.path()).filter(|p| p.is_dir()).collect();
        children.sort();
        directories.extend(children);
    }
    for directory in directories {
        for name in ["library_600x900.jpg", "library_capsule.jpg", "library_600x900.png", "library_capsule.png"] {
            paths.push(directory.join(name));
        }
    }
    paths
}
fn read_cover(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > 5 * 1024 * 1024 { return None; }
    let (_, width, height) = Pixbuf::file_info(path)?;
    // Do not stretch logos or banners into a portrait, or decode enormous images.
    if width <= 0 || height <= 0 || width > 8192 || height > 8192 || !(0.60..=0.75).contains(&(width as f64 / height as f64)) { return None; }
    let pixbuf = Pixbuf::from_file_at_scale(path, 600, 900, true).ok()?;
    let bytes = pixbuf.save_to_bufferv("jpeg", &[("quality", "90")]).ok()?;
    if bytes.len() > 1024 * 1024 { return None; }
    Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}
fn find_at(ids: Vec<String>, roots: &[PathBuf]) -> Result<Vec<SteamCover>, String> {
    if ids.len() > 256 || ids.iter().any(|id| game_id(id).is_none()) { return Err("IDs de jogos Steam inválidos.".into()); }
    let mut seen = HashSet::new();
    Ok(ids.into_iter().filter_map(|app_id| {
        if !seen.insert(app_id.clone()) { return None; }
        let game = game_id(&app_id)?;
        let image = roots.iter().flat_map(|root| candidates(root, game)).find_map(|path| read_cover(&path))?;
        Some(SteamCover { app_id, image })
    }).collect())
}
pub fn find(ids: Vec<String>) -> Result<Vec<SteamCover>, String> {
    find_at(ids, &super::application_icons::steam_roots())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_ids_and_oversized_batches() {
        for id in ["steam:0", "steam:01", "steam:4294967296", "steam:../570", "steam:570\n", "gamescope", "steam:"] {
            assert!(find_at(vec![id.into()], &[]).is_err(), "{id}");
        }
        assert_eq!(game_id("steam:4294967295"), Some("4294967295"));
        assert!(find_at(vec!["steam:570".into(); 257], &[]).is_err());
    }
    #[test]
    fn reads_portrait_art_from_flat_and_hashed_caches_with_fallback_for_missing_or_corrupt_files() {
        let root = std::env::temp_dir().join(format!("bindforge-covers-{}", std::process::id()));
        let cache = root.join("appcache/librarycache");
        let hashed = cache.join("570/hash");
        std::fs::create_dir_all(&hashed).unwrap();
        let portrait = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 600, 900).unwrap();
        portrait.fill(0x6849aaff);
        portrait.savev(hashed.join("library_capsule.jpg"), "jpeg", &[]).unwrap();
        portrait.savev(cache.join("730_library_600x900.png"), "png", &[]).unwrap();
        std::fs::write(cache.join("570_library_600x900.jpg"), b"corrupt").unwrap();
        let landscape = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 900, 300).unwrap();
        landscape.fill(0x123456ff);
        landscape.savev(cache.join("440_library_600x900.jpg"), "jpeg", &[]).unwrap();
        let covers = find_at(vec!["steam:570", "steam:730", "steam:570", "steam:57", "steam:440"].into_iter().map(str::to_string).collect(), &[root.clone()]).unwrap();
        assert_eq!(covers.len(), 2);
        assert_eq!(covers[0].app_id, "steam:570");
        assert_eq!(covers[1].app_id, "steam:730");
        assert!(covers.iter().all(|cover| cover.image.starts_with("data:image/jpeg;base64,")));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "reads installed Steam artwork selected with BINDFORGE_COVER_TEST_IDS"]
    fn installed_steam_covers() {
        let ids: Vec<String> = std::env::var("BINDFORGE_COVER_TEST_IDS").expect("provide comma-separated Steam IDs").split(',').map(str::to_string).collect();
        let covers = find(ids.clone()).unwrap();
        for id in ids { assert!(covers.iter().any(|cover| cover.app_id == id), "missing cover for {id}"); }
        if let Some(output) = std::env::var_os("BINDFORGE_COVER_TEST_OUTPUT") {
            std::fs::write(output, serde_json::to_vec(&covers).unwrap()).unwrap();
        }
    }
}
