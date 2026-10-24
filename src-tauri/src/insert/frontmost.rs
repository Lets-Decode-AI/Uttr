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

/// Classifies the accessibility role of the focused element. `None` means nothing has focus.
pub fn classify_role(role: Option<&str>) -> Focus {
    match role {
        None => Focus::NotEditable,
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

/// macOS: whether this app may post synthetic key events. Checking first avoids the system's
/// "Accessibility Access (Events)" prompt on every paste. Always true elsewhere.
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
        fn CGPreflightPostEventAccess() -> bool;
    }

    /// kAXErrorNoValue: the attribute exists but has no value (nothing is focused).
    const AX_ERROR_NO_VALUE: i32 = -25212;

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
        let focused = match copy_attr(system.as_CFTypeRef(), "AXFocusedUIElement") {
            Ok(f) => f,
            Err(AX_ERROR_NO_VALUE) => return super::classify_role(None),
            Err(_) => return super::Focus::Unknown,
        };
        let role = copy_attr(focused.as_CFTypeRef(), "AXRole")
            .ok()
            .and_then(|r| r.downcast::<CFString>())
            .map(|r| r.to_string());
        match role {
            Some(r) => super::classify_role(Some(&r)),
            None => super::Focus::Unknown,
        }
    }

    pub fn can_post_keys() -> bool {
        // SAFETY: no-argument query; does not show a prompt.
        unsafe { CGPreflightPostEventAccess() }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, MAX_PATH};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

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
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; MAX_PATH as usize];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            if ok == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
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
    fn nothing_focused_or_non_text_roles_are_not_editable() {
        assert_eq!(classify_role(None), Focus::NotEditable);
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
