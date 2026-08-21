# distribute.ps1
# Assemble le dossier portable ReformoJuste-Portable/ prêt à distribuer.
# Prérequis : setup_languagetool.ps1 exécuté + npm run build effectué.

$ErrorActionPreference = "Stop"
$Root    = $PSScriptRoot
$ExePath = Join-Path $Root "src-tauri\target\release\reformojuste.exe"
$OutDir  = Join-Path $Root "ReformoJuste-Portable"

# Vérifications
if (-not (Test-Path $ExePath)) {
    Write-Error "Exe introuvable : $ExePath`nLancez d'abord : npm run build"
}
if (-not (Test-Path (Join-Path $Root "bundle\languagetool\languagetool-server.jar"))) {
    Write-Error "JAR LanguageTool introuvable.`nLancez d'abord : .\setup_languagetool.ps1"
}
if (-not (Test-Path (Join-Path $Root "bundle\jre\bin\java.exe"))) {
    Write-Error "JRE introuvable.`nLancez d'abord : .\setup_languagetool.ps1"
}

# Nettoyage + création
Write-Host "Assemblage dans : $OutDir"
if (Test-Path $OutDir) { Remove-Item $OutDir -Recurse -Force }
New-Item -ItemType Directory $OutDir | Out-Null

# Exe
Copy-Item $ExePath $OutDir

# config.json
Copy-Item (Join-Path $Root "config.json") $OutDir

# bundle/ (JAR + JRE)
Copy-Item (Join-Path $Root "bundle") -Destination $OutDir -Recurse

Write-Host ""
Write-Host "=== Distribution prête ==="
Write-Host "Contenu de $OutDir :"
Get-ChildItem $OutDir -Recurse | Where-Object { -not $_.PSIsContainer } |
    Select-Object -ExpandProperty FullName |
    ForEach-Object { "  " + $_.Substring($OutDir.Length + 1) }
Write-Host ""
Write-Host "Pour distribuer : compressez le dossier ReformoJuste-Portable/ en ZIP."
Write-Host "L'utilisateur n'a besoin d'installer ni Python ni Java ni aucune dépendance."
