# metal-live-resize

[![Crates.io](https://img.shields.io/crates/v/metal-live-resize?style=flat-square&logo=rust)](https://crates.io/crates/metal-live-resize)
[![docs.rs](https://img.shields.io/docsrs/metal-live-resize?style=flat-square&logo=docs.rs)](https://docs.rs/metal-live-resize)
[![License](https://img.shields.io/crates/l/metal-live-resize?style=flat-square)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue?style=flat-square&logo=rust)](Cargo.toml)
[![Downloads](https://img.shields.io/crates/d/metal-live-resize?style=flat-square)](https://crates.io/crates/metal-live-resize)

[English](README.md) | [简体中文](README.zh-CN.md) | **日本語**

macOS の `CAMetalLayer` ウィンドウライブリサイズをちらつきなく実現——Metal ベースの macOS Rust アプリが毎回再発見するバグ修正を、約 30 行の `objc2` 呼び出しにまとめました。

**プラットフォーム**: macOS のみ。他のプラットフォームでは crate は空になるので、クロスプラットフォームな workspace でもビルドが壊れません。

> **同義語 / 検索ワード**: macOS Metal リサイズちらつき、CAMetalLayer wobble、グリッチレス（glitchless）Metal ウィンドウリサイズ、リサイズ時の引き伸ばし/歪み、`kCAGravityTopLeft` Metal 修正、Retina `backingScaleFactor` リサイズ中のずれ。

## 問題

macOS でウィンドウ端をドラッグしてリサイズしている間、コンポジタはアプリのレンダリングより 1 フレーム以上遅れる。既定では、コンポジタは直前の drawable の内容を新しいウィンドウ境界に引き伸ばして埋める（`contentsGravity = kCAGravityResize`）ため、次のフレームが提示されるまで目に見える揺れや歪みが発生する。

## 修正

`CAMetalLayer` に対する 2 つの 1 回限りの設定呼び出し + 1 フレームごとの作法：

| | 何を | なぜ |
|---|---|---|
| 1 | `contentsGravity = kCAGravityTopLeft` | 古いフレームを拡大縮小せず、左上にピン留め |
| 2 | `contentsScale = view.window.backingScaleFactor` | Retina ディスプレイで drawable ピクセルを画面ピクセルに 1:1 でマップ |
| 3 | フレームごとに `drawable.texture.width/height` を読む | キャッシュされたレイヤのサイズを使わない（現在の drawable と一致しない可能性がある） |

## API

```rust
// layer を view に取り付けた後、1 回だけ設定：
unsafe { metal_live_resize::configure_for_live_resize(layer, view) };

// あるいはプリミティブを自分で組み合わせる：
unsafe {
    metal_live_resize::set_contents_gravity_top_left(layer);
    if let Some(scale) = metal_live_resize::view_backing_scale(view) {
        metal_live_resize::set_contents_scale(layer, scale);
    }
}

// フレームごと: drawable の実テクスチャサイズを読む、キャッシュされた layer サイズは使わない
if let Some((w, h)) = unsafe { metal_live_resize::drawable_texture_size(layer) } {
    // (w, h) でレンダリング
}
```

すべての関数は `*mut c_void` を受け取り、`metal-rs` / `objc2-quartz-core` / 生 FFI / その他の `CAMetalLayer` / `NSView` プロバイダと組み合わせ可能でバージョン固定を強制しません。

## インストール

```toml
[target.'cfg(target_os = "macos")'.dependencies]
metal-live-resize = "0.1"
```

## やってはいけないこと

`presentsWithTransaction = true` + `commandBuffer.waitUntilScheduled()` が代替案として勧められることがあります。実測では **フレーム配信を壊します**——AppKit イベントは発火し（ヒットテストは通り、状態も更新される）、画面だけ更新されなくなります。`contentsGravity + contentsScale` のみで十分で、イベントループをブロックしません。

## 参考

このパターンは Tristan Hume が 2019 年に最初に文書化: [Glitchless Metal Window Resizing](https://thume.ca/2019/06/19/glitchless-metal-window-resizing/)。

プロダクション実装: [`goliajp/tora`](https://github.com/goliajp/tora) —— `crates/tora-gpu/src/surface.rs`。

<!-- ECOSYSTEM BEGIN (synced by claws/opensource/scripts/sync-ecosystem.py — edit ecosystem.toml, not this block) -->

## エコシステム

GOLIA の Rust GUI/GPU インフラ系列の一員 — 実プロダクションのインシデントから抽出した narrow な crate、各々独立にバージョン管理:

| Crate / Package | リポジトリ | 説明 |
|---|---|---|
| **metal-live-resize**（本 crate） | [rust-metal-live-resize](https://github.com/goliajp/rust-metal-live-resize) | macOS Metal ウィンドウのちらつきなしリサイズ（CAMetalLayer contentsGravity + contentsScale） |
| [coalesce-worker](https://crates.io/crates/coalesce-worker) | [rust-coalesce-worker](https://github.com/goliajp/rust-coalesce-worker) | コアレッシング worker + 世代カウンタで古い結果を破棄 |
| [damage-rects](https://crates.io/crates/damage-rects) | [rust-damage-rects](https://github.com/goliajp/rust-damage-rects) | ダーティ矩形の累積・合成・出力で部分 GPU 再描画を支援 |

<!-- ECOSYSTEM END -->

## ライセンス

MIT —— [LICENSE](LICENSE) を参照。
