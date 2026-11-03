# Contributing to tpt-fonts

## License headers

Every source file carries an SPDX header as its first lines. Use the comment
syntax of the file type:

Rust, TypeScript, JavaScript:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
```

TOML, YAML, shell, and other `#` comment files:

```toml
# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 TPT Solutions
```

HTML:

```html
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
<!-- Copyright (c) 2026 TPT Solutions -->
```

Do not add headers to generated files, lockfiles, or third-party code. Update the
copyright year when you substantially rewrite a file.

## Checks before opening a pull request

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run build
```
