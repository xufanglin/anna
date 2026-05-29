# 支持的平台与 target triple

`anna` 在以下 6 个 target triple 上构建并通过 CI：

| 操作系统 | x86_64                          | aarch64                         |
|----------|---------------------------------|---------------------------------|
| Linux    | `x86_64-unknown-linux-gnu`      | `aarch64-unknown-linux-gnu`     |
| macOS    | `x86_64-apple-darwin`           | `aarch64-apple-darwin`          |
| Windows  | `x86_64-pc-windows-msvc`        | `aarch64-pc-windows-msvc`       |

本机交叉编译可用 [`cross`](https://github.com/cross-rs/cross) 简化：

```bash
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

CI 在 GitHub Actions 上为这 6 个 triple 各跑一次 `cargo build --workspace` 与 `cargo test --workspace`；release tag 推送时同时为它们打包预编译二进制。
