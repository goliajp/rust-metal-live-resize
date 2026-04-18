# Changelog

All notable changes to this crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-18

### Added
- Initial release extracted from [goliajp/tora](https://github.com/goliajp/tora).
- `configure_for_live_resize(layer, view)` one-call setup.
- Primitives: `set_contents_gravity_top_left`, `set_contents_scale`,
  `view_backing_scale`, `drawable_texture_size`.
- `resize_demo` winit+softbuffer example reproducing and fixing the wobble.
