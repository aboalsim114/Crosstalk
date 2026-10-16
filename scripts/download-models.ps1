# Downloads the speech models used by Crosstalk (Parakeet TDT 0.6B v3 for
# sherpa-onnx, and the Silero VAD) into %LOCALAPPDATA%\Crosstalk\models and
# checks them, without the app (offline PC, test machine). Crosstalk
# downloads them itself otherwise. Interrupted downloads resume (curl.exe -C).
# Translation needs no model file: it is done by the LLM the user connects
# (Audio & IA > Traduction IA), a cloud provider or Ollama / LM Studio.
#
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1
#
# The table below copies the catalogue of src-tauri/src/speech/models.rs; a
# Rust test checks that every SHA-256 is here.
param(
  [string]$Dir = (Join-Path $env:LOCALAPPDATA 'Crosstalk\models')
)
$ErrorActionPreference = 'Stop'

# Licences: Parakeet TDT 0.6B v3 (NVIDIA) CC-BY-4.0; Silero VAD MIT.
$parakeet = 'https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/main'
$files = @(
  @{ Url = 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx'; Path = 'silero_vad.onnx'; Size = 643854; Sha256 = '9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6' }
  @{ Url = "$parakeet/encoder.int8.onnx"; Path = 'parakeet-tdt-0.6b-v3-int8\encoder.int8.onnx'; Size = 652184281; Sha256 = 'acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247' }
  @{ Url = "$parakeet/decoder.int8.onnx"; Path = 'parakeet-tdt-0.6b-v3-int8\decoder.int8.onnx'; Size = 11845275; Sha256 = '179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e' }
  @{ Url = "$parakeet/joiner.int8.onnx"; Path = 'parakeet-tdt-0.6b-v3-int8\joiner.int8.onnx'; Size = 6355277; Sha256 = '3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3' }
  @{ Url = "$parakeet/tokens.txt"; Path = 'parakeet-tdt-0.6b-v3-int8\tokens.txt'; Size = 93939; Sha256 = 'd58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d' }
)

foreach ($f in $files) {
  $path = Join-Path $Dir $f.Path
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
  if ((Test-Path $path) -and (Get-Item $path).Length -eq $f.Size) {
    Write-Host "deja present : $($f.Path)"
  } else {
    Write-Host "telechargement : $($f.Url)"
    curl.exe -fL --retry 3 -C - -o "$path.part" $f.Url
    if ($LASTEXITCODE -ne 0) { throw "telechargement interrompu : relancez le script pour reprendre ($($f.Path))" }
    Move-Item -Force "$path.part" $path
  }
  $len = (Get-Item $path).Length
  if ($len -ne $f.Size) { throw "taille inattendue pour $($f.Path) : $len au lieu de $($f.Size)" }
  $h = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLower()
  if ($h -ne $f.Sha256) { Remove-Item $path; throw "SHA-256 invalide pour $($f.Path) : fichier supprime, relancez le script" }
  Write-Host "ok : $($f.Path)"
}

Write-Host "Modeles installes dans $Dir. Dans Crosstalk : Audio & IA > Rechercher les modeles."
