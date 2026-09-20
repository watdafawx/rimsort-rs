set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default:
    just --list

# Launch app with hot reload
dev:
    cargo tauri dev

# Debug bundle
build:
    cargo tauri build --debug

test:
    cargo test --workspace
    npm --prefix ui test

lint:
    cargo clippy --workspace --all-targets -- -D warnings
    npm --prefix ui run check
    npm --prefix ui run lint

fmt:
    cargo fmt --all
    npm --prefix ui run fmt

# Regenerate ui/src/bindings.ts
bindings:
    $env:UPDATE_BINDINGS="1"; cargo test -p rimsort-rs bindings_up_to_date


# Screenshot running app -> debug/screenshots/
shot name="":
    powershell -NoProfile -File scripts/shot.ps1 {{name}}

# Dev run against a COPY of ModsConfig.xml (debug/testconfig) so saves can't touch the real one
dev-safe:
    $env:RIMSORT_RS_DATA_DIR = "$PWD/debug/appdata"; cargo tauri dev

# dev-safe + WebView2 DevTools port 9222 so tools can drive the UI (scripts/cdp.mjs)
dev-debug:
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"; $env:RIMSORT_RS_DATA_DIR = "$PWD/debug/appdata"; cargo tauri dev

# One-time: Python venv with RimSort's deps for the golden sort comparison
golden-setup:
    uv venv debug/pyvenv
    uv pip install --python debug/pyvenv/Scripts/python.exe loguru lxml msgspec networkx toposort xmltodict pygit2 natsort platformdirs

# Sort your real install with RimSort's Python code and ours; fails if the orders differ
golden:
    $env:RIMSORT_RS_DATA_DIR = "$PWD/debug/testdata"; $env:RUST_SORT_OUT = "$PWD/debug/rust_sorted.json"; cargo test -p rimsort-core --test real_machine -- --ignored
    debug/pyvenv/Scripts/python.exe tests/golden/py_sort.py "$env:LOCALAPPDATA/RimSort/settings.json" debug/py_sorted.json
    node tests/golden/compare.mjs debug/py_sorted.json debug/rust_sorted.json

# Synthetic 5,000-mod benchmark (release): scan, lists, validate, sort
bench:
    node scripts/gen_mods.mjs 5000 debug/synth
    $env:SYNTH_DIR = "$PWD/debug/synth"; $env:RIMSORT_RS_DATA_DIR = "$PWD/debug/synthdata"; cargo test -p rimsort-core --release --test synthetic -- --ignored --nocapture
