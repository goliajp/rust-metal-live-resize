# metal-live-resize

[![Crates.io](https://img.shields.io/crates/v/metal-live-resize?style=flat-square&logo=rust)](https://crates.io/crates/metal-live-resize)
[![docs.rs](https://img.shields.io/docsrs/metal-live-resize?style=flat-square&logo=docs.rs)](https://docs.rs/metal-live-resize)
[![License](https://img.shields.io/crates/l/metal-live-resize?style=flat-square)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue?style=flat-square&logo=rust)](Cargo.toml)
[![Downloads](https://img.shields.io/crates/d/metal-live-resize?style=flat-square)](https://crates.io/crates/metal-live-resize)

[English](README.md) | **简体中文** | [日本語](README.ja.md)

macOS `CAMetalLayer` 窗口实时拖拽缩放无抖动——约 30 行 `objc2` 调用，打包了每个基于 Metal 的 macOS Rust 应用都会踩一次的 bug 的修复方案。

**平台**：仅 macOS。其他平台上 crate 为空，跨平台 workspace 不会编译失败。

> **同义词 / 你可能在搜索**：macOS Metal 缩放抖动、CAMetalLayer wobble、无故障（glitchless）Metal 窗口缩放、缩放拉伸/扭曲、`kCAGravityTopLeft` Metal 修复、Retina `backingScaleFactor` 缩放时错位。

## 问题

macOS 上拖拽窗口边缘时，合成器比应用渲染落后一帧或多帧。默认情况下合成器把上一个 drawable 的内容拉伸去填满新的窗口边界（`contentsGravity = kCAGravityResize`），表现为可见的抖动或变形直到下一帧呈现。

## 修复

对 `CAMetalLayer` 做两次一次性的配置调用 + 每帧一条纪律：

| | 做什么 | 为什么 |
|---|---|---|
| 1 | `contentsGravity = kCAGravityTopLeft` | 旧帧钉在左上角，不被拉伸 |
| 2 | `contentsScale = view.window.backingScaleFactor` | Retina 屏上 drawable 像素与屏幕像素 1:1 对应 |
| 3 | 每帧读 `drawable.texture.width/height` | 避免用缓存的尺寸渲染，缓存尺寸可能跟当前 drawable 不匹配 |

## API

```rust
// 把 layer 挂到 view 之后，一次性配置：
unsafe { metal_live_resize::configure_for_live_resize(layer, view) };

// 或者自己组合原语：
unsafe {
    metal_live_resize::set_contents_gravity_top_left(layer);
    if let Some(scale) = metal_live_resize::view_backing_scale(view) {
        metal_live_resize::set_contents_scale(layer, scale);
    }
}

// 每帧：读 drawable 的真实纹理尺寸，别用缓存的 layer 尺寸
if let Some((w, h)) = unsafe { metal_live_resize::drawable_texture_size(layer) } {
    // 以 (w, h) 渲染
}
```

所有函数接收 `*mut c_void`，可配合 `metal-rs`、`objc2-quartz-core`、原始 FFI 或任何其他 `CAMetalLayer` / `NSView` 提供者使用，不绑死版本。

## 安装

```toml
[target.'cfg(target_os = "macos")'.dependencies]
metal-live-resize = "0.1"
```

## 别用这个方法

有时人们建议 `presentsWithTransaction = true` + `commandBuffer.waitUntilScheduled()`。实测会 **打断帧投递**——AppKit 事件照常触发（点击能命中、状态会更新），但屏幕不再刷新。`contentsGravity + contentsScale` 方案就足够了，不会阻塞事件循环。

## 参考

模式最早由 Tristan Hume 于 2019 年记录：[Glitchless Metal Window Resizing](https://thume.ca/2019/06/19/glitchless-metal-window-resizing/)。

生产参考实现：[`goliajp/tora`](https://github.com/goliajp/tora) —— `crates/tora-gpu/src/surface.rs`。

<!-- ECOSYSTEM BEGIN (synced by claws/opensource/scripts/sync-ecosystem.py — edit ecosystem.toml, not this block) -->

## 生态系统

**metal-live-resize** · [coalesce-worker](https://crates.io/crates/coalesce-worker) · [damage-rects](https://crates.io/crates/damage-rects)

<!-- ECOSYSTEM END -->

## 许可证

MIT —— 见 [LICENSE](LICENSE)。
