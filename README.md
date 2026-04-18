# metal-live-resize

Glitch-free macOS `CAMetalLayer` window live resize — ~30 lines of `objc2`
calls packaging a fix that every Metal-based macOS Rust app rediscovers.

**Platform:** macOS only. The crate is empty on other platforms and will
not cause build failures in cross-platform workspaces.

> **Also known as / if you're searching for:** macOS Metal resize
> flicker, CAMetalLayer wobble, glitchless Metal window resize, live
> resize stretch/distortion, `kCAGravityTopLeft` Metal fix, Retina
> `backingScaleFactor` mismatch during resize.

## The problem

During live window resize on macOS, the compositor lags behind the app's
rendering by one or more frames. The default behaviour is to stretch the
previous drawable contents to fill the new window bounds
(`contentsGravity = kCAGravityResize`), which shows up as visible wobble
or distortion until the next frame is presented.

## The fix

Two one-time configuration calls + one per-frame discipline:

| | What | Why |
|---|---|---|
| 1 | `contentsGravity = kCAGravityTopLeft` | Pin stale frames to top-left instead of scaling them |
| 2 | `contentsScale = view.window.backingScaleFactor` | Map drawable pixels 1:1 to screen pixels on Retina |
| 3 | Read `drawable.texture.width/height` per frame | Avoid rendering at a cached size that may not match the current drawable |

## API

```rust
// one-time setup after attaching the layer to the view:
unsafe { metal_live_resize::configure_for_live_resize(layer, view) };

// or compose primitives yourself:
unsafe {
    metal_live_resize::set_contents_gravity_top_left(layer);
    if let Some(scale) = metal_live_resize::view_backing_scale(view) {
        metal_live_resize::set_contents_scale(layer, scale);
    }
}

// per-frame: read the drawable's real texture size, not cached layer w/h
if let Some((w, h)) = unsafe { metal_live_resize::drawable_texture_size(layer) } {
    // render at (w, h)
}
```

All functions take `*mut c_void` so you can use them with `metal-rs`,
`objc2-quartz-core`, raw FFI, or any other `CAMetalLayer`/`NSView`
provider without version-locking.

## Install

```toml
[target.'cfg(target_os = "macos")'.dependencies]
metal-live-resize = "0.1"
```

## What NOT to do

The `presentsWithTransaction = true` + `commandBuffer.waitUntilScheduled()`
approach is sometimes suggested as an alternative. It has been tested
and **breaks frame delivery** — AppKit events fire (hit tests pass,
state updates) but nothing renders to the screen. The
`contentsGravity + contentsScale` approach alone is sufficient and does
not block the event loop.

## Reference

Pattern first documented by Tristan Hume, 2019:
[Glitchless Metal Window Resizing](https://thume.ca/2019/06/19/glitchless-metal-window-resizing/).

Production reference implementation:
[`goliajp/tora`](https://github.com/goliajp/tora) — `crates/tora-gpu/src/surface.rs`.

## License

MIT — see [LICENSE](LICENSE).
