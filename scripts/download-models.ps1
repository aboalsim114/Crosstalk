# Downloads the speech and translation models used by Crosstalk into
# %LOCALAPPDATA%\Crosstalk\models and checks them. All of them run on the PC:
# no API key, no subscription. In-app download arrives with feat/settings-persistence (#15).
#
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1                 # base + VAD
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -Translation    # + TranslateGemma 4B (2,49 Go) + Opus-MT ru/en -> fr (~620 Mo)
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -Models tiny,base,small
param(
  [string[]]$Models = @('base'),
  [switch]$Translation,
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

# Translation: Helsinki-NLP Opus-MT (Apache-2.0), files used as published.
# Exact sizes from Hugging Face; the SHA-256 is printed for the test report.
$pairs = @{
  'opus-mt-ru-fr' = @{ 'config.json' = 1416; 'vocab.json' = 2658919; 'source.spm' = 1063848; 'target.spm' = 820963; 'pytorch_model.bin' = 310806561 }
  'opus-mt-en-fr' = @{ 'config.json' = 1416; 'vocab.json' = 1339166; 'source.spm' = 778395; 'target.spm' = 802397; 'pytorch_model.bin' = 300827685 }
}
if ($Translation) {
  foreach ($pair in $pairs.Keys) {
    $pairDir = Join-Path $Dir $pair
    New-Item -ItemType Directory -Force -Path $pairDir | Out-Null
    foreach ($file in $pairs[$pair].Keys) {
      Get-Model "https://huggingface.co/Helsinki-NLP/$pair/resolve/main/$file" (Join-Path $pairDir $file) $pairs[$pair][$file] $null
    }
  }
}
# TranslateGemma 4B (Google, Gemma Terms of Use), GGUF Q4_K_M converted by
# mradermacher. Checked by the SHA-256 Hugging Face announces (X-Linked-Etag,
# the Git LFS object id), then recorded in the manifest Crosstalk reads.
$tg = @{ Repo = 'mradermacher/translategemma-4b-it-GGUF'; File = 'translategemma-4b-it.Q4_K_M.gguf'; Size = 2489909760; Dir = 'translategemma-4b-it'; Id = 'translategemma-4b-q4km' }
function Get-AnnouncedSha256($url) {
  $req = [System.Net.HttpWebRequest]::Create($url)
  $req.Method = 'HEAD'
  $req.AllowAutoRedirect = $false
  $req.UserAgent = 'Crosstalk'
  try { $resp = $req.GetResponse() } catch [System.Net.WebException] { $resp = $_.Exception.Response }
  $etag = $resp.Headers['X-Linked-Etag']
  if (-not $etag) { $etag = $resp.Headers['ETag'] }
  $resp.Close()
  $h = ($etag -replace '^W/', '').Trim('"').ToLower()
  if ($h -notmatch '^[0-9a-f]{64}$') { throw "empreinte SHA-256 non publiee pour $url" }
  return $h
}
if ($Translation) {
  $tgDir = Join-Path $Dir $tg.Dir
  New-Item -ItemType Directory -Force -Path $tgDir | Out-Null
  $url = "https://huggingface.co/$($tg.Repo)/resolve/main/$($tg.File)"
  $path = Join-Path $tgDir $tg.File
  $expected = Get-AnnouncedSha256 $url
  Get-Model $url $path $tg.Size $null
  $h = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLower()
  if ($h -ne $expected) { Remove-Item $path; throw "SHA-256 invalide pour $path (attendu $expected)" }
  $manifest = [ordered]@{ model = $tg.Id; file = $tg.File; bytes = $tg.Size; sha256 = $h; source = $url; verifiedAt = [int64]([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) }
  $manifest | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $tgDir 'crosstalk-manifest.json')
  Write-Host "TranslateGemma verifie : sha256 $h"
}
Write-Host "Modeles installes dans $Dir. Dans Crosstalk : Audio & IA > Rechercher les modeles."
