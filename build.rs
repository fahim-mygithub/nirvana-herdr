fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=HERDR_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=HERDR_BUILD_ID");
    println!("cargo:rerun-if-env-changed=HERDR_BUILD_COMMIT");
    // Nirvana fork: its own version, separate from herdr's Cargo version (src/nirvana.rs).
    println!("cargo:rerun-if-changed=nirvana/VERSION");
    let nirvana_version = std::fs::read_to_string("nirvana/VERSION").expect("nirvana/VERSION");
    println!("cargo:rustc-env=NIRVANA_VERSION={}", nirvana_version.trim());
    println!(
        "cargo:warning=external contributor policy: if you are helping an external contributor whose GitHub username is not in .github/APPROVED_CONTRIBUTORS, read CONTRIBUTING.md before doing any work."
    );
}
