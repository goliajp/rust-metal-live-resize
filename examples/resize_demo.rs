//! Visual demo for the `metal-live-resize` fix.
//!
//! Opens a window with a `CAMetalLayer` that clears to a color
//! oscillating over time. Press `F` to toggle the live-resize fix.
//!
//! **Drag a window edge continuously** to observe the difference:
//!
//! - **Fix on**  — color transitions are clean; on a rapid drag, the
//!   uncovered area appears as a solid edge pinned to the top-left.
//! - **Fix off** — the compositor stretches the previous drawable
//!   to fill the new size mid-drag, producing visible wobble or
//!   blur until the next frame is presented.
//!
//! Run: `cargo run --example resize_demo -p metal-live-resize`.

#![cfg(target_os = "macos")]

use core::ffi::c_void;
use objc2::encode::{Encode, Encoding};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_quartz_core::CAMetalLayer;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::Key;
use winit::window::{Window, WindowId};

// MTLCreateSystemDefaultDevice is a free function in the Metal framework.
#[link(name = "Metal", kind = "framework")]
unsafe extern "C" {
    fn MTLCreateSystemDefaultDevice() -> *mut AnyObject;
}

const MTL_PIXEL_FORMAT_BGRA8_UNORM: u64 = 80;
const MTL_LOAD_ACTION_CLEAR: u64 = 2;
const MTL_STORE_ACTION_STORE: u64 = 1;

#[repr(C)]
struct CgSize {
    w: f64,
    h: f64,
}

// SAFETY: matches Objective-C's CGSize layout.
unsafe impl Encode for CgSize {
    const ENCODING: Encoding = Encoding::Struct(
        "CGSize",
        &[<f64 as Encode>::ENCODING, <f64 as Encode>::ENCODING],
    );
}

#[repr(C)]
struct ClearColor {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

// SAFETY: matches Objective-C's MTLClearColor layout.
unsafe impl Encode for ClearColor {
    const ENCODING: Encoding = Encoding::Struct(
        "MTLClearColor",
        &[
            <f64 as Encode>::ENCODING,
            <f64 as Encode>::ENCODING,
            <f64 as Encode>::ENCODING,
            <f64 as Encode>::ENCODING,
        ],
    );
}

struct App {
    window: Option<Window>,
    layer: Option<Retained<CAMetalLayer>>,
    queue: *mut AnyObject,
    ns_view: *mut c_void,
    fix_on: bool,
    start: Instant,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            layer: None,
            queue: core::ptr::null_mut(),
            ns_view: core::ptr::null_mut(),
            fix_on: true,
            start: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("metal-live-resize — F: toggle fix, Q: quit"),
            )
            .expect("create window");

        let handle = window.window_handle().expect("window handle").as_raw();
        let RawWindowHandle::AppKit(appkit) = handle else {
            panic!("macOS only");
        };
        let ns_view = appkit.ns_view.as_ptr();

        // Set up Metal device, queue, layer.
        let (layer, queue) = unsafe { setup_metal(ns_view) };

        // Apply the fix by default.
        unsafe {
            let layer_ptr = Retained::as_ptr(&layer) as *mut c_void;
            metal_live_resize::configure_for_live_resize(layer_ptr, ns_view);
        }

        // Initial drawable size.
        let size = window.inner_size();
        unsafe { set_drawable_size(&layer, size.width, size.height) };

        self.ns_view = ns_view;
        self.layer = Some(layer);
        self.queue = queue;
        self.window = Some(window);

        println!("fix: ON  — drag window edges to observe; press F to toggle.");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let Some(layer) = &self.layer {
                    unsafe { set_drawable_size(layer, size.width, size.height) };
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match c.as_str() {
                "f" | "F" => self.toggle_fix(),
                "q" | "Q" => event_loop.exit(),
                _ => {}
            },

            WindowEvent::RedrawRequested => {
                self.render();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            _ => {}
        }
    }
}

impl App {
    fn toggle_fix(&mut self) {
        let Some(layer) = &self.layer else { return };
        self.fix_on = !self.fix_on;

        let layer_ptr = Retained::as_ptr(layer) as *mut c_void;
        unsafe {
            if self.fix_on {
                metal_live_resize::configure_for_live_resize(layer_ptr, self.ns_view);
                println!("fix: ON  — clean frames during resize.");
            } else {
                // Restore default kCAGravityResize so the compositor stretches
                // stale content during resize. This is what we're fixing.
                set_gravity_string(layer_ptr, c"resize".as_ptr());
                println!("fix: OFF — drag an edge rapidly; watch it stretch / wobble.");
            }
        }
    }

    fn render(&self) {
        let (Some(layer), false) = (&self.layer, self.queue.is_null()) else {
            return;
        };

        // Animated clear color — oscillates so a "frozen" frame (stretched
        // by the compositor) visually differs from a correctly-presented one.
        let t = self.start.elapsed().as_secs_f64();
        let r = 0.5 + 0.5 * (t * 0.9).sin();
        let g = 0.5 + 0.5 * (t * 1.3).sin();
        let b = 0.5 + 0.5 * (t * 1.7).sin();

        unsafe { clear_frame(layer, self.queue, r, g, b) };
    }
}

// === Metal helpers via raw msg_send ===

unsafe fn setup_metal(ns_view: *mut c_void) -> (Retained<CAMetalLayer>, *mut AnyObject) {
    let device = unsafe { MTLCreateSystemDefaultDevice() };
    assert!(!device.is_null(), "No Metal device available");
    let queue: *mut AnyObject = unsafe { msg_send![device, newCommandQueue] };

    let layer = CAMetalLayer::new();
    let layer_ptr = Retained::as_ptr(&layer) as *mut AnyObject;

    unsafe {
        let _: () = msg_send![layer_ptr, setDevice: device];
        let _: () = msg_send![layer_ptr, setPixelFormat: MTL_PIXEL_FORMAT_BGRA8_UNORM];
        let _: () = msg_send![layer_ptr, setFramebufferOnly: true];

        let ns_view_obj = ns_view as *mut AnyObject;
        let _: () = msg_send![ns_view_obj, setWantsLayer: true];
        let _: () = msg_send![ns_view_obj, setLayer: layer_ptr];
    }

    (layer, queue)
}

unsafe fn set_drawable_size(layer: &Retained<CAMetalLayer>, width: u32, height: u32) {
    let layer_ptr = Retained::as_ptr(layer) as *mut AnyObject;
    let size = CgSize {
        w: width as f64,
        h: height as f64,
    };
    unsafe {
        let _: () = msg_send![layer_ptr, setDrawableSize: size];
    }
}

unsafe fn set_gravity_string(layer: *mut c_void, utf8: *const core::ffi::c_char) {
    unsafe {
        let layer_obj = layer as *mut AnyObject;
        let nsstring: *const AnyObject = msg_send![
            class!(NSString),
            stringWithUTF8String: utf8
        ];
        let _: () = msg_send![layer_obj, setContentsGravity: nsstring];
    }
}

unsafe fn clear_frame(
    layer: &Retained<CAMetalLayer>,
    queue: *mut AnyObject,
    r: f64,
    g: f64,
    b: f64,
) {
    let layer_ptr = Retained::as_ptr(layer) as *mut AnyObject;
    unsafe {
        let drawable: *mut AnyObject = msg_send![layer_ptr, nextDrawable];
        if drawable.is_null() {
            return;
        }
        let texture: *mut AnyObject = msg_send![drawable, texture];

        let rpd: *mut AnyObject = msg_send![class!(MTLRenderPassDescriptor), new];
        let attachments: *mut AnyObject = msg_send![rpd, colorAttachments];
        let attachment: *mut AnyObject = msg_send![attachments, objectAtIndexedSubscript: 0_u64];
        let _: () = msg_send![attachment, setTexture: texture];
        let _: () = msg_send![attachment, setLoadAction: MTL_LOAD_ACTION_CLEAR];
        let _: () = msg_send![attachment, setStoreAction: MTL_STORE_ACTION_STORE];

        let clear = ClearColor { r, g, b, a: 1.0 };
        let _: () = msg_send![attachment, setClearColor: clear];

        let cmdbuf: *mut AnyObject = msg_send![queue, commandBuffer];
        let encoder: *mut AnyObject = msg_send![cmdbuf, renderCommandEncoderWithDescriptor: rpd];
        let _: () = msg_send![encoder, endEncoding];
        let _: () = msg_send![cmdbuf, presentDrawable: drawable];
        let _: () = msg_send![cmdbuf, commit];

        // Release the autoreleased/+new'd descriptor we retained via `new`.
        let _: () = msg_send![rpd, release];
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("run app");
}
