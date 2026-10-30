# octo-lib

A deterministic pixel art engine in Rust. One codebase for the browser (WebAssembly), Python, native apps and the `octo` command-line tool.

Status: before the first release. The design document, roadmap and decision log are in [docs.md](docs.md).

## Crates

| Crate | What it holds |
| --- | --- |
| `octobit-core` | document, pixel buffers, palettes, color, history, files |

Other crates join the workspace when they have real code.

## Install

Nothing is published yet. Install instructions come with the first release of `octobit-core` on crates.io.

## Build

```
cargo build
cargo test
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
