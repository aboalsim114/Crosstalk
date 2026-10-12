fn main() {
    // Build number (commits on the branch) and commit, for the in-app update:
    // a newer build on the update feed has a higher number.
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let build = std::env::var("CROSSTALK_BUILD")
        .ok()
        .or_else(|| git(&["rev-list", "--count", "HEAD"]))
        .unwrap_or_else(|| "0".into());
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "inconnu".into());
    println!("cargo:rustc-env=CROSSTALK_BUILD={build}");
    println!("cargo:rustc-env=CROSSTALK_COMMIT={commit}");
    println!("cargo:rerun-if-env-changed=CROSSTALK_BUILD");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/heads");
    tauri_build::build()
}
