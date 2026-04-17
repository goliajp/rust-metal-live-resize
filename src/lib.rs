//! # metal-live-resize
//!
//! Glitch-free macOS `CAMetalLayer` window live resize.
//!
//! **Status:** placeholder — extraction from [goliajp/tora](https://github.com/goliajp/tora)
//! (`crates/tora-gpu/src/surface.rs`) pending.
//!
//! The pattern: set `contentsGravity = kCAGravityTopLeft` and
//! `contentsScale = backingScaleFactor`, read drawable dimensions at render
//! time (never cache), and do **not** use `presentsWithTransaction`
//! + `waitUntilScheduled`.
