#!/bin/sh
# Compile le mod Rubis : produit serveur.wasm et client.wasm dans ce dossier.
# Il faut la cible WebAssembly de Rust : rustup target add wasm32-unknown-unknown
set -eu
cd "$(dirname "$0")"
for part in serveur client; do
    cargo build --release --target wasm32-unknown-unknown --manifest-path "$part/Cargo.toml"
    cp "$part/target/wasm32-unknown-unknown/release/rubis_$part.wasm" "$part.wasm"
done
echo "Pret : copiez le dossier rubis dans le dossier mods du jeu ou du serveur."
