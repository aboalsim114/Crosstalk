# Validation locale complète de Crosstalk sur Windows (remplace la CI GitHub Actions).
#
#   powershell -ExecutionPolicy Bypass -File scripts\validate.ps1               # tout
#   powershell -ExecutionPolicy Bypass -File scripts\validate.ps1 -SkipBundle   # sans installateurs
#   powershell -ExecutionPolicy Bypass -File scripts\validate.ps1 -Release      # + contrôles de release
#
# Étapes : garde anti-triche, typecheck, tests UI, build UI, rustfmt, clippy,
# tests Rust, installateurs NSIS + MSI (MSVC), crosstalk-audio-probe,
# crosstalk-asr-bench et crosstalk-translate. Résultat : dist-local\<commit>\ avec les exécutables,
# SHA256SUMS.txt et validation.md, le rapport à coller dans la Pull Request.
# S'arrête à la première étape en échec. Ne mesure ni Whisper en partie, ni CS2,
# ni les FPS : ces tests restent manuels (docs/testing/).
[CmdletBinding()]
param(
    [switch]$SkipBundle,
    [switch]$Release,
    [string]$Out = 'dist-local'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$target = 'x86_64-pc-windows-msvc'

function Fail([string]$msg) { Write-Host "ÉCHEC : $msg" -ForegroundColor Red; exit 1 }

# --- Préalables -------------------------------------------------------------
if ($env:OS -ne 'Windows_NT') { Fail 'à lancer sous Windows (build MSVC).' }
if (-not $env:LIBCLANG_PATH -and (Test-Path 'C:\Program Files\LLVM\bin\libclang.dll')) {
    $env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
}
if (-not $env:LIBCLANG_PATH) { Fail 'LLVM introuvable : installez-le ou définissez LIBCLANG_PATH (bindgen de whisper-rs).' }
$link = Get-Command link.exe -ErrorAction SilentlyContinue
if ($link -and $link.Source -match '\\Git\\') {
    Fail "link.exe de Git ($($link.Source)) masque celui de MSVC : retirez Git\usr\bin du PATH ou utilisez un Developer PowerShell."
}
foreach ($tool in 'git', 'node', 'npm', 'cargo', 'rustc', 'cmake') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Fail "$tool introuvable dans le PATH." }
}

$commit = (git rev-parse HEAD).Trim()
$short = $commit.Substring(0, 7)
$branch = (git rev-parse --abbrev-ref HEAD).Trim()
$dirty = [bool](git status --porcelain)

$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$machine = [ordered]@{
    'Windows'     = "$($os.Caption) build $($os.BuildNumber)"
    'Processeur'  = "$($cpu.Name.Trim()) ($($cpu.NumberOfCores) cœurs / $($cpu.NumberOfLogicalProcessors) threads)"
    'RAM'         = '{0:N1} Go' -f ($os.TotalVisibleMemorySize / 1MB)
    'Node / npm'  = "$(node --version) / $(npm --version)"
    'Rust'        = (rustc --version)
    'CMake'       = ((cmake --version) | Select-Object -First 1)
    'LIBCLANG'    = $env:LIBCLANG_PATH
}

$results = New-Object System.Collections.Generic.List[object]
function Step([string]$name, [string]$dir, [scriptblock]$run) {
    Write-Host "`n==> $name" -ForegroundColor Cyan
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Push-Location $dir
    try { & $run; $ok = ($LASTEXITCODE -eq 0 -or $null -eq $LASTEXITCODE) }
    catch { Write-Host $_ -ForegroundColor Red; $ok = $false }
    finally { Pop-Location }
    $sw.Stop()
    $results.Add([pscustomobject]@{ Step = $name; Ok = $ok; Secs = [int]$sw.Elapsed.TotalSeconds })
    if (-not $ok) { Write-Report; Fail "$name (voir la sortie ci-dessus)." }
}

$dest = Join-Path $root (Join-Path $Out $short)
function Write-Report {
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    $lines = @(
        "## Validation locale — commit ``$commit``",
        '',
        "Branche ``$branch``$(if ($dirty) { ' — **copie de travail modifiée : le résultat ne correspond pas exactement au commit**' })",
        "Date : $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')",
        ''
    )
    foreach ($k in $machine.Keys) { $lines += "- $k : $($machine[$k])" }
    $lines += '', '| Étape | Résultat | Durée |', '| --- | --- | --- |'
    foreach ($r in $results) { $lines += "| $($r.Step) | $(if ($r.Ok) { 'OK' } else { 'ÉCHEC' }) | $($r.Secs) s |" }
    $lines += '', '**Non vérifié par ce script** : transcription Whisper en partie, capture CS2, latence réelle, FPS, installation des installateurs. Voir docs/testing/.'
    if ($SkipBundle) { $lines += '', 'Installateurs non générés (-SkipBundle).' }
    $lines | Set-Content -Encoding UTF8 (Join-Path $dest 'validation.md')
}

if ($Release) {
    Step 'Release : commit présent sur origin/main' $root {
        git fetch origin main --quiet
        git merge-base --is-ancestor HEAD origin/main
        if ($LASTEXITCODE -ne 0) { throw 'HEAD n''est pas sur origin/main : seule une version fusionnée dans main se publie.' }
    }
    Step 'Release : versions identiques' $root {
        $v = @(
            (Get-Content package.json -Raw | ConvertFrom-Json).version,
            (Get-Content src-tauri\tauri.conf.json -Raw | ConvertFrom-Json).version,
            ((Select-String -Path src-tauri\Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value)
        )
        if (($v | Select-Object -Unique).Count -ne 1) { throw "versions différentes : $($v -join ', ')" }
        Write-Host "version $($v[0])"
    }
}

# --- Contrôles --------------------------------------------------------------
Step 'Dépendances JS (npm ci)' $root { npm ci }
Step 'Garde anti-triche' $root { npm run check:anticheat }
Step 'Typecheck UI' $root { npm run typecheck }
Step 'Tests UI (Vitest)' $root { npm test }
Step 'Build UI' $root { npm run build }
Step 'rustfmt' "$root\src-tauri" { cargo fmt --all -- --check }
Step 'clippy -D warnings' "$root\src-tauri" { cargo clippy --all-targets --locked -- -D warnings }
Step 'Tests Rust' "$root\src-tauri" { cargo test --all-targets --locked }
if (-not $SkipBundle) {
    Step 'Installateurs NSIS + MSI (MSVC)' $root { npx tauri build --target $target }
}
Step 'audio_probe + asr_bench + translate + translate_bench (release, MSVC)' "$root\src-tauri" {
    cargo build --release --locked --example audio_probe --example asr_bench --example translate --example translate_bench --target $target
}
# TranslateGemma's engine (llama.cpp; Vulkan needs the Vulkan SDK, VULKAN_SDK).
Step 'crosstalk-llm : tests' "$root\src-llm" { cargo test --release --locked }
Step 'crosstalk-llm (release, MSVC, Vulkan)' "$root\src-llm" { cargo build --release --locked --features gpu --target $target }

# --- Paquet de test ---------------------------------------------------------
$rel = "src-tauri\target\$target\release"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item "$rel\examples\audio_probe.exe" "$dest\crosstalk-audio-probe.exe"
Copy-Item "$rel\examples\asr_bench.exe" "$dest\crosstalk-asr-bench.exe"
Copy-Item "$rel\examples\translate.exe" "$dest\crosstalk-translate.exe"
Copy-Item "$rel\examples\translate_bench.exe" "$dest\crosstalk-translate-bench.exe"
Copy-Item "src-llm\target\$target\release\crosstalk-llm.exe", src-tauri\tests\data\translation-corpus.tsv $dest
Copy-Item scripts\download-models.ps1, scripts\fetch-asr-corpus.py $dest
if (-not $SkipBundle) {
    Copy-Item "$rel\crosstalk.exe" $dest
    Copy-Item "$rel\bundle\nsis\*.exe", "$rel\bundle\msi\*.msi" $dest
}
Get-ChildItem $dest -File -Include *.exe, *.msi -Recurse |
    Get-FileHash -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash.ToLower())  $(Split-Path -Leaf $_.Path)" } |
    Set-Content -Encoding ASCII (Join-Path $dest 'SHA256SUMS.txt')
Write-Report

Write-Host "`nOK : toutes les étapes ont réussi. Paquet et rapport : $dest" -ForegroundColor Green
if ($Release) {
    $v = (Get-Content package.json -Raw | ConvertFrom-Json).version
    Write-Host "Publication (à lancer vous-même après relecture) :"
    Write-Host "  git tag v$v $commit; git push origin v$v"
    Write-Host "  gh release create v$v --draft --title ""Crosstalk v$v"" $dest\*-setup.exe $dest\*.msi $dest\SHA256SUMS.txt"
}
