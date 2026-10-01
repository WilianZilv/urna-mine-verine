# Fica olhando o código; quando muda, recompila o wasm e publica no Cloudflare.
# Uso: powershell -ExecutionPolicy Bypass -File tools\auto_deploy.ps1
Set-Location (Split-Path $PSScriptRoot)
$watch = @("src", "server", "web\index.html", "web\urna.js", "wrangler.toml")
function Stamp { ($watch | ForEach-Object { Get-ChildItem $_ -Recurse -File } | Measure-Object LastWriteTime -Maximum).Maximum }
$last = Stamp
Write-Host "auto-deploy: olhando $($watch -join ', ')"
while ($true) {
    Start-Sleep 3
    $now = Stamp
    if ($now -eq $last) { continue }
    Start-Sleep 4
    if ((Stamp) -ne $now) { continue }
    $last = Stamp
    Write-Host "$(Get-Date -Format HH:mm:ss) mudou, compilando..."
    cmd /c "cargo build --release --target wasm32-unknown-unknown 2>&1" | Select-String "^error" -Context 0, 6
    if ($LASTEXITCODE -ne 0) { Write-Host "erro de compilacao, nao publiquei"; continue }
    $target = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
    Copy-Item "$target\wasm32-unknown-unknown\release\urna_mine_verine.wasm" web\urna.wasm -Force
    cmd /c "npx -y wrangler@latest deploy 2>&1" | Select-String "workers.dev|ERROR"
    Write-Host "$(Get-Date -Format HH:mm:ss) publicado"
}
