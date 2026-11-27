# Multijoueur et serveur

Le jeu fonctionne toujours avec un serveur, qui fait autorité sur le monde, les inventaires et la survie. En solo, ce serveur tourne à l'intérieur du jeu ; à plusieurs, les joueurs se connectent à une partie ouverte ou à un serveur dédié.

## Jouer à plusieurs sur le réseau local

1. Chez l'hôte : **Paramètres → Solo en réseau local : oui**, puis lancer une partie solo. Le bouton de l'écran titre indique « Jouer (ouvert au réseau local) ».
2. Chez les autres : **Multijoueur**, saisir l'adresse de l'hôte (par exemple `192.168.1.20`) et un pseudo.

La partie écoute sur le port **25600** (TCP). Client et serveur doivent être de la même version : un client d'une autre version du protocole est refusé avec un message clair.

## Serveur dédié

```bash
epicube-craft-server --port 25600 --seed 1337 --world monde
```

| Option | Rôle | Valeur par défaut |
|---|---|---|
| `--port` | port d'écoute (TCP) | 25600 |
| `--seed` | graine d'un monde neuf | 1337 |
| `--world` | nom du monde, dans le dossier `saves` | `serveur` |

`Ctrl+C` sauvegarde le monde puis arrête le serveur. Pour jouer depuis Internet, il faut ouvrir ce port sur le routeur de l'hôte.

Le serveur tourne à 20 ticks par seconde. L'écran `F3` du client affiche sa cadence réelle, la durée moyenne d'un tick et le ping.

## Options du client

```bash
epicube-craft [--solo] [--connect hote:port] [--name Pseudo] [--seed 1337] [--world monde] [--lan]
```

| Option | Rôle |
|---|---|
| `--solo` | lance directement une partie solo |
| `--connect hote:port` | se connecte directement à un serveur |
| `--name` | pseudo |
| `--seed` | graine du monde à créer (chiffres ou texte) |
| `--world` | nom du monde |
| `--lan` | ouvre la partie solo au réseau local |
| `--export-assets` | écrit textures et modèles dans le dossier `assets`, puis quitte |
| `--item-sheet fichier.png` | écrit une planche de toutes les icônes d'objets, puis quitte |

Sans option, le jeu s'ouvre sur l'écran titre.

## Sauvegardes

Chaque monde a son dossier dans `saves/` :

| Fichier | Contenu |
|---|---|
| `graine.txt` | la graine, telle qu'elle a été saisie |
| `regles.txt` | les règles du monde (voir ci-dessous) |
| `monde.bin`, `monde_d1.bin`, `monde_d2.bin` | état de chaque monde : heure, coffres, fours, créatures |
| `c_X_Z.bin` | les chunks modifiés ; les autres sont régénérés depuis la graine |
| `joueurs/<pseudo>.bin` | position, vie, faim, inventaire, équipement, mode de jeu |

Le monde est enregistré toutes les trente secondes, et à l'arrêt. Pour sauvegarder un monde, copiez son dossier.

## Règles d'un monde

Le fichier `regles.txt` se modifie à la main, serveur arrêté :

```text
mode=0
difficulte=2
extreme=0
triche=0
```

| Ligne | Valeurs |
|---|---|
| `mode` | mode donné aux nouveaux joueurs : 0 survie, 1 créatif, 2 aventure, 3 spectateur |
| `difficulte` | 0 paisible, 1 facile, 2 normale, 3 difficile |
| `extreme` | 1 : difficile imposé, une seule vie, pas de triche |
| `triche` | 1 : autorise `/mode`, `/donner`, `/tp`... et la touche `F4` |

Un monde créé avant l'existence de ce fichier garde la triche autorisée.

## Limites connues

- Pas de liste blanche, de bannissement ni de rôles : tout joueur connecté a les mêmes droits, ceux que donnent les règles du monde.
- Les échanges ne sont pas chiffrés ; les pseudos ne sont pas authentifiés. À réserver aux parties entre personnes de confiance.
- La voix passe par le serveur, sur la même connexion que le reste : environ 16 ko par seconde et par joueur qui parle.
