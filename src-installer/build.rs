//! Embeds the application files in the installer.
//!
//! `CROSSTALK_PAYLOAD_DIR` names a flat folder with everything to install
//! (crosstalk.exe and the DLLs next to it). Each file is deflated into
//! OUT_DIR and listed in `payload.rs`; without the variable the payload is
//! empty (UI development, tests). The app version comes from
//! src-tauri/tauri.conf.json so the installer always shows the real one.

use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-env-changed=CROSSTALK_PAYLOAD_DIR");
    println!("cargo:rerun-if-changed=../src-tauri/tauri.conf.json");

    let conf: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("../src-tauri/tauri.conf.json").unwrap())
            .unwrap();
    println!(
        "cargo:rustc-env=CROSSTALK_VERSION={}",
        conf["version"].as_str().unwrap()
    );

    let mut list = String::from("pub static FILES: &[Entry] = &[\n");
    if let Some(dir) = std::env::var_os("CROSSTALK_PAYLOAD_DIR") {
        let dir = PathBuf::from(dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("CROSSTALK_PAYLOAD_DIR {}: {e}", dir.display()))
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .collect();
        names.sort();
        assert!(
            names.iter().any(|p| p.ends_with("crosstalk.exe")),
            "CROSSTALK_PAYLOAD_DIR must contain crosstalk.exe"
        );
        for path in names {
            println!("cargo:rerun-if-changed={}", path.display());
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            let data = std::fs::read(&path).unwrap();
            let packed = out.join(format!("{name}.z"));
            let mut enc =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            enc.write_all(&data).unwrap();
            std::fs::write(&packed, enc.finish().unwrap()).unwrap();
            list += &format!(
                "    Entry {{ name: {name:?}, size: {}, data: include_bytes!({:?}) }},\n",
                data.len(),
                packed.display().to_string()
            );
        }
    }
    list += "];\n";
    std::fs::write(out.join("payload.rs"), list).unwrap();

    tauri_build::build()
}
