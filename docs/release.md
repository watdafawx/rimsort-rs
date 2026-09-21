# Releasing

1. Make sure `main` is green (CI: rustfmt, clippy `-D warnings`, `cargo test` incl. the bindings-freshness test,
   `svelte-check`, ESLint, Prettier on Windows + Linux).
2. Bump `version` in `Cargo.toml` (workspace), `src-tauri/tauri.conf.json`, and `ui/package.json`.
3. Tag and push: `git tag v0.x.y && git push origin v0.x.y`. The **Release** workflow builds:
   Windows (MSI + NSIS), macOS (arm64 + x64 `.dmg`), Linux (AppImage, `.deb`, `.rpm`) and attaches them to a
   **draft** GitHub release. Review, then publish.

## Local build

`just build` (debug bundle) or `cargo tauri build` (release bundle) — needs WebView2 on Windows and the WebKitGTK
packages listed in `.github/workflows/ci.yml` on Linux.

## Signing (not configured yet)

- **Windows**: an Authenticode certificate is needed to avoid SmartScreen warnings. Set the certificate via
  `bundle.windows.certificateThumbprint` / a signing command in `tauri.conf.json`.
- **macOS**: Developer ID certificate + notarization. Provide `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` as repository secrets (see comments in `release.yml`).
- **Updater** (`tauri-plugin-updater`, not added yet): generate a key with `cargo tauri signer generate`, store the
  private key in `TAURI_SIGNING_PRIVATE_KEY`, and publish the public key in `tauri.conf.json`.

## Untested

`release.yml` and `ci.yml` have not run yet (no remote). First run may need small fixes (runner images, macOS
target setup).

## Verified locally (Windows)

`cargo tauri build --no-bundle` (release exe ≈ 20 MB, ~5 min cold) and `cargo tauri build --bundles nsis`
(installer ≈ 5.4 MB) both succeed. Release builds are warning-free (`cargo clippy --release -p rimsort-rs`).
