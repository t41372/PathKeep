//! The release tool writes the desktop package version into the workspace manifest.
fn main() {
    let manifest = "../../Cargo.toml";
    println!("cargo:rerun-if-changed={manifest}");
    let text = std::fs::read_to_string(manifest).expect("desktop Cargo manifest");
    let version = text
        .split("[package]")
        .nth(1)
        .and_then(|package| {
            package
                .lines()
                .take_while(|line| !line.starts_with('['))
                .find_map(|line| line.strip_prefix("version = "))
        })
        .and_then(|value| value.trim().strip_prefix('"')?.strip_suffix('"'))
        .expect("desktop package version");
    println!("cargo:rustc-env=PATHKEEP_APP_VERSION={version}");
}
