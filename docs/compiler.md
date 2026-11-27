# Compiler et distribuer

## Prérequis

- [Rust](https://rustup.rs). Le projet fixe sa version (1.98) dans `rust-toolchain.toml` : `rustup` l'installe tout seul au premier `cargo`. Les versions plus récentes refusent pour l'instant une dépendance de la fenêtre macOS.
- **Windows** : les outils de compilation C++ de Visual Studio (proposés par l'installateur de Rust).
- **Linux** : un compilateur C, `pkg-config` et les en-têtes de la bibliothèque son ALSA.

  | Distribution | Commande |
  |---|---|
  | Debian, Ubuntu | `sudo apt install build-essential pkg-config libasound2-dev` |
  | Fedora | `sudo dnf install gcc pkgconf-pkg-config alsa-lib-devel` |
  | Arch | `sudo pacman -S base-devel alsa-lib` |

- **macOS** : les outils de ligne de commande Xcode (`xcode-select --install`).

## Compiler et lancer

```bash
cargo run --release -p client
```

```bash
cargo run --release -p server -- --world monde
```

Les exécutables sont dans `target/release/` : `epicube-craft` et `epicube-craft-server`. La version finale est compilée avec l'optimisation globale (LTO), ce qui prend plusieurs minutes ; pour itérer, `cargo run -p client` suffit.

Le jeu lit `audio/`, `assets/` et `saves/` dans le dossier courant ; s'il n'y trouve pas `audio/`, il se place à côté de son exécutable, ou dans le projet qui l'a compilé.

## Produire une version à distribuer

| Système | Commande | Résultat |
|---|---|---|
| macOS, Linux | `sh scripts/empaqueter.sh` | `dist/epicube-craft-<système>-<processeur>/` et son `.tar.gz` |
| Windows | `scripts\empaqueter.ps1` | `dist\epicube-craft-windows-x86_64\` et son `.zip` |

Chaque dossier contient le jeu, le serveur dédié, les sons et un fichier `LISEZ-MOI.txt`.

Un exécutable se compile sur le système auquel il est destiné. La recette `.github/workflows/build.yml` le fait automatiquement sur les trois systèmes à chaque envoi sur GitHub : elle lance les tests, empaquette, et publie les archives comme *artifacts* de l'exécution.

Pour publier une version, posez une étiquette et envoyez-la : la recette ajoute les archives des trois systèmes à la release du même nom.

```bash
git tag v0.9.1
```

```bash
git push origin v0.9.1
```

## Tests

```bash
cargo test --workspace
```

Les tests couvrent la génération du monde, les circuits, la ferme, le combat, les armures, les modes de jeu, les commandes, le chat vocal, les formats de sauvegarde, les modèles d'objets et leur aller-retour Blockbench, et la présence de tous les sons que le jeu demande.

Deux mesures se lancent à la demande :

```bash
cargo test --release -p server -- --ignored --nocapture vitesse
```

```bash
cargo test --release -p client -- --ignored --nocapture vitesse
```

## Mesurer les performances

```bash
EPICUBE_FULL=1 EPICUBE_BENCH=10 ./target/release/epicube-craft --solo --world mesure
```

Le jeu laisse le monde se charger, mesure pendant dix secondes en cadence libre, puis affiche les images par seconde (moyenne et pires images) et le temps passé dans chaque étape d'une image.

## Variables de développement

| Variable | Effet |
|---|---|
| `EPICUBE_DEMO=<nom>` | ouvre la partie dans une situation préparée : `inventaire`, `armes`, `armure`, `creatif`, `chat`, `infos`, `village`, `circuits`, `ferme`, `grotte`, `abime`, `zenith`, `lointain`, `marche`, `parametres`... |
| `EPICUBE_BENCH=<secondes>` | mode mesure (voir ci-dessus) |
| `EPICUBE_FULL=1` | avec la mesure : passe en plein écran |
| `EPICUBE_BACKEND=vulkan` \| `metal` \| `auto` | choisit le moteur graphique (Vulkan d'abord par défaut, puis celui du système) |
| `RUST_LOG=warn` | affiche les journaux du moteur graphique |

Sous Windows, le jeu n'ouvre pas de fenêtre de terminal ; lancé depuis un terminal, il y écrit tout de même ses messages.

## Limites connues

- Le jeu est développé sur macOS. Les versions Windows et Linux compilent, mais n'ont pas encore été essayées en jeu.
- Sur macOS, l'exécutable n'est ni signé ni empaqueté en application (`.app`).
- En fenêtre, macOS peut plafonner l'affichage à la fréquence de l'écran même synchro verticale coupée ; le plein écran lève cette limite.
