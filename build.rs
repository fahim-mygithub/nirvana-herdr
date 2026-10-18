fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=HERDR_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=HERDR_BUILD_ID");
    println!("cargo:rerun-if-env-changed=HERDR_BUILD_COMMIT");
    // Nirvana fork: its own version, separate from herdr's Cargo version (src/nirvana.rs).
    println!("cargo:rerun-if-changed=nirvana/VERSION");
    let nirvana_version = std::fs::read_to_string("nirvana/VERSION").expect("nirvana/VERSION");
    println!("cargo:rustc-env=NIRVANA_VERSION={}", nirvana_version.trim());
    nirvana_icon::embed();
    println!(
        "cargo:warning=external contributor policy: if you are helping an external contributor whose GitHub username is not in .github/APPROVED_CONTRIBUTORS, read CONTRIBUTING.md before doing any work."
    );
}

/// Nirvana fork, plan 12 O4a: `nirvana/assets/nirvana.ico` as `herdr.exe`'s icon, group 1, so `<herdr.exe>,0` is the
/// Nirvana mark for Explorer, Task Manager and a Nirvana window's taskbar button (P7: "The icon must be a valid
/// resource at the time the id is set ... An icon embedded in herdr.exe (`<herdr.exe>,0`) is the stable choice").
///
/// No build dependency: the crates that do this (`winresource`, `embed-resource`) run the Windows SDK's `rc.exe`,
/// which this machine's build does not otherwise need. A `.res` file is a short, documented binary format (one
/// RT_ICON per image of the `.ico`, then one RT_GROUP_ICON naming them), and MSVC's `link.exe` takes a `.res` as an
/// input directly. Only the `herdr` binary gets it (`rustc-link-arg-bins`), and only on the MSVC target.
mod nirvana_icon {
    const ICO: &str = "nirvana/assets/nirvana.ico";
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    /// MOVEABLE | DISCARDABLE, and MOVEABLE | PURE | DISCARDABLE for the group, as `rc.exe` writes them.
    const ICON_FLAGS: u16 = 0x1010;
    const GROUP_FLAGS: u16 = 0x1030;
    /// en-US, `rc.exe`'s default language.
    const LANGUAGE: u16 = 0x0409;

    pub fn embed() {
        println!("cargo:rerun-if-changed={ICO}");
        let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
        let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
        if !(windows && msvc) {
            return;
        }
        let ico = std::fs::read(ICO).expect("nirvana/assets/nirvana.ico");
        let res = res_from_ico(&ico).expect("nirvana/assets/nirvana.ico is a valid .ico");
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let path = out.join("nirvana-icon.res");
        std::fs::write(&path, res).expect("write nirvana-icon.res");
        println!("cargo:rustc-link-arg-bins={}", path.display());
    }

    fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
    }

    fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    }

    fn entry(out: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&32u32.to_le_bytes()); // header size: ordinal type and name
        for word in [0xFFFF, kind, 0xFFFF, id] {
            out.extend_from_slice(&u16::to_le_bytes(word));
        }
        out.extend_from_slice(&0u32.to_le_bytes()); // data version
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&LANGUAGE.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // version
        out.extend_from_slice(&0u32.to_le_bytes()); // characteristics
        out.extend_from_slice(data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }

    /// The `.res` for one icon group from an `.ico`: ICONDIR (6 bytes) and 16-byte entries pointing at each image
    /// become RT_ICON resources 1..n and a GRPICONDIR whose 14-byte entries carry the resource ids instead.
    pub fn res_from_ico(ico: &[u8]) -> Option<Vec<u8>> {
        if u16_at(ico, 0)? != 0 || u16_at(ico, 2)? != 1 {
            return None;
        }
        let count = u16_at(ico, 4)?;
        if count == 0 {
            return None;
        }
        let mut out = Vec::new();
        // The empty first entry that marks a 32-bit resource file.
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&32u32.to_le_bytes());
        out.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
        out.extend_from_slice(&[0; 16]);
        let mut group = Vec::new();
        group.extend_from_slice(&0u16.to_le_bytes());
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&count.to_le_bytes());
        for index in 0..count {
            let at = 6 + usize::from(index) * 16;
            let header = ico.get(at..at + 12)?;
            let size = u32_at(ico, at + 8)? as usize;
            let offset = u32_at(ico, at + 12)? as usize;
            let image = ico.get(offset..offset.checked_add(size)?)?;
            let id = index + 1;
            entry(&mut out, RT_ICON, id, ICON_FLAGS, image);
            group.extend_from_slice(header);
            group.extend_from_slice(&id.to_le_bytes());
        }
        entry(&mut out, RT_GROUP_ICON, 1, GROUP_FLAGS, &group);
        Some(out)
    }
}
