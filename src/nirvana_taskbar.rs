//! Nirvana fork, plan 12 O4a: a Nirvana window's own taskbar button, the folder's letter as its badge, and the
//! "Nirvana · <folder>" toast.
//!
//! The owner, 2026-10-17 (plan 12 R8): "Can wew have our own icon for this? ... have its own Icon and a toast with
//! the directory first Letter as the icon". P7 (`nirvana/evidence/2026-10-17-o0-probes/p7-icon.md`) measured the
//! way on this machine; this file follows it step for step:
//!
//! - The window stays Windows Terminal. herdr's client finds it as the root owner of `GetConsoleWindow()` and sets on
//!   it `RelaunchIconResource = <herdr.exe>,0`, `RelaunchCommand`, `RelaunchDisplayNameResource = Nirvana`, and
//!   **last** `AppUserModel_ID = Nirvana.Herdr.<folder hash>`, in one commit. "WM_SETICON alone does nothing:
//!   Terminal groups under its packaged id."
//! - Then the badge, `ITaskbarList3::SetOverlayIcon`. "A badge set before the id left a stale copy on Terminal's
//!   own group. Set the id first, then the badge; on exit clear the badge, then the properties."
//! - "The icon must be a valid resource at the time the id is set": build.rs embeds `nirvana/assets/nirvana.ico`
//!   as `herdr.exe`'s first icon group.
//!
//! The badge is drawn here at run time from the numbers of `nirvana/assets/badge/badge_template_saffron_32.svg`
//! (a vertical saffron gradient tile, a near-black rim, a bold near-black letter); a test reads the template and
//! holds these numbers to it, so a redrawn template cannot drift from what herdr draws.
//!
//! Which windows are Nirvana's: the one `herdr nirvana window` (the Explorer verb) runs in, and one whose client
//! opened a space from the "new" menu's Nirvana entry. Nothing else is touched.

use std::path::{Path, PathBuf};

/// The window properties P7 set, in the order they are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    RelaunchIconResource,
    RelaunchCommand,
    RelaunchDisplayNameResource,
    AppUserModelId,
}

impl Prop {
    /// `PKEY_AppUserModel_*`: fmtid {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, these pids (propkey.h).
    pub fn pid(self) -> u32 {
        match self {
            Self::RelaunchCommand => 2,
            Self::RelaunchIconResource => 3,
            Self::RelaunchDisplayNameResource => 4,
            Self::AppUserModelId => 5,
        }
    }
}

/// One step on the window. `Set(_, None)` removes the property; `Overlay(None)` removes the badge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Set(Prop, Option<String>),
    Commit,
    Overlay(Option<char>),
}

/// What a Nirvana window shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub app_id: String,
    pub icon_resource: String,
    pub relaunch_command: String,
    pub display_name: String,
    pub letter: char,
}

impl Identity {
    pub fn for_folder(folder: &Path, herdr: &Path) -> Self {
        Self {
            app_id: crate::nirvana_launch::window_app_id(folder),
            icon_resource: format!("{},0", herdr.display()),
            relaunch_command: relaunch_command(herdr, folder),
            display_name: crate::nirvana_launch::DISPLAY_NAME.to_owned(),
            letter: crate::nirvana_launch::folder_letter(folder),
        }
    }
}

/// The longest `RelaunchCommand` the window property store takes. Measured 2026-10-17 on this machine (Windows 11
/// 26200) by `IPropertyStore::SetValue` on a Terminal window: 255 characters set, 256 to 259 gave E_INVALIDARG, 260
/// and more ERROR_INSUFFICIENT_BUFFER. The first live run used the verb's own line, 487 characters, and the button
/// never split: only the icon property took.
pub const RELAUNCH_MAX: usize = 255;

/// What a pinned Nirvana button starts: the verb's window on the same folder, `wt.exe` by its app alias (on the
/// PATH) rather than its full path, to stay inside `RELAUNCH_MAX`. A folder too long even for that relaunches the
/// "Nirvana" profile, which opens the nirvana session; the button still splits and carries the mark.
pub fn relaunch_command(herdr: &Path, folder: &Path) -> String {
    let full = format!(
        "wt.exe -w new -p \"{}\" \"{}\" --session {} nirvana window \"{}\"",
        crate::nirvana_launch::DISPLAY_NAME,
        herdr.display(),
        crate::nirvana_launch::SESSION,
        folder.display()
    );
    if full.chars().count() <= RELAUNCH_MAX {
        return full;
    }
    format!(
        "wt.exe -w new -p \"{}\"",
        crate::nirvana_launch::DISPLAY_NAME
    )
}

/// P7's order: the three relaunch properties, the id last, one commit, then the badge.
pub fn set_steps(identity: &Identity) -> Vec<Step> {
    vec![
        Step::Set(
            Prop::RelaunchIconResource,
            Some(identity.icon_resource.clone()),
        ),
        Step::Set(
            Prop::RelaunchCommand,
            Some(identity.relaunch_command.clone()),
        ),
        Step::Set(
            Prop::RelaunchDisplayNameResource,
            Some(identity.display_name.clone()),
        ),
        Step::Set(Prop::AppUserModelId, Some(identity.app_id.clone())),
        Step::Commit,
        Step::Overlay(Some(identity.letter)),
    ]
}

/// P7's order backwards: the badge first, then the id, then the rest, one commit.
pub fn clear_steps() -> Vec<Step> {
    vec![
        Step::Overlay(None),
        Step::Set(Prop::AppUserModelId, None),
        Step::Set(Prop::RelaunchIconResource, None),
        Step::Set(Prop::RelaunchCommand, None),
        Step::Set(Prop::RelaunchDisplayNameResource, None),
        Step::Commit,
    ]
}

/// The window the steps act on.
pub trait WindowShell {
    fn set(&mut self, prop: Prop, value: Option<&str>) -> std::io::Result<()>;
    fn commit(&mut self) -> std::io::Result<()>;
    fn overlay(&mut self, letter: Option<char>) -> std::io::Result<()>;
}

/// Runs `steps` in order. Setting stops at the first failure, so a badge never goes on a window whose id did not
/// take (the stale badge P7 saw); clearing goes on, so one failure leaves as little behind as it can.
pub fn apply(
    shell: &mut impl WindowShell,
    steps: &[Step],
    keep_going: bool,
) -> std::io::Result<()> {
    let mut first_error = None;
    for step in steps {
        let result = match step {
            Step::Set(prop, value) => shell.set(*prop, value.as_deref()),
            Step::Commit => shell.commit(),
            Step::Overlay(letter) => shell.overlay(*letter),
        };
        if let Err(err) = result {
            if !keep_going {
                return Err(err);
            }
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

// ---- the badge ----

/// The numbers of `badge_template_saffron_32.svg`, in its 32-unit square.
pub mod template {
    pub const UNITS: f32 = 32.0;
    pub const TOP: [u8; 3] = [0xF7, 0xB9, 0x55];
    pub const BOTTOM: [u8; 3] = [0xE8, 0x95, 0x2C];
    pub const INK: [u8; 3] = [0x1A, 0x1A, 0x1A];
    /// `<rect x="1.0" y="1.0" width="30.0" height="30.0" rx="7.0" stroke-width="2.0">`
    pub const RECT_INSET: f32 = 1.0;
    pub const RECT_SIZE: f32 = 30.0;
    pub const RADIUS: f32 = 7.0;
    pub const STROKE: f32 = 2.0;
    /// `<text x="16.0" y="24.4" font-weight="700" font-size="23.0">`
    pub const TEXT_X: f32 = 16.0;
    pub const TEXT_BASELINE: f32 = 24.4;
    pub const FONT_SIZE: f32 = 23.0;
    pub const FONT_FACE: &str = "Segoe UI";
}

/// The badge tile, `size` pixels square, as straight-alpha RGBA rows, with the letter's coverage (one byte a
/// pixel, 255 = ink) composited when given.
pub fn badge_rgba(size: u32, letter_coverage: Option<&[u8]>) -> Vec<u8> {
    use template::*;
    let scale = size as f32 / UNITS;
    let half = RECT_SIZE / 2.0 * scale;
    let radius = RADIUS * scale;
    let stroke = STROKE * scale;
    let centre = (RECT_INSET + RECT_SIZE / 2.0) * scale;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5 - centre;
            let py = y as f32 + 0.5 - centre;
            // Signed distance to the rounded square the stroke is centred on.
            let qx = px.abs() - (half - radius);
            let qy = py.abs() - (half - radius);
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let distance = outside + qx.max(qy).min(0.0) - radius;
            let shape = (0.5 - (distance - stroke / 2.0)).clamp(0.0, 1.0);
            let fill = (0.5 - (distance + stroke / 2.0)).clamp(0.0, 1.0);
            let t = ((y as f32 + 0.5) / size as f32).clamp(0.0, 1.0);
            let mut rgb = [0f32; 3];
            for (channel, value) in rgb.iter_mut().enumerate() {
                let gradient =
                    TOP[channel] as f32 + (BOTTOM[channel] as f32 - TOP[channel] as f32) * t;
                *value = INK[channel] as f32 + (gradient - INK[channel] as f32) * fill;
            }
            if let Some(coverage) = letter_coverage {
                let ink = coverage[(y * size + x) as usize] as f32 / 255.0 * fill;
                for (channel, value) in rgb.iter_mut().enumerate() {
                    *value += (INK[channel] as f32 - *value) * ink;
                }
            }
            out.extend(rgb.iter().map(|v| v.round().clamp(0.0, 255.0) as u8));
            out.push((shape * 255.0).round() as u8);
        }
    }
    out
}

/// The badge as a PNG file, for the toast's `appLogoOverride`.
pub fn write_badge_png(path: &Path, size: u32, letter: char) -> std::io::Result<()> {
    let coverage = letter_coverage(letter, size);
    let rgba = badge_rgba(size, coverage.as_deref());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(&rgba)
        .map_err(std::io::Error::other)?;
    Ok(())
}

/// Where the toast's letter picture goes: one file per letter, under the temp folder, rewritten each open.
pub fn toast_logo_path(letter: char) -> PathBuf {
    std::env::temp_dir()
        .join("nirvana-herdr")
        .join(format!("badge-{:04x}-96.png", letter as u32))
}

#[cfg(windows)]
// The menu's claim runs only outside tests (client/shell/nirvana_new_menu.rs), and terminal_window is used in step 2.
#[allow(unused_imports)]
pub use windows_impl::{
    claim_menu_window, claim_window, letter_coverage, release_window, terminal_window,
};

#[cfg(not(windows))]
pub fn claim_window(_folder: Option<&Path>) -> std::io::Result<()> {
    Err(std::io::Error::other("the taskbar button is Windows only"))
}
#[cfg(not(windows))]
pub fn claim_menu_window(_folder: &Path) {}
#[cfg(not(windows))]
pub fn release_window() {}
#[cfg(not(windows))]
pub fn letter_coverage(_letter: char, _size: u32) -> Option<Vec<u8>> {
    None
}
#[cfg(not(windows))]
pub fn terminal_window() -> Option<usize> {
    None
}
#[cfg(not(windows))]
pub fn flash_window(_hwnd: usize) -> std::io::Result<()> {
    Ok(())
}

/// Plan 21 §5: FlashWindowEx on the mark's window handle with FLASHW_TRAY | FLASHW_TIMERNOFG.
#[cfg(windows)]
pub fn flash_window(hwnd: usize) -> std::io::Result<()> {
    if hwnd == 0 {
        return Ok(());
    }
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FlashWindowEx, FLASHWINFO, FLASHW_TIMERNOFG, FLASHW_TRAY,
    };
    let mut info = FLASHWINFO {
        cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
        hwnd: hwnd as _,
        dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
        uCount: 0,
        dwTimeout: 0,
    };
    // SAFETY: info has its cbSize set correctly, FlashWindowEx safely handles invalid window handles.
    unsafe { FlashWindowEx(&mut info) };
    Ok(())
}

/// The toast "Nirvana · <folder>" with the folder's letter as its picture.
// The menu's toast runs only outside tests (client/shell/nirvana_new_menu.rs), and the verb's window now names its
// preset through `toast_open_with`, so a test build never uses this one.
#[cfg_attr(test, allow(dead_code))]
pub fn toast_open(folder: &Path) {
    toast_open_with(folder, None);
}

/// Plan 13 §4: the open toast with the folder's preset, "Nirvana · <folder> · 3d-print preset · 31 off · −5.9k a
/// turn · Preload p to undo". One line in the title: `show_nirvana_toast` takes no body (the platform file is not
/// this package's), and Windows wraps a long title onto its second line.
pub fn toast_text(folder: &Path, preset: Option<&str>) -> String {
    let title = crate::nirvana_launch::toast_title(folder);
    match preset {
        Some(line) => format!("{title} \u{b7} {line}"),
        None => title,
    }
}

pub fn toast_open_with(folder: &Path, preset: Option<&str>) {
    #[cfg(windows)]
    {
        let title = toast_text(folder, preset);
        let logo = toast_logo_path(crate::nirvana_launch::folder_letter(folder));
        let logo = write_badge_png(&logo, 96, crate::nirvana_launch::folder_letter(folder))
            .map(|()| logo)
            .map_err(|err| tracing::warn!(%err, "nirvana: the toast's letter was not drawn"))
            .ok();
        if let Err(err) = crate::platform::show_nirvana_toast(&title, logo.as_deref()) {
            tracing::warn!(%err, "nirvana: the open toast was not shown");
        }
    }
    #[cfg(not(windows))]
    let _ = (folder, preset);
}

/// The installed `herdr.exe` when there is one (its icon resource is the one the id points at), else this one.
pub fn herdr_exe() -> PathBuf {
    crate::nirvana::home()
        .map(|home| crate::nirvana::installed_exe_in(&home))
        .filter(|exe| exe.is_file())
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("herdr.exe"))
}

#[cfg(windows)]
mod windows_impl {
    use std::path::Path;
    use std::sync::Mutex;

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{HWND, PROPERTYKEY};
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IPropertyStore, SHGetPropertyStoreForWindow,
    };
    use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
    use windows::Win32::UI::WindowsAndMessaging::HICON;

    use super::{apply, clear_steps, set_steps, Identity, Prop, Step, WindowShell};

    /// The window this process claimed, so the release clears the same one even if the console moved.
    static CLAIMED: Mutex<Option<usize>> = Mutex::new(None);

    const APP_USER_MODEL: windows::core::GUID =
        windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3);

    pub fn terminal_window() -> Option<usize> {
        use windows_sys::Win32::System::Console::GetConsoleWindow;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOTOWNER};
        // SAFETY: both calls take and return plain handles; a null console window gives a null ancestor.
        let window = unsafe { GetAncestor(GetConsoleWindow(), GA_ROOTOWNER) };
        (!window.is_null()).then_some(window as usize)
    }

    struct Shell {
        window: HWND,
        store: IPropertyStore,
        taskbar: Option<ITaskbarList3>,
    }

    impl Shell {
        fn open(window: usize) -> std::io::Result<Self> {
            let window = HWND(window as *mut core::ffi::c_void);
            // SAFETY: a window handle, possibly of another process, which the shell accepts (P7).
            let store: IPropertyStore =
                unsafe { SHGetPropertyStoreForWindow(window) }.map_err(std::io::Error::other)?;
            Ok(Self {
                window,
                store,
                taskbar: None,
            })
        }

        fn taskbar(&mut self) -> std::io::Result<&ITaskbarList3> {
            if self.taskbar.is_none() {
                // SAFETY: the CLSID and interface are the shell's own; HrInit is required before use.
                let taskbar: ITaskbarList3 =
                    unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }
                        .map_err(std::io::Error::other)?;
                unsafe { taskbar.HrInit() }.map_err(std::io::Error::other)?;
                self.taskbar = Some(taskbar);
            }
            Ok(self.taskbar.as_ref().expect("just set"))
        }
    }

    /// A `VT_LPWSTR` value, the type the AppUserModel keys take; its string is freed by `PropVariantClear` on drop.
    fn lpwstr(value: &str) -> std::io::Result<PROPVARIANT> {
        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a fresh CoTaskMem block of exactly the bytes copied into it.
        let block = unsafe { CoTaskMemAlloc(wide.len() * 2) } as *mut u16;
        if block.is_null() {
            return Err(std::io::Error::other("CoTaskMemAlloc failed"));
        }
        unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), block, wide.len()) };
        let mut variant = PROPVARIANT::default();
        // SAFETY: writing the tag and the matching union member of a VT_EMPTY variant.
        unsafe {
            let inner = &mut variant.Anonymous.Anonymous;
            inner.vt = VT_LPWSTR;
            inner.Anonymous.pwszVal = PWSTR(block);
        }
        Ok(variant)
    }

    impl WindowShell for Shell {
        fn set(&mut self, prop: Prop, value: Option<&str>) -> std::io::Result<()> {
            let key = PROPERTYKEY {
                fmtid: APP_USER_MODEL,
                pid: prop.pid(),
            };
            let variant = match value {
                Some(value) => lpwstr(value)?,
                None => PROPVARIANT::default(),
            };
            // SAFETY: key and variant live across the call; the store copies the value.
            unsafe { self.store.SetValue(&key, &variant) }.map_err(std::io::Error::other)
        }

        fn commit(&mut self) -> std::io::Result<()> {
            unsafe { self.store.Commit() }.map_err(std::io::Error::other)
        }

        fn overlay(&mut self, letter: Option<char>) -> std::io::Result<()> {
            let window = self.window;
            let taskbar = self.taskbar()?.clone();
            match letter {
                None => unsafe { taskbar.SetOverlayIcon(window, HICON::default(), None) }
                    .map_err(std::io::Error::other),
                Some(letter) => {
                    let icon = badge_icon(letter)?;
                    let description: Vec<u16> = format!("Nirvana {letter}")
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    // SAFETY: the taskbar copies the icon, so it is destroyed after the call.
                    let result = unsafe {
                        taskbar.SetOverlayIcon(
                            window,
                            HICON(icon),
                            windows::core::PCWSTR(description.as_ptr()),
                        )
                    };
                    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(icon) };
                    result.map_err(std::io::Error::other)
                }
            }
        }
    }

    /// Runs `work` on a fresh single-threaded COM apartment, as the shell's taskbar objects expect.
    fn on_com_thread<T: Send + 'static>(
        work: impl FnOnce() -> std::io::Result<T> + Send + 'static,
    ) -> std::io::Result<T> {
        std::thread::Builder::new()
            .name("nirvana-taskbar".into())
            .spawn(move || {
                // SAFETY: paired with CoUninitialize on this same thread.
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                    .ok()
                    .map_err(std::io::Error::other)?;
                let result = work();
                unsafe { CoUninitialize() };
                result
            })?
            .join()
            .map_err(|_| std::io::Error::other("the taskbar thread panicked"))?
    }

    /// Makes this process's Terminal window a Nirvana window for `folder`: its own button with the mark, the letter.
    pub fn claim_window(folder: Option<&Path>) -> std::io::Result<()> {
        let window = terminal_window().ok_or_else(|| std::io::Error::other("no console window"))?;
        let dummy = std::path::PathBuf::from("");
        let folder = folder.unwrap_or(&dummy);
        let identity = Identity::for_folder(folder, &super::herdr_exe());
        let steps = set_steps(&identity);
        on_com_thread(move || {
            let mut shell = Shell::open(window)?;
            let set = apply(&mut shell, &steps, false);
            if set.is_err() {
                // A window's property store applies each value as it is set, so a refusal midway leaves the earlier
                // ones on the window (the first live run left the icon); take them off again.
                let _ = apply(&mut shell, &clear_steps(), true);
            }
            set
        })?;
        *CLAIMED.lock().unwrap_or_else(|e| e.into_inner()) = Some(window);
        tracing::info!(app_id = %identity.app_id, letter = %identity.letter, "nirvana: claimed the window");
        Ok(())
    }

    /// The "new" menu's Nirvana entry: claim this client's window unless the verb's window already did, or this
    /// client already claimed it for another folder.
    #[cfg_attr(test, allow(dead_code))]
    pub fn claim_menu_window(folder: &Path) {
        if std::env::var_os(crate::nirvana_launch::WINDOW_FOLDER_ENV).is_some()
            || CLAIMED.lock().unwrap_or_else(|e| e.into_inner()).is_some()
        {
            return;
        }
        if let Err(err) = claim_window(Some(folder)) {
            tracing::warn!(%err, "nirvana: the taskbar identity was not set");
        }
    }

    /// Clears what `claim_window` set: the badge, then the properties (P7). Nothing when nothing was claimed.
    pub fn release_window() {
        let Some(window) = CLAIMED.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        let steps: Vec<Step> = clear_steps();
        if let Err(err) = on_com_thread(move || {
            let mut shell = Shell::open(window)?;
            apply(&mut shell, &steps, true)
        }) {
            tracing::warn!(%err, "nirvana: the taskbar identity was not fully cleared");
        }
    }

    /// The letter's coverage drawn by GDI at the template's size and baseline: one byte a pixel, 255 = ink.
    pub fn letter_coverage(letter: char, size: u32) -> Option<Vec<u8>> {
        use super::template::*;
        use windows_sys::Win32::Graphics::Gdi::*;
        let scale = size as f32 / UNITS;
        let mut text = [0u16; 2];
        let text = letter.encode_utf16(&mut text);
        let face: Vec<u16> = FONT_FACE.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: every GDI object made here is selected out and deleted before return; `bits` is read only while
        // the DIB section that owns it is alive.
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let (bitmap, bits) = dib_section(dc, size)?;
            let old_bitmap = SelectObject(dc, bitmap);
            let font = CreateFontW(
                -((FONT_SIZE * scale).round() as i32),
                0,
                0,
                0,
                FW_BOLD as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                ANTIALIASED_QUALITY as u32,
                (DEFAULT_PITCH | FF_SWISS) as u32,
                face.as_ptr(),
            );
            let old_font = SelectObject(dc, font);
            SetTextColor(dc, 0x00FF_FFFF);
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextAlign(dc, TA_BASELINE | TA_CENTER);
            TextOutW(
                dc,
                (TEXT_X * scale).round() as i32,
                (TEXT_BASELINE * scale).round() as i32,
                text.as_ptr(),
                text.len() as i32,
            );
            GdiFlush();
            let pixels = std::slice::from_raw_parts(bits as *const u8, (size * size * 4) as usize);
            // BGRA rows; grey anti-aliasing puts the same coverage in each channel, so green is read.
            let coverage = pixels.chunks_exact(4).map(|px| px[1]).collect();
            SelectObject(dc, old_font);
            DeleteObject(font);
            SelectObject(dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(dc);
            Some(coverage)
        }
    }

    /// A top-down 32-bit DIB section, zeroed.
    unsafe fn dib_section(
        dc: windows_sys::Win32::Graphics::Gdi::HDC,
        size: u32,
    ) -> Option<(
        windows_sys::Win32::Graphics::Gdi::HBITMAP,
        *mut core::ffi::c_void,
    )> {
        use windows_sys::Win32::Graphics::Gdi::*;
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size as i32,
                biHeight: -(size as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            },
            ..std::mem::zeroed()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            dc,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        (!bitmap.is_null() && !bits.is_null()).then_some((bitmap, bits))
    }

    /// The badge as an icon at the small-icon size Windows draws overlays at (16 px at 100%).
    fn badge_icon(
        letter: char,
    ) -> std::io::Result<windows_sys::Win32::UI::WindowsAndMessaging::HICON> {
        use windows_sys::Win32::Graphics::Gdi::*;
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        // SAFETY: plain metric read.
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.clamp(16, 64) as u32;
        let rgba = super::badge_rgba(size, letter_coverage(letter, size).as_deref());
        // SAFETY: the DIB is filled within its bounds; both bitmaps are deleted after CreateIconIndirect copied them.
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let (color, bits) = dib_section(dc, size).ok_or_else(std::io::Error::last_os_error)?;
            let target =
                std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
            for (dst, src) in target.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
            }
            let stride = (size as usize).div_ceil(16) * 2;
            let mask_bits = vec![0u8; stride * size as usize];
            let mask = CreateBitmap(size as i32, size as i32, 1, 1, mask_bits.as_ptr().cast());
            let info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            let icon = CreateIconIndirect(&info);
            DeleteObject(mask);
            DeleteObject(color);
            DeleteDC(dc);
            if icon.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            Ok(icon)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        steps: Vec<Step>,
        fail_on: Option<Prop>,
    }

    impl WindowShell for Recorder {
        fn set(&mut self, prop: Prop, value: Option<&str>) -> std::io::Result<()> {
            self.steps.push(Step::Set(prop, value.map(str::to_owned)));
            if self.fail_on == Some(prop) {
                return Err(std::io::Error::other("refused"));
            }
            Ok(())
        }
        fn commit(&mut self) -> std::io::Result<()> {
            self.steps.push(Step::Commit);
            Ok(())
        }
        fn overlay(&mut self, letter: Option<char>) -> std::io::Result<()> {
            self.steps.push(Step::Overlay(letter));
            Ok(())
        }
    }

    fn identity() -> Identity {
        Identity::for_folder(
            Path::new(r"C:\Users\o\Desktop\Pojects"),
            Path::new(r"C:\Users\o\AppData\Local\Programs\Nirvana-Herdr\bin\herdr.exe"),
        )
    }

    #[test]
    fn the_id_is_set_last_and_the_badge_only_after_the_commit() {
        let identity = identity();
        let mut shell = Recorder::default();
        apply(&mut shell, &set_steps(&identity), false).unwrap();
        let kinds: Vec<_> = shell
            .steps
            .iter()
            .map(|step| match step {
                Step::Set(prop, _) => format!("{prop:?}"),
                Step::Commit => "Commit".into(),
                Step::Overlay(letter) => format!("Overlay({letter:?})"),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "RelaunchIconResource",
                "RelaunchCommand",
                "RelaunchDisplayNameResource",
                "AppUserModelId",
                "Commit",
                "Overlay(Some('P'))",
            ]
        );
        assert_eq!(
            shell.steps[0],
            Step::Set(
                Prop::RelaunchIconResource,
                Some(r"C:\Users\o\AppData\Local\Programs\Nirvana-Herdr\bin\herdr.exe,0".into())
            )
        );
        assert_eq!(
            shell.steps[3],
            Step::Set(
                Prop::AppUserModelId,
                Some(crate::nirvana_launch::window_app_id(Path::new(
                    r"C:\Users\o\Desktop\Pojects"
                )))
            )
        );
        assert_eq!(
            shell.steps[2],
            Step::Set(Prop::RelaunchDisplayNameResource, Some("Nirvana".into()))
        );
    }

    #[test]
    fn clearing_takes_the_badge_off_first_then_every_property() {
        let mut shell = Recorder::default();
        apply(&mut shell, &clear_steps(), true).unwrap();
        assert_eq!(shell.steps.first(), Some(&Step::Overlay(None)));
        assert_eq!(shell.steps.last(), Some(&Step::Commit));
        for prop in [
            Prop::AppUserModelId,
            Prop::RelaunchIconResource,
            Prop::RelaunchCommand,
            Prop::RelaunchDisplayNameResource,
        ] {
            assert!(shell.steps.contains(&Step::Set(prop, None)), "{prop:?}");
        }
    }

    #[test]
    fn a_refused_id_puts_no_badge_on_but_a_refused_clear_still_clears_the_rest() {
        let mut shell = Recorder {
            fail_on: Some(Prop::AppUserModelId),
            ..Recorder::default()
        };
        assert!(apply(&mut shell, &set_steps(&identity()), false).is_err());
        assert!(!shell
            .steps
            .iter()
            .any(|step| matches!(step, Step::Overlay(_))));
        let mut shell = Recorder {
            fail_on: Some(Prop::AppUserModelId),
            ..Recorder::default()
        };
        assert!(apply(&mut shell, &clear_steps(), true).is_err());
        assert_eq!(shell.steps.len(), clear_steps().len());
    }

    #[test]
    fn the_relaunch_command_reopens_the_same_folder_and_fits_the_property() {
        let command = identity().relaunch_command;
        assert_eq!(
            command,
            r#"wt.exe -w new -p "Nirvana" "C:\Users\o\AppData\Local\Programs\Nirvana-Herdr\bin\herdr.exe" --session nirvana nirvana window "C:\Users\o\Desktop\Pojects""#
        );
        assert!(command.chars().count() <= RELAUNCH_MAX);
        // The owner's own install and project paths fit; a folder past the limit relaunches the profile.
        let herdr = Path::new(r"C:\Users\fahim\AppData\Local\Programs\Nirvana-Herdr\bin\herdr.exe");
        let fits = relaunch_command(
            herdr,
            Path::new(r"C:\Users\fahim\Desktop\Pojects\MotoProject"),
        );
        assert!(
            fits.contains("nirvana window") && fits.chars().count() <= RELAUNCH_MAX,
            "{fits}"
        );
        let deep = format!(r"C:\{}", "d".repeat(200));
        assert_eq!(
            relaunch_command(herdr, Path::new(&deep)),
            r#"wt.exe -w new -p "Nirvana""#
        );
    }

    #[test]
    fn the_badge_holds_the_templates_numbers() {
        // The drawing above uses these numbers; the template is the owner's. One must follow the other.
        use template::*;
        let svg = include_str!("../nirvana/assets/badge/badge_template_saffron_32.svg");
        let hex = |c: [u8; 3]| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
        for needle in [
            format!("stop-color=\"{}\"", hex(TOP)),
            format!("stop-color=\"{}\"", hex(BOTTOM)),
            format!(
                "x=\"{RECT_INSET:.1}\" y=\"{RECT_INSET:.1}\" width=\"{RECT_SIZE:.1}\" height=\"{RECT_SIZE:.1}\" rx=\"{RADIUS:.1}\""
            ),
            format!("stroke=\"{}\" stroke-width=\"{STROKE:.1}\"", hex(INK)),
            format!("x=\"{TEXT_X:.1}\" y=\"{TEXT_BASELINE:.1}\""),
            format!("font-weight=\"700\" font-size=\"{FONT_SIZE:.1}\" fill=\"{}\"", hex(INK)),
            "{LETTER}".to_string(),
        ] {
            assert!(svg.contains(&needle), "the template no longer has {needle}");
        }
        assert_eq!(UNITS, 32.0);
    }

    #[test]
    fn the_tile_is_saffron_inside_dark_at_the_rim_and_clear_at_the_corners() {
        let size = 32;
        let rgba = badge_rgba(size, None);
        let px = |x: u32, y: u32| {
            let i = ((y * size + x) * 4) as usize;
            [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
        };
        assert_eq!(
            px(0, 0)[3],
            0,
            "the corner outside the rounded tile is clear"
        );
        let top = px(16, 3);
        assert!(top[3] == 255 && top[0] > 0xF0 && top[2] < 0x70, "{top:?}");
        let bottom = px(16, 28);
        assert!(
            bottom[0] > 0xE0 && bottom[1] < top[1],
            "{bottom:?} is the darker end"
        );
        let rim = px(16, 0);
        assert!(
            rim[0] < 0x40 && rim[3] > 0x80,
            "{rim:?} is the near-black rim"
        );
        // Full ink coverage turns the inside near-black.
        let ink = badge_rgba(size, Some(&vec![255; (size * size) as usize]));
        let i = ((16 * size + 16) * 4) as usize;
        assert_eq!(&ink[i..i + 3], &[0x1A, 0x1A, 0x1A]);
    }

    #[cfg(windows)]
    #[test]
    fn gdi_draws_ink_for_a_letter_and_none_for_a_space() {
        let letter = letter_coverage('P', 32).expect("GDI drew");
        let inked = letter.iter().filter(|&&c| c > 128).count();
        assert!(inked > 30, "only {inked} pixels of P");
        let blank = letter_coverage(' ', 32).expect("GDI drew");
        assert_eq!(blank.iter().filter(|&&c| c > 128).count(), 0);
    }

    #[test]
    fn the_toast_picture_is_a_png_of_the_badge() {
        let dir = std::env::temp_dir().join(format!("nirvana-badge-{}", std::process::id()));
        let path = dir.join("b.png");
        write_badge_png(&path, 96, 'P').unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (96, 96));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(toast_logo_path('P')
            .display()
            .to_string()
            .ends_with("badge-0050-96.png"));
    }

    #[test]
    fn the_open_toast_carries_the_presets_line_after_the_folder() {
        let folder = Path::new(r"C:\work\3D-Printer-Flow-xtool");
        assert_eq!(
            toast_text(folder, None),
            "Nirvana \u{b7} 3D-Printer-Flow-xtool"
        );
        assert_eq!(
            toast_text(
                folder,
                Some("3d-print preset \u{b7} 31 off \u{b7} \u{2212}5.9k a turn \u{b7} Preload p to undo")
            ),
            "Nirvana \u{b7} 3D-Printer-Flow-xtool \u{b7} 3d-print preset \u{b7} 31 off \u{b7} \u{2212}5.9k a turn \u{b7} Preload p to undo"
        );
    }
}
