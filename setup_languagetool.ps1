# setup_languagetool.ps1
# Télécharge LanguageTool standalone + JRE Eclipse Temurin dans bundle/
# Compatible PowerShell 5.1 (Windows PowerShell)

$ErrorActionPreference = "Stop"
$BundleDir = Join-Path $PSScriptRoot "bundle"
$ltDir     = Join-Path $BundleDir "languagetool"
$jreDir    = Join-Path $BundleDir "jre"
$ltJar     = Join-Path $ltDir "languagetool-server.jar"
$jreExe    = Join-Path (Join-Path $jreDir "bin") "java.exe"

# URL directes et stables
$ltUrl  = "https://languagetool.org/download/LanguageTool-stable.zip"
# JRE Eclipse Temurin 21.0.7 — Windows x64 — lien direct GitHub Releases
$jreUrl = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.7%2B6/OpenJDK21U-jre_x64_windows_hotspot_21.0.7_6.zip"

New-Item -ItemType Directory -Force $BundleDir | Out-Null

# ── LanguageTool ──────────────────────────────────────────────────────────────
if (Test-Path $ltJar) {
    Write-Host "[LT] languagetool-server.jar deja present - ignore."
} else {
    Write-Host "[LT] Telechargement de LanguageTool..."
    $ltZip = Join-Path $BundleDir "LanguageTool.zip"

    $wc = New-Object System.Net.WebClient
    $wc.DownloadFile($ltUrl, $ltZip)

    $size = (Get-Item $ltZip).Length
    Write-Host "[LT] Telecharge : $([math]::Round($size/1MB, 1)) Mo"

    if ($size -lt 1MB) {
        Remove-Item $ltZip -Force
        throw "Echec du telechargement LanguageTool (fichier trop petit)."
    }

    Write-Host "[LT] Extraction..."
    $tmpLt = Join-Path $BundleDir "lt_tmp"
    Expand-Archive -Path $ltZip -DestinationPath $tmpLt -Force
    $inner = Get-ChildItem $tmpLt | Select-Object -First 1
    New-Item -ItemType Directory -Force $ltDir | Out-Null
    Copy-Item (Join-Path $inner.FullName "*") -Destination $ltDir -Recurse -Force
    Remove-Item $tmpLt -Recurse -Force
    Remove-Item $ltZip -Force

    if (-not (Test-Path $ltJar)) {
        throw "Extraction echouee : languagetool-server.jar introuvable dans $ltDir"
    }
    Write-Host "[LT] OK"
}

# ── JRE Eclipse Temurin ───────────────────────────────────────────────────────
if (Test-Path $jreExe) {
    Write-Host "[JRE] java.exe deja present - ignore."
} else {
    Write-Host "[JRE] Telechargement du JRE Temurin 21 (Windows x64)..."
    $jreZip = Join-Path $BundleDir "jre.zip"

    $wc = New-Object System.Net.WebClient
    $wc.DownloadFile($jreUrl, $jreZip)

    $size = (Get-Item $jreZip).Length
    Write-Host "[JRE] Telecharge : $([math]::Round($size/1MB, 1)) Mo"

    if ($size -lt 10MB) {
        Remove-Item $jreZip -Force
        throw "Echec du telechargement JRE (fichier trop petit)."
    }

    Write-Host "[JRE] Extraction..."
    $tmpJre = Join-Path $BundleDir "jre_tmp"
    Expand-Archive -Path $jreZip -DestinationPath $tmpJre -Force
    $inner = Get-ChildItem $tmpJre | Select-Object -First 1
    New-Item -ItemType Directory -Force $jreDir | Out-Null
    Copy-Item (Join-Path $inner.FullName "*") -Destination $jreDir -Recurse -Force
    Remove-Item $tmpJre -Recurse -Force
    Remove-Item $jreZip -Force

    if (-not (Test-Path $jreExe)) {
        throw "Extraction echouee : java.exe introuvable dans $jreDir\bin"
    }
    Write-Host "[JRE] OK"
}

# ── Verification finale ───────────────────────────────────────────────────────
Write-Host ""
Write-Host "=== Verification ==="
if (Test-Path $ltJar)  { Write-Host "  OK : $ltJar" }  else { throw "MANQUANT : $ltJar" }
if (Test-Path $jreExe) { Write-Host "  OK : $jreExe" } else { throw "MANQUANT : $jreExe" }
Write-Host ""
Write-Host "=== Setup termine === Lancez maintenant : .\distribute.ps1"
