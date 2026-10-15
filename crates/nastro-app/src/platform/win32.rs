//! The main window's shape on Windows. gpui-pre 0.3.7 implements `Window::set_input_region`
//! only on Wayland, so on Windows the transparent margin and the gaps between the panels would
//! take the clicks meant for the windows behind. A window region (`SetWindowRgn`) limits both
//! where the window takes input and what it draws.
//!
//! The only `unsafe` code in Nastro (`AGENTS.md`): Win32 calls on the main window's handle,
//! always from the UI thread that owns it.

#![allow(unsafe_code)]

use gpui_kit::{Bounds, Edges, Pixels, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, CombineRgn, CreateRectRgn, DeleteObject, HRGN, RGN_ERROR, RGN_OR, SetWindowRgn,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

/// Limits `window` to `panels` grown by `shadow`, or gives it its whole rectangle back with
/// `None`.
pub fn set_window_region(
    window: &Window,
    panels: Option<&[Bounds<Pixels>]>,
    shadow: Edges<Pixels>,
) {
    let Some(hwnd) = hwnd(window) else {
        log::warn!("no Win32 handle for the main window, its region is left as it is");
        return;
    };
    let region = match panels {
        Some(panels) => {
            let scale = window.scale_factor();
            let rects: Vec<RECT> = panels
                .iter()
                .map(|panel| device_rect(panel.extend(shadow), scale))
                .collect();
            match build_region(hwnd, &rects) {
                Some(region) => region,
                None => {
                    log::warn!("could not build the main window region");
                    return;
                }
            }
        }
        // A null region removes the window region.
        None => std::ptr::null_mut(),
    };
    // SAFETY: `hwnd` is the live main window and this runs on its thread. On success the system
    // owns `region`; on failure it is still ours, so it is deleted.
    unsafe {
        if SetWindowRgn(hwnd, region, 1) == 0 {
            log::warn!("SetWindowRgn failed, the main window keeps its previous region");
            if !region.is_null() {
                DeleteObject(region);
            }
        }
    }
}

/// The window's Win32 handle. Called through the trait: `Window::window_handle` is also an
/// inherent method, which returns gpui's own handle.
fn hwnd(window: &Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
        _ => None,
    }
}

/// `bounds` in device pixels, rounded outwards so the edge pixels stay inside.
fn device_rect(bounds: Bounds<Pixels>, scale: f32) -> RECT {
    let device = |value: Pixels, round: fn(f32) -> f32| round(f32::from(value) * scale) as i32;
    RECT {
        left: device(bounds.left(), f32::floor),
        top: device(bounds.top(), f32::floor),
        right: device(bounds.right(), f32::ceil),
        bottom: device(bounds.bottom(), f32::ceil),
    }
}

/// The union of `rects` (client coordinates) as a region in window coordinates, which is what
/// `SetWindowRgn` takes. The caller owns the region.
fn build_region(hwnd: HWND, rects: &[RECT]) -> Option<HRGN> {
    let (dx, dy) = client_offset(hwnd)?;
    // SAFETY: plain GDI object calls. Every region created here is deleted, except the union,
    // which is returned to the caller.
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        if region.is_null() {
            return None;
        }
        for rect in rects {
            let part = CreateRectRgn(
                rect.left + dx,
                rect.top + dy,
                rect.right + dx,
                rect.bottom + dy,
            );
            // A null `part` (out of GDI objects) makes the combination fail too.
            let combined = CombineRgn(region, region, part, RGN_OR);
            DeleteObject(part);
            if combined == RGN_ERROR {
                DeleteObject(region);
                return None;
            }
        }
        Some(region)
    }
}

/// Where the client area starts inside the window. Nastro's window has no frame, so it is
/// `(0, 0)`, but `SetWindowRgn` takes window coordinates and gpui's frame handling may change.
fn client_offset(hwnd: HWND) -> Option<(i32, i32)> {
    let mut window = RECT::default();
    let mut client = POINT::default();
    // SAFETY: `hwnd` is a live window and both pointers are to locals.
    let ok =
        unsafe { GetWindowRect(hwnd, &mut window) != 0 && ClientToScreen(hwnd, &mut client) != 0 };
    ok.then_some((client.x - window.left, client.y - window.top))
}
