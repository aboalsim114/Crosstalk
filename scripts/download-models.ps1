# Downloads the speech and translation models used by Crosstalk into
# %LOCALAPPDATA%\Crosstalk\models and checks them, without the app (offline
# PC, test machine). Crosstalk downloads them itself otherwise (Langues,
# Audio & IA). All of them run on the PC: no API key, no subscription.
#
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1                    # base + VAD
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -Translation       # + ru, en -> fr
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -Translation -Target ru -Sources en,de
#   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -Models tiny,base,small
#
# -Target is the user's language, -Sources the players' languages (Langues).
# A pair without a direct model goes through English, as in the app.
# The tables below copy the catalogues of src-tauri/src/speech/models.rs and
# src-tauri/src/translation/models.rs; a Rust test checks they stay equal.
param(
  [string[]]$Models = @('base'),
  [switch]$Translation,
  [string]$Target = 'fr',
  [string[]]$Sources = @('ru', 'en'),
  [string]$Dir = (Join-Path $env:LOCALAPPDATA 'Crosstalk\models')
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
# With powershell -File, "tiny,base" arrives as one string: split it.
$Models = @($Models | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Sources = @($Sources | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

# Exact sizes (Hugging Face) and SHA-1 (whisper.cpp models/README.md).
$catalog = @{
  'tiny'           = @{ File = 'ggml-tiny.bin';                Size = 77691713;   Sha1 = 'bd577a113a864445d4c299885e0cb97d4ba92b5f' }
  'base'           = @{ File = 'ggml-base.bin';                Size = 147951465;  Sha1 = '465707469ff3a37a2b9b8d8f89f2f99de7299dac' }
  'small'          = @{ File = 'ggml-small.bin';               Size = 487601967;  Sha1 = '55356645c2b361a969dfd0ef2c5a50d530afd8d5' }
  'medium'         = @{ File = 'ggml-medium.bin';              Size = 1533763059; Sha1 = 'fd9727b6e1217c2f614f9b698455c4ffd82463b4' }
  'large-v3-turbo' = @{ File = 'ggml-large-v3-turbo-q5_0.bin'; Size = 574041195;  Sha1 = 'e050f7970618a659205450ad97eb95a18d69c9ee' }
}
$vad = @{ File = 'ggml-silero-v5.1.2.bin'; Size = 885098 }

# Translation: Helsinki-NLP Opus-MT (Apache-2.0), files used as published.
# Exact sizes from Hugging Face; the SHA-256 is printed for the test report.
$pairs = @{
  'opus-mt-fr-en' = @{ 'config.json' = 1416; 'vocab.json' = 1339166; 'source.spm' = 802397; 'target.spm' = 778395; 'pytorch_model.bin' = 300827685 }
  'opus-mt-fr-ru' = @{ 'config.json' = 1381; 'vocab.json' = 2658919; 'source.spm' = 820963; 'target.spm' = 1063848; 'pytorch_model.bin' = 310806561 }
  'opus-mt-fr-de' = @{ 'config.json' = 1381; 'vocab.json' = 1464136; 'source.spm' = 812399; 'target.spm' = 827737; 'pytorch_model.bin' = 304190913 }
  'opus-mt-fr-es' = @{ 'config.json' = 1381; 'vocab.json' = 2337705; 'source.spm' = 811963; 'target.spm' = 818689; 'pytorch_model.bin' = 332239713 }
  'opus-mt-fr-uk' = @{ 'config.json' = 1381; 'vocab.json' = 2443170; 'source.spm' = 832408; 'target.spm' = 1012055; 'pytorch_model.bin' = 305721705 }
  'opus-mt-en-fr' = @{ 'config.json' = 1416; 'vocab.json' = 1339166; 'source.spm' = 778395; 'target.spm' = 802397; 'pytorch_model.bin' = 300827685 }
  'opus-mt-en-ru' = @{ 'config.json' = 1381; 'vocab.json' = 2601758; 'source.spm' = 802781; 'target.spm' = 1080169; 'pytorch_model.bin' = 306991893 }
  'opus-mt-en-de' = @{ 'config.json' = 1335; 'vocab.json' = 1273232; 'source.spm' = 768489; 'target.spm' = 796845; 'pytorch_model.bin' = 297928209 }
  'opus-mt-en-es' = @{ 'config.json' = 1473; 'vocab.json' = 1590040; 'source.spm' = 801636; 'target.spm' = 825924; 'pytorch_model.bin' = 312087523 }
  'opus-mt-en-uk' = @{ 'config.json' = 1381; 'vocab.json' = 2367710; 'source.spm' = 808645; 'target.spm' = 1007605; 'pytorch_model.bin' = 305081481 }
  'opus-mt-ru-fr' = @{ 'config.json' = 1416; 'vocab.json' = 2658919; 'source.spm' = 1063848; 'target.spm' = 820963; 'pytorch_model.bin' = 310806561 }
  'opus-mt-ru-en' = @{ 'config.json' = 1381; 'vocab.json' = 2601758; 'source.spm' = 1080169; 'target.spm' = 802781; 'pytorch_model.bin' = 306991893 }
  'opus-mt-ru-es' = @{ 'config.json' = 1381; 'vocab.json' = 2619455; 'source.spm' = 1060731; 'target.spm' = 829382; 'pytorch_model.bin' = 308863317 }
  'opus-mt-ru-uk' = @{ 'config.json' = 1394; 'vocab.json' = 3359419; 'source.spm' = 1049819; 'target.spm' = 1031670; 'pytorch_model.bin' = 297263361 }
  'opus-mt-de-fr' = @{ 'config.json' = 1381; 'vocab.json' = 1464136; 'source.spm' = 827737; 'target.spm' = 812399; 'pytorch_model.bin' = 304190913 }
  'opus-mt-de-en' = @{ 'config.json' = 1381; 'vocab.json' = 1273232; 'source.spm' = 796845; 'target.spm' = 768489; 'pytorch_model.bin' = 297928209 }
  'opus-mt-de-es' = @{ 'config.json' = 1381; 'vocab.json' = 1465772; 'source.spm' = 828663; 'target.spm' = 818345; 'pytorch_model.bin' = 304494609 }
  'opus-mt-de-uk' = @{ 'config.json' = 1394; 'vocab.json' = 2510373; 'source.spm' = 846351; 'target.spm' = 1031063; 'pytorch_model.bin' = 307002153 }
  'opus-mt-es-fr' = @{ 'config.json' = 1381; 'vocab.json' = 2337705; 'source.spm' = 818689; 'target.spm' = 811963; 'pytorch_model.bin' = 332239713 }
  'opus-mt-es-en' = @{ 'config.json' = 1438; 'vocab.json' = 1590040; 'source.spm' = 825924; 'target.spm' = 801636; 'pytorch_model.bin' = 312087523 }
  'opus-mt-es-ru' = @{ 'config.json' = 1381; 'vocab.json' = 2619455; 'source.spm' = 829382; 'target.spm' = 1060731; 'pytorch_model.bin' = 308863317 }
  'opus-mt-es-de' = @{ 'config.json' = 1381; 'vocab.json' = 1465772; 'source.spm' = 818345; 'target.spm' = 828663; 'pytorch_model.bin' = 304494609 }
  'opus-mt-es-uk' = @{ 'config.json' = 1394; 'vocab.json' = 2503645; 'source.spm' = 840419; 'target.spm' = 1027691; 'pytorch_model.bin' = 306872877 }
  'opus-mt-uk-fr' = @{ 'config.json' = 1381; 'vocab.json' = 2443170; 'source.spm' = 1012055; 'target.spm' = 832408; 'pytorch_model.bin' = 305721705 }
  'opus-mt-uk-en' = @{ 'config.json' = 1381; 'vocab.json' = 2367710; 'source.spm' = 1007605; 'target.spm' = 808645; 'pytorch_model.bin' = 305081481 }
  'opus-mt-uk-ru' = @{ 'config.json' = 1394; 'vocab.json' = 3359419; 'source.spm' = 1031670; 'target.spm' = 1049819; 'pytorch_model.bin' = 297263361 }
  'opus-mt-uk-de' = @{ 'config.json' = 1394; 'vocab.json' = 2510373; 'source.spm' = 1031063; 'target.spm' = 846351; 'pytorch_model.bin' = 307002153 }
  'opus-mt-uk-es' = @{ 'config.json' = 1381; 'vocab.json' = 2423810; 'source.spm' = 1008282; 'target.spm' = 833438; 'pytorch_model.bin' = 305801733 }
  'opus-mt-pl-en' = @{ 'config.json' = 1381; 'vocab.json' = 1515249; 'source.spm' = 834921; 'target.spm' = 785212; 'pytorch_model.bin' = 308863317 }
  'opus-mt-pl-fr' = @{ 'config.json' = 1381; 'vocab.json' = 1462731; 'source.spm' = 837215; 'target.spm' = 808576; 'pytorch_model.bin' = 301927557 }
  'opus-mt-pl-de' = @{ 'config.json' = 1381; 'vocab.json' = 1467369; 'source.spm' = 839333; 'target.spm' = 823900; 'pytorch_model.bin' = 302709369 }
  'opus-mt-pl-es' = @{ 'config.json' = 1381; 'vocab.json' = 1447545; 'source.spm' = 835744; 'target.spm' = 814578; 'pytorch_model.bin' = 301164213 }
  'opus-mt-pl-uk' = @{ 'config.json' = 1394; 'vocab.json' = 2560781; 'source.spm' = 850461; 'target.spm' = 1029149; 'pytorch_model.bin' = 306987789 }
  'opus-mt-tr-en' = @{ 'config.json' = 1381; 'vocab.json' = 1563964; 'source.spm' = 839750; 'target.spm' = 796647; 'pytorch_model.bin' = 306727185 }
  'opus-mt-tr-fr' = @{ 'config.json' = 1381; 'vocab.json' = 1512671; 'source.spm' = 841064; 'target.spm' = 814377; 'pytorch_model.bin' = 301474065 }
  'opus-mt-tr-es' = @{ 'config.json' = 1381; 'vocab.json' = 1501501; 'source.spm' = 840025; 'target.spm' = 816299; 'pytorch_model.bin' = 300932337 }
  'opus-mt-tr-uk' = @{ 'config.json' = 1394; 'vocab.json' = 2616593; 'source.spm' = 859415; 'target.spm' = 1028184; 'pytorch_model.bin' = 307461801 }
}
# The models of one route: direct, otherwise through English (none when the
# languages are equal or no model exists).
function Get-Route($src, $tgt) {
  if ($src -eq $tgt) { return @() }
  if ($pairs.ContainsKey("opus-mt-$src-$tgt")) { return @("opus-mt-$src-$tgt") }
  if ($src -eq 'en' -or $tgt -eq 'en') { return $null }
  if ($pairs.ContainsKey("opus-mt-$src-en") -and $pairs.ContainsKey("opus-mt-en-$tgt")) { return @("opus-mt-$src-en", "opus-mt-en-$tgt") }
  return $null
}
# Models of the chosen languages, checked before anything is downloaded.
$wanted = @()
if ($Translation) {
  foreach ($src in $Sources) {
    $route = Get-Route $src $Target
    if ($null -eq $route) { throw "pas de modele de traduction $src -> $Target" }
    $wanted += $route
  }
  $wanted = @($wanted | Select-Object -Unique)
}

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

if ($Translation) {
  foreach ($pair in $wanted) {
    $pairDir = Join-Path $Dir $pair
    New-Item -ItemType Directory -Force -Path $pairDir | Out-Null
    foreach ($file in $pairs[$pair].Keys) {
      Get-Model "https://huggingface.co/Helsinki-NLP/$pair/resolve/main/$file" (Join-Path $pairDir $file) $pairs[$pair][$file] $null
    }
  }
}
Write-Host "Modeles installes dans $Dir. Dans Crosstalk : Audio & IA > Rechercher les modeles."
