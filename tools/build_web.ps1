# Compila o jogo pra navegador e copia pra web/ (pasta publicada pelo Cloudflare).
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot)
cargo build --release --target wasm32-unknown-unknown
$target = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
Copy-Item "$target\wasm32-unknown-unknown\release\urna_mine_verine.wasm" web\urna.wasm -Force
# Opcional (cargo install wasm-opt): ~12% menor
if (Get-Command wasm-opt -ErrorAction SilentlyContinue) {
    wasm-opt -O3 --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int --enable-mutable-globals --enable-multivalue --enable-reference-types web\urna.wasm -o web\urna.wasm
}
if (-not (Test-Path web\mq_js_bundle.js)) {
    $bundle = Get-ChildItem "$env:USERPROFILE\.cargo\registry\src" -Recurse -Filter mq_js_bundle.js | Where-Object FullName -Match "macroquad-0\.4" | Select-Object -First 1
    Copy-Item $bundle.FullName web\mq_js_bundle.js
}
Write-Host "OK: web\urna.wasm pronto"
