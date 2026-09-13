$ErrorActionPreference = "Stop"
$env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot "src-tauri\target\phase-3-5-2a-e2e"
& "C:\Program Files\nodejs\npx.cmd" tauri dev `
  --config src-tauri/tauri.windows.conf.json `
  --config tauri.phase-3-5-2a-acceptance.conf.json
