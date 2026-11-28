#!/bin/sh
# Compile Epicube Craft et rassemble une version prete a distribuer (macOS ou Linux).
# Resultat : dist/epicube-craft-<systeme>-<processeur>/ et l'archive .tar.gz du meme nom.
#
# Linux : il faut les en-tetes de la bibliotheque son, par exemple
#   Debian / Ubuntu : sudo apt install build-essential pkg-config libasound2-dev
#   Fedora          : sudo dnf install gcc pkgconf-pkg-config alsa-lib-devel
#   Arch            : sudo pacman -S base-devel alsa-lib
set -eu
cd "$(dirname "$0")/.."

case "$(uname -s)" in
    Darwin) os=macos ;;
    Linux) os=linux ;;
    *) echo "Systeme non pris en charge par ce script : $(uname -s)" >&2; exit 1 ;;
esac
name="epicube-craft-$os-$(uname -m)"
out="dist/$name"

cargo build --release -p client -p server

rm -rf "$out"
mkdir -p "$out"
cp target/release/epicube-craft target/release/epicube-craft-server "$out/"
cp -R audio "$out/audio"
# Les modes de jeu livres avec le jeu (UHC, Bed Wars...).
cp -R modes "$out/modes"
cat > "$out/LISEZ-MOI.txt" <<LISEZ
Epicube Craft

Lancer le jeu : double-cliquer sur epicube-craft, ou dans un terminal :
    ./epicube-craft

Serveur dedie (facultatif) :
    ./epicube-craft-server --port 25600 --world monde

Les dossiers audio et modes doivent rester a cote de l'executable. Les mondes sont enregistres
dans le dossier saves, les reglages dans epicube-craft.cfg.

Linux : il faut une carte graphique compatible Vulkan et la bibliotheque son ALSA
(libasound2), presentes sur la plupart des distributions.
macOS : au premier lancement, clic droit sur epicube-craft puis « Ouvrir », car
l'executable n'est pas signe.
LISEZ

tar -czf "dist/$name.tar.gz" -C dist "$name"
echo "Pret : $out et dist/$name.tar.gz"
