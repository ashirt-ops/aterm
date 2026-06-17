# CI workflows

The repository has a single GitHub Actions workflow for the Rust crate.

| File      | Name      | Language | Triggers                        | Purpose |
|-----------|-----------|----------|---------------------------------|---------|
| `ci.yml`  | `Rust CI` | Rust     | every push, every PR, tags `v*` | build/test/clippy (`-D warnings`)/fmt across Linux/macOS/Windows, a `--features playback` build to keep the optional `avt` path compiling, and a tag-triggered job that builds optimized release binaries for each OS and uploads them as workflow artifacts. |

The legacy Go workflow (`ci.yaml`) was removed once the Rust rewrite replaced
the Go implementation.
