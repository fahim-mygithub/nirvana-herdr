//! Nirvana fork, plan 12 O4a: `herdr.exe` carries `nirvana/assets/nirvana.ico` as its icon group 1 (build.rs), so
//! `<herdr.exe>,0`, the taskbar id's RelaunchIconResource, is the Nirvana mark.

#![cfg(windows)]

use windows_sys::Win32::Foundation::FreeLibrary;
use windows_sys::Win32::System::LibraryLoader::{
    FindResourceW, LoadLibraryExW, SizeofResource, LOAD_LIBRARY_AS_DATAFILE,
    LOAD_LIBRARY_AS_IMAGE_RESOURCE,
};

const RT_ICON: usize = 3;
const RT_GROUP_ICON: usize = 14;

/// MAKEINTRESOURCE: a resource id or type passed as a pointer value.
fn ordinal(id: usize) -> *const u16 {
    std::ptr::without_provenance(id)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[test]
fn herdr_exe_carries_the_nirvana_mark_as_its_first_icon_group() {
    let ico = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/nirvana/assets/nirvana.ico"
    ))
    .expect("the mark");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let sizes: Vec<u32> = (0..count)
        .map(|i| {
            let at = 6 + i * 16 + 8;
            u32::from_le_bytes(ico[at..at + 4].try_into().unwrap())
        })
        .collect();
    assert_eq!(count, 6, "16, 24, 32, 48, 64 and 256");

    let exe = wide(env!("CARGO_BIN_EXE_herdr"));
    // SAFETY: loaded as data only, freed at the end; the resource ids are MAKEINTRESOURCE ordinals.
    unsafe {
        let module = LoadLibraryExW(
            exe.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        );
        assert!(!module.is_null(), "herdr.exe loads as data");
        let group = FindResourceW(module, ordinal(1), ordinal(RT_GROUP_ICON));
        assert!(!group.is_null(), "icon group 1 is there");
        let found: Vec<u32> = (1..=count)
            .map(|id| {
                let icon = FindResourceW(module, ordinal(id), ordinal(RT_ICON));
                if icon.is_null() {
                    0
                } else {
                    SizeofResource(module, icon)
                }
            })
            .collect();
        FreeLibrary(module);
        assert_eq!(
            found, sizes,
            "each image of the .ico, byte for byte in size"
        );
    }
}
