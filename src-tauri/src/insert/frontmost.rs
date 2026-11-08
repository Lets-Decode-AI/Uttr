//! Frontmost-app name, secure-input and Wayland detection.

/// Name of the frontmost application, or `None` if unknown.
/// Linux: not implemented in Part 1 (returns `None`).
pub fn frontmost_app_name() -> Option<String> {
    imp::frontmost_app_name()
}

/// Whether keyboard focus is somewhere text can be pasted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Editable,
    NotEditable,
    /// The app does not say (some Electron and web apps); paste as before.
    Unknown,
}

/// Classifies the accessibility role of the focused element. `None` means macOS reported no
/// focused element, which Electron apps (Claude, Slack, VS Code) do even while a text box has
/// focus, so only a known non-text role stops the paste.
pub fn classify_role(role: Option<&str>) -> Focus {
    match role {
        None => Focus::Unknown,
        Some("AXTextField" | "AXTextArea" | "AXComboBox" | "AXSearchField") => Focus::Editable,
        Some(
            "AXButton" | "AXList" | "AXOutline" | "AXTable" | "AXRow" | "AXCell" | "AXImage"
            | "AXMenu" | "AXMenuBar" | "AXMenuItem" | "AXMenuBarItem" | "AXBrowser" | "AXCheckBox"
            | "AXRadioButton" | "AXSlider" | "AXScrollArea" | "AXDockItem",
        ) => Focus::NotEditable,
        Some(_) => Focus::Unknown,
    }
}

/// macOS: what currently has keyboard focus. Elsewhere `Unknown` (paste as before).
pub fn focused_element() -> Focus {
    imp::focused_element()
}

/// macOS: whether this app may post synthetic key events, using the same Accessibility check as
/// the permission banner (`CGPreflightPostEventAccess` can say no while Accessibility is on).
/// Checking first avoids the system's "Accessibility Access (Events)" prompt. True elsewhere.
pub fn can_post_keys() -> bool {
    imp::can_post_keys()
}

/// macOS: whether Secure Event Input is on (password fields; synthetic keys
/// are swallowed). Always false elsewhere.
pub fn secure_input_active() -> bool {
    imp::secure_input_active()
}

/// Pure Wayland detection. `DISPLAY` is deliberately ignored: XWayland sets
/// it on Wayland desktops too.
pub fn detect_wayland(wayland_display: Option<&str>, session_type: Option<&str>) -> bool {
    wayland_display.is_some() || session_type == Some("wayland")
}

/// Linux Wayland session (synthetic paste is unreliable there).
pub fn is_wayland() -> bool {
    #[cfg(target_os = "linux")]
    {
        detect_wayland(
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};
    use objc2_app_kit::NSWorkspace;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn IsSecureEventInputEnabled() -> u8;
    }

    pub fn frontmost_app_name() -> Option<String> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        app.localizedName().map(|n| n.to_string())
    }

    pub fn secure_input_active() -> bool {
        // SAFETY: no-argument Carbon query with no side effects.
        unsafe { IsSecureEventInputEnabled() != 0 }
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn AXIsProcessTrusted() -> bool;
    }

    /// Copies an attribute; the returned object is owned (create rule).
    fn copy_attr(element: CFTypeRef, name: &str) -> Result<CFType, i32> {
        let name = CFString::new(name);
        let mut value: CFTypeRef = std::ptr::null();
        // SAFETY: `element` is a live AXUIElement, `value` is a valid out pointer.
        let err = unsafe {
            AXUIElementCopyAttributeValue(element, name.as_concrete_TypeRef(), &mut value)
        };
        if err != 0 || value.is_null() {
            return Err(err);
        }
        // SAFETY: Copy functions return a +1 reference we now own.
        Ok(unsafe { CFType::wrap_under_create_rule(value) })
    }

    pub fn focused_element() -> super::Focus {
        // SAFETY: returns a +1 system-wide element we wrap and release.
        let system = unsafe { CFType::wrap_under_create_rule(AXUIElementCreateSystemWide()) };
        let role = copy_attr(system.as_CFTypeRef(), "AXFocusedUIElement")
            .ok()
            .and_then(|f| copy_attr(f.as_CFTypeRef(), "AXRole").ok())
            .and_then(|r| r.downcast::<CFString>())
            .map(|r| r.to_string());
        super::classify_role(role.as_deref())
    }

    pub fn can_post_keys() -> bool {
        // SAFETY: no-argument query; does not show a prompt.
        unsafe { AXIsProcessTrusted() }
    }
}

/// The name to record for an app: its product description when that looks like a name, else the
/// executable's file name. Descriptions that are empty, generic or very long are not used.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn display_name(description: Option<&str>, stem: &str) -> String {
    let d = description.map(str::trim).unwrap_or_default();
    let generic = ["", "application", "app", "executable"];
    if generic.contains(&d.to_lowercase().as_str()) || d.chars().count() > 40 {
        stem.to_string()
    } else {
        d.to_string()
    }
}

#[cfg(test)]
mod display_name_tests {
    use super::display_name;

    #[test]
    fn uses_the_product_description_when_it_is_a_name() {
        assert_eq!(
            display_name(Some("Brave Browser"), "brave"),
            "Brave Browser"
        );
        assert_eq!(
            display_name(Some(" Visual Studio Code "), "Code"),
            "Visual Studio Code"
        );
        assert_eq!(display_name(None, "notepad"), "notepad");
        assert_eq!(display_name(Some(""), "slack"), "slack");
        assert_eq!(display_name(Some("Application"), "foo"), "foo");
        assert_eq!(display_name(Some(&"x".repeat(60)), "foo"), "foo");
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH};
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetForegroundWindow, GetWindowThreadProcessId,
    };
    use windows_sys::core::BOOL;

    /// The app in front, by its product name ("Brave Browser", "Visual Studio Code") rather than
    /// the executable's file name ("brave", "Code"). Store apps run inside
    /// ApplicationFrameHost.exe, so the hosted app's own process is looked up instead.
    pub fn frontmost_app_name() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return None;
            }
            let mut path = process_path(pid)?;
            if stem(&path).eq_ignore_ascii_case("ApplicationFrameHost")
                && let Some(hosted) = hosted_app_pid(hwnd, pid).and_then(|p| process_path(p))
            {
                path = hosted;
            }
            Some(super::display_name(
                file_description(&path).as_deref(),
                &stem(&path),
            ))
        }
    }

    fn stem(path: &str) -> String {
        Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Full image path of a process.
    unsafe fn process_path(pid: u32) -> Option<String> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; MAX_PATH as usize];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    /// The process of the first child window that does not belong to the frame host.
    unsafe fn hosted_app_pid(hwnd: HWND, host: u32) -> Option<u32> {
        struct Search {
            host: u32,
            found: u32,
        }
        unsafe extern "system" fn visit(child: HWND, data: LPARAM) -> BOOL {
            unsafe {
                let search = &mut *(data as *mut Search);
                let mut pid = 0u32;
                GetWindowThreadProcessId(child, &mut pid);
                if pid != 0 && pid != search.host {
                    search.found = pid;
                    return 0;
                }
                1
            }
        }
        let mut search = Search { host, found: 0 };
        unsafe {
            EnumChildWindows(hwnd, Some(visit), &mut search as *mut Search as LPARAM);
        }
        (search.found != 0).then_some(search.found)
    }

    /// The executable's FileDescription version resource, if it has one.
    fn file_description(path: &str) -> Option<String> {
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let file = wide(path);
        unsafe {
            let size = GetFileVersionInfoSizeW(file.as_ptr(), std::ptr::null_mut());
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            if GetFileVersionInfoW(file.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 {
                return None;
            }
            let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut len = 0u32;
            let translation = wide(r"\VarFileInfo\Translation");
            let (lang, cp) = if VerQueryValueW(
                data.as_ptr().cast(),
                translation.as_ptr(),
                &mut ptr,
                &mut len,
            ) != 0
                && len >= 4
            {
                let pair = ptr as *const u16;
                (*pair, *pair.add(1))
            } else {
                (0x0409, 0x04B0)
            };
            let key = wide(&format!(
                r"\StringFileInfo\{lang:04x}{cp:04x}\FileDescription"
            ));
            if VerQueryValueW(data.as_ptr().cast(), key.as_ptr(), &mut ptr, &mut len) == 0
                || len == 0
            {
                return None;
            }
            let chars = std::slice::from_raw_parts(ptr as *const u16, len as usize);
            let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
            Some(String::from_utf16_lossy(&chars[..end]))
        }
    }

    pub fn secure_input_active() -> bool {
        false
    }
    pub fn focused_element() -> super::Focus {
        super::Focus::Unknown
    }
    pub fn can_post_keys() -> bool {
        true
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn frontmost_app_name() -> Option<String> {
        None
    }
    pub fn secure_input_active() -> bool {
        false
    }
    pub fn focused_element() -> super::Focus {
        super::Focus::Unknown
    }
    pub fn can_post_keys() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roles_are_editable() {
        for r in ["AXTextField", "AXTextArea", "AXComboBox"] {
            assert_eq!(classify_role(Some(r)), Focus::Editable, "{r}");
        }
    }

    #[test]
    fn unreported_focus_still_pastes() {
        // Electron apps such as Claude report no focused element while a text box has focus.
        assert_eq!(classify_role(None), Focus::Unknown);
    }

    #[test]
    fn non_text_roles_are_not_editable() {
        for r in ["AXButton", "AXList", "AXTable", "AXScrollArea", "AXImage"] {
            assert_eq!(classify_role(Some(r)), Focus::NotEditable, "{r}");
        }
    }

    #[test]
    fn unclear_roles_are_unknown_so_paste_still_happens() {
        for r in ["AXWebArea", "AXGroup", "AXWindow", "AXStaticText"] {
            assert_eq!(classify_role(Some(r)), Focus::Unknown, "{r}");
        }
    }
}
