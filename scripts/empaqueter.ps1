# Compile Epicube Craft et rassemble une version prete a distribuer pour Windows.
# Resultat : dist\epicube-craft-windows-x86_64\ et l'archive .zip du meme nom.
# Il faut Rust (https://rustup.rs) et les outils de compilation C++ de Visual Studio.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$name = "epicube-craft-windows-x86_64"
$out = Join-Path "dist" $name

cargo build --release -p client -p server
if ($LASTEXITCODE -ne 0) { throw "La compilation a echoue." }

if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "target\release\epicube-craft.exe", "target\release\epicube-craft-server.exe" $out
Copy-Item -Recurse "audio" (Join-Path $out "audio")
@"
Epicube Craft

Lancer le jeu : double-cliquer sur epicube-craft.exe.

Serveur dedie (facultatif), dans un terminal :
    epicube-craft-server.exe --port 25600 --world monde

Le dossier audio doit rester a cote de l'executable. Les mondes sont enregistres
dans le dossier saves, les reglages dans epicube-craft.cfg.

Il faut une carte graphique compatible Vulkan ou DirectX 12, avec des pilotes a jour.
"@ | Set-Content -Encoding UTF8 (Join-Path $out "LISEZ-MOI.txt")

$zip = Join-Path "dist" "$name.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $out -DestinationPath $zip
Write-Host "Pret : $out et $zip"
