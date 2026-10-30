//! Windows-only window fixes.
//!
//! A see-through (Acrylic/Mica) window extends its frame into the page, and
//! Windows then paints its own minimise/maximise/close buttons on top, even
//! though the window has no title bar. That's the second set of buttons in
//! the top-right corner. Windows only draws them when the window has a
//! system menu, so taking that flag off hides them. Arcadia's own buttons,
//! snapping, the taskbar and Alt+F4 keep working.

use raw_window_handle::HasWindowHandle;

/// Hide the caption buttons Windows draws over a see-through window.
pub fn hide_native_buttons(w: &impl HasWindowHandle) {
    #[cfg(windows)]
    {
        use raw_window_handle::RawWindowHandle;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SWP_NOZORDER, WS_SYSMENU,
        };
        let Ok(h) = w.window_handle() else { return };
        let RawWindowHandle::Win32(h) = h.as_raw() else { return };
        let hwnd = h.hwnd.get() as windows_sys::Win32::Foundation::HWND;
        // SAFETY: a live window handle from Tauri; only the style bits change.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            if style & WS_SYSMENU as isize != 0 {
                SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(WS_SYSMENU as isize));
                SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = w;
}
