# CI workflows

During the Rust rewrite the repository carries **two** GitHub Actions
workflows. Both are active and are expected to coexist until the rewrite
becomes the shipping artifact on `main`.

| File       | Name     | Language | Triggers                          | Purpose |
|------------|----------|----------|-----------------------------------|---------|
| `ci.yaml`  | `ci`     | Go       | push/PR to `main`, tags `v*`      | Legacy: builds/tests/lints the original Go sources, signs+notarizes the macOS binary, and publishes GitHub Release zips for the Go binaries. **Do not delete yet** — it is what currently ships from `main`. |
| `ci.yml`   | `Rust CI`| Rust     | every push, every PR, tags `v*`   | Rewrite: build/test/clippy (`-D warnings`)/fmt across Linux/macOS/Windows, a `--features playback` build to keep the optional `avt` path compiling, and a tag-triggered job that builds optimized release binaries for each OS and uploads them as workflow artifacts. |

## Why both

The rewrite lives on the `rust-rewrite` branch. The Go workflow targets `main`
only, so it does not run on rewrite branches/PRs; the Rust workflow runs on all
pushes and PRs. They therefore do not collide on day-to-day rewrite work. On a
`v*` tag both can fire — the Go workflow produces the Go release, the Rust
workflow uploads the Rust binaries as artifacts.

## Migration plan

When the Rust rewrite replaces the Go binary on `main`:

1. Move the release/publish responsibilities (GitHub Release creation, asset
   upload, macOS signing/notarization) from `ci.yaml` into `ci.yml`.
2. Delete `ci.yaml` and the Go sources.
3. Rename `ci.yml` → `ci.yaml` (or keep, per preference).
