//! Glitch-free macOS `CAMetalLayer` window live resize.
//!
//! Provides the minimum configuration primitives to prevent content
//! distortion during window resize on macOS Metal apps.
//!
//! # The problem
//!
//! During live window resize on macOS, the compositor lags behind the
//! app's rendering by one or more frames. By default, the compositor
//! stretches the previous drawable contents to fill the new window
//! bounds (`contentsGravity = kCAGravityResize`), which shows up as
//! visible wobble or distortion until the next frame is presented.
//!
//! # The fix
//!
//! Two one-time configuration calls on the `CAMetalLayer`:
//!
//! 1. **`contentsGravity = kCAGravityTopLeft`** — pins stale frames to
//!    the top-left corner instead of scaling them, so the "wrong" area
//!    is clipped rather than stretched.
//! 2. **`contentsScale = NSWindow.backingScaleFactor`** — ensures
//!    drawable pixels map 1:1 to screen pixels on Retina displays,
//!    without which topLeft gravity puts content at the wrong position.
//!
//! And one per-frame discipline:
//!
//! 3. **Read drawable texture dimensions at render time**, never cached
//!    layer `width`/`height`. During resize, the drawable's texture
//!    may not yet match the cached size and rendering at the wrong
//!    size causes visible distortion.
//!
//! # What NOT to do
//!
//! The `presentsWithTransaction = true` + `commandBuffer.waitUntilScheduled()`
//! approach is sometimes suggested as an alternative. It has been tested
//! and **breaks frame delivery** — AppKit events fire (hit tests pass,
//! state updates) but nothing renders to the screen. The
//! `contentsGravity + contentsScale` approach alone is sufficient and
//! does not block the event loop.
//!
//! # Reference
//!
//! Pattern first documented by Tristan Hume, 2019:
//! <https://thume.ca/2019/06/19/glitchless-metal-window-resizing/>
//!
//! # Example
//!
//! ```no_run
//! use metal_live_resize::configure_for_live_resize;
//! use core::ffi::c_void;
//!
//! # unsafe fn example(layer: *mut c_void, view: *mut c_void) {
//! // after attaching your CAMetalLayer to an NSView:
//! unsafe { configure_for_live_resize(layer, view) };
//!
//! // per-frame, read the actual drawable size rather than cached w/h:
//! if let Some((w, h)) = unsafe { metal_live_resize::drawable_texture_size(layer) } {
//!     // render at (w, h)
//! }
//! # }
//! ```

#![cfg(target_os = "macos")]
#![deny(missing_docs)]

use core::ffi::{c_void, CStr};
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

/// Apply both live-resize fixes: sets `contentsGravity = kCAGravityTopLeft`
/// and `contentsScale = view.window.backingScaleFactor` on the layer.
///
/// Convenience wrapper around [`set_contents_gravity_top_left`] and
/// [`set_contents_scale`] with scale read via [`view_backing_scale`].
/// If the view has no attached window yet, only the gravity is set.
///
/// Call once after attaching the `CAMetalLayer` to the `NSView`.
///
/// # Safety
/// - `layer` must be a valid pointer to a `CAMetalLayer`.
/// - `view` must be a valid pointer to an `NSView` that owns the layer.
/// - Must be called on the main thread (AppKit requirement).
pub unsafe fn configure_for_live_resize(layer: *mut c_void, view: *mut c_void) {
    // SAFETY: invariants delegated to the individual calls; see function docs.
    unsafe {
        set_contents_gravity_top_left(layer);
        if let Some(scale) = view_backing_scale(view) {
            set_contents_scale(layer, scale);
        }
    }
}

/// Sets `contentsGravity = kCAGravityTopLeft` on the layer.
///
/// Prevents the compositor from scaling old drawable contents during
/// live resize. Without this, each resize tick visibly stretches the
/// previous frame until the next `nextDrawable` completes.
///
/// # Safety
/// - `layer` must be a valid pointer to a `CALayer` (or subclass such
///   as `CAMetalLayer`).
/// - Must be called on the main thread.
pub unsafe fn set_contents_gravity_top_left(layer: *mut c_void) {
    // SAFETY: caller guarantees `layer` is a valid CALayer pointer and
    // is accessed from the main thread. NSString creation via
    // `stringWithUTF8String:` returns an autoreleased NSString; the
    // layer retains its `contentsGravity` value internally.
    unsafe {
        let layer_obj = layer as *mut AnyObject;
        let s: *const CStr = c"topLeft";
        let nsstring: *const AnyObject =
            msg_send![class!(NSString), stringWithUTF8String: s.cast::<core::ffi::c_char>()];
        let _: () = msg_send![layer_obj, setContentsGravity: nsstring];
    }
}

/// Sets `contentsScale` on the layer.
///
/// Pass the window's `backingScaleFactor` (2.0 on standard Retina
/// displays, 1.0 on 1x displays, 3.0 on iPhone-class displays).
/// Without this, drawable pixels are mismatched from screen pixels
/// and top-left gravity puts content at a scaled position.
///
/// # Safety
/// - `layer` must be a valid pointer to a `CALayer` (or subclass).
/// - Must be called on the main thread.
pub unsafe fn set_contents_scale(layer: *mut c_void, scale: f64) {
    // SAFETY: caller guarantees `layer` is a valid CALayer pointer.
    // `setContentsScale:` takes a CGFloat (f64 on 64-bit macOS).
    unsafe {
        let layer_obj = layer as *mut AnyObject;
        let _: () = msg_send![layer_obj, setContentsScale: scale];
    }
}

/// Reads the `backingScaleFactor` from an `NSView`'s window.
///
/// Returns `None` if the view is detached from any window (i.e.
/// `[view window]` returns `nil`). In that case fall back to 1.0 or
/// query the main screen directly.
///
/// # Safety
/// - `view` must be a valid pointer to an `NSView`.
/// - Must be called on the main thread.
pub unsafe fn view_backing_scale(view: *mut c_void) -> Option<f64> {
    // SAFETY: caller guarantees `view` is a valid NSView pointer.
    // `[view window]` returns nullable; we null-check before messaging.
    unsafe {
        let view_obj = view as *mut AnyObject;
        let window: *mut AnyObject = msg_send![view_obj, window];
        if window.is_null() {
            return None;
        }
        let scale: f64 = msg_send![window, backingScaleFactor];
        Some(scale)
    }
}

/// Gets the actual pixel dimensions of the drawable's current texture.
///
/// Obtains `nextDrawable` from the layer and reads the `width`/`height`
/// of its bound texture. During live resize, the drawable may not yet
/// match the window's reported size, and rendering at the cached size
/// causes visible distortion.
///
/// Returns `None` if `nextDrawable` returns `nil` (drawable pool
/// exhausted — caller should skip the frame).
///
/// **This consumes the next drawable from the layer's pool.** If you
/// need the drawable itself for rendering, obtain it via `nextDrawable`
/// yourself and read `drawable.texture.width/height` directly.
///
/// # Safety
/// - `layer` must be a valid pointer to a `CAMetalLayer` with a Metal
///   device attached.
/// - Must be called on the main thread.
pub unsafe fn drawable_texture_size(layer: *mut c_void) -> Option<(u32, u32)> {
    // SAFETY: caller guarantees `layer` is a valid CAMetalLayer.
    // `nextDrawable` returns an autoreleased `id<CAMetalDrawable>`; we
    // null-check before accessing its texture.
    unsafe {
        let layer_obj = layer as *mut AnyObject;
        let drawable: *mut AnyObject = msg_send![layer_obj, nextDrawable];
        if drawable.is_null() {
            return None;
        }
        let texture: *mut AnyObject = msg_send![drawable, texture];
        if texture.is_null() {
            return None;
        }
        let w: u64 = msg_send![texture, width];
        let h: u64 = msg_send![texture, height];
        Some((w as u32, h as u32))
    }
}
