# Downloads the speech models used by Crosstalk into %LOCALAPPDATA%\Crosstalk\models
# and checks them. In-app download arrives with feat/settings-persistence (#15).
#
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1            # base + VAD
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -Models tiny,base,small
param(
  [string[]]$Models = @('base'),
  [string]$Dir = (Join-Path $env:LOCALAPPDATA 'Crosstalk\models')
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Exact sizes (Hugging Face) and SHA-1 (whisper.cpp models/README.md).
$catalog = @{
  'tiny'           = @{ File = 'ggml-tiny.bin';                Size = 77691713;   Sha1 = 'bd577a113a864445d4c299885e0cb97d4ba92b5f' }
  'base'           = @{ File = 'ggml-base.bin';                Size = 147951465;  Sha1 = '465707469ff3a37a2b9b8d8f89f2f99de7299dac' }
  'small'          = @{ File = 'ggml-small.bin';               Size = 487601967;  Sha1 = '55356645c2b361a969dfd0ef2c5a50d530afd8d5' }
  'medium'         = @{ File = 'ggml-medium.bin';              Size = 1533763059; Sha1 = 'fd9727b6e1217c2f614f9b698455c4ffd82463b4' }
  'large-v3-turbo' = @{ File = 'ggml-large-v3-turbo-q5_0.bin'; Size = 574041195;  Sha1 = 'e050f7970618a659205450ad97eb95a18d69c9ee' }
}
$vad = @{ File = 'ggml-silero-v5.1.2.bin'; Size = 885098 }

New-Item -ItemType Directory -Force -Path $Dir | Out-Null

function Get-Model($url, $path, $size, $sha1) {
  if ((Test-Path $path) -and (Get-Item $path).Length -eq $size) {
    Write-Host "deja present : $path"
  } else {
    Write-Host "telechargement : $url"
    Invoke-WebRequest -Uri $url -OutFile "$path.part"
    Move-Item -Force "$path.part" $path
  }
  $len = (Get-Item $path).Length
  if ($len -ne $size) { throw "taille inattendue pour $path : $len au lieu de $size" }
  if ($sha1) {
    $h = (Get-FileHash -Algorithm SHA1 $path).Hash.ToLower()
    if ($h -ne $sha1) { Remove-Item $path; throw "SHA-1 invalide pour $path" }
  }
  $h256 = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLower()
  Write-Host "ok : $(Split-Path -Leaf $path)  sha256 $h256"
}

Get-Model "https://huggingface.co/ggml-org/whisper-vad/resolve/main/$($vad.File)" (Join-Path $Dir $vad.File) $vad.Size $null
foreach ($m in $Models) {
  $spec = $catalog[$m]
  if (-not $spec) { throw "modele inconnu : $m (choix : $($catalog.Keys -join ', '))" }
  Get-Model "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$($spec.File)" (Join-Path $Dir $spec.File) $spec.Size $spec.Sha1
}
Write-Host "Modeles installes dans $Dir. Dans Crosstalk : Audio & IA > Rechercher les modeles."
