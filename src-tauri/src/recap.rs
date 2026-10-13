//! End-of-match recap: the interface draws the card (PNG); this saves it in
//! the user's Documents folder, where it can be posted on Discord or X.
//! Nothing is sent anywhere by Crosstalk.

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Checks the image and writes it to `to`.
pub fn save(png: &[u8], to: &std::path::Path) -> Result<(), String> {
    if !png.starts_with(PNG_MAGIC) {
        return Err("image invalide".into());
    }
    if png.len() > MAX_BYTES {
        return Err("image trop grande".into());
    }
    std::fs::write(to, png).map_err(|e| format!("écriture impossible : {e}"))
}

/// Saves the recap card as `Documents\Crosstalk-resume-<date>.png` and
/// returns the path.
#[tauri::command]
pub fn recap_save(png: Vec<u8>) -> Result<String, String> {
    let path = crate::history::documents_path("Crosstalk-resume", "png", crate::history::now_ms());
    save(&png, &path)?;
    crate::logs::info("recap", "résumé de partie enregistré");
    Ok(path.display().to_string())
}

/// Shows a saved recap in the file explorer.
#[tauri::command]
pub fn recap_reveal(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    let name_ok = p
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("Crosstalk-resume-") && n.ends_with(".png"));
    if !name_ok || !p.is_file() {
        return Err("fichier introuvable".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{path}"))
            .spawn()
            .map(drop)
            .map_err(|e| format!("ouverture impossible : {e}"))
    }
    #[cfg(not(windows))]
    {
        crate::logs::shell_open(&p.parent().unwrap_or(p).display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_only_png_images() {
        let to = std::env::temp_dir().join(format!("crosstalk-recap-{}.png", std::process::id()));
        assert!(save(b"GIF89a....", &to).is_err());
        let mut big = PNG_MAGIC.to_vec();
        big.resize(MAX_BYTES + 1, 0);
        assert!(save(&big, &to).is_err());
        let png = [PNG_MAGIC, b"rest"].concat();
        save(&png, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), png);
        let _ = std::fs::remove_file(&to);
        assert!(
            recap_reveal("C:/Windows/notepad.exe".into()).is_err(),
            "only recap files"
        );
    }
}
