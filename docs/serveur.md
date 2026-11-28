# Multijoueur et serveur

Le jeu fonctionne toujours avec un serveur, qui fait autorité sur le monde, les inventaires et la survie. En solo, ce serveur tourne à l'intérieur du jeu ; à plusieurs, les joueurs se connectent à une partie ouverte ou à un serveur dédié.

## Jouer à plusieurs

Tout passe par l'écran **Multijoueur**. À gauche : la recherche, les modes de jeu avec leur nombre de parties, les amis et le bouton pour héberger. À droite : toutes les parties en ligne, qu'elles tournent dans un centre de données, sur un NAS ou dans le jeu d'un joueur.

- **Rejoindre** une partie de la liste. On cherche par nom, mode de jeu, étiquette ou pseudo de l'hôte.
- **Jouer : (un mode)** : choisir un mode à gauche, puis ce bouton. Le jeu rejoint une partie de ce mode qui a de la place ; s'il n'y en a pas, il en héberge une, publique, et l'on attend les autres.
- **Héberger une partie** : choisir le mode de jeu, le nom, le nombre de places, et **publique** ou **privée**. La partie tourne dans le jeu de celui qui l'héberge, et s'arrête quand il la quitte.

Il n'y a rien à régler : ni port à ouvrir, ni box à toucher. Une partie que l'on ne peut pas joindre directement (derrière une box, c'est presque toujours le cas) passe d'elle-même par le relais d'Epicube Hub.

### Parties publiques et privées

Une partie **publique** est ouverte à tous. Une partie **privée** figure aussi dans la liste, avec un cadenas : seuls les **amis** de celui qui l'héberge peuvent y entrer.

### Amis

Les amis sont ceux du compte Epicube : la même liste que dans Epicube Hub. Dans **Multijoueur → Amis**, on demande quelqu'un par son pseudo, on accepte ou refuse les demandes, on retire un ami, on voit qui est en ligne et l'on s'écrit ; un message envoyé depuis le jeu se lit aussi dans le lanceur, et inversement. Il faut être connecté à son compte, donc avoir lancé le jeu depuis Epicube Hub.

### Modes de jeu

| Mode | En bref |
|---|---|
| Survie | récolter, fabriquer, bâtir, se défendre ; le monde est gardé d'une partie à l'autre |
| Créatif | tous les objets, vol libre ; le monde est gardé |
| UHC | la vie ne remonte pas toute seule, la zone rétrécit, le dernier en vie gagne |
| Bed Wars | des îles dans le ciel, un lit par équipe, une boutique ; la dernière équipe gagne |

UHC et Bed Wars commencent dès que deux joueurs sont là, dans un monde neuf à chaque partie. En survie et en créatif, les mods installés par l'hôte accompagnent la partie ; les autres joueurs les reçoivent en entrant.

Un mode de jeu est un [mod](mods.md#modes-de-jeu) : chacun peut en écrire d'autres. Ceux du jeu sont dans le dossier `modes`.

Client et serveur doivent être de la même version : la liste ne montre que les parties de la version du joueur.

## Serveur dédié

Un serveur dédié s'inscrit lui aussi à l'annuaire : les joueurs le trouvent dans **Multijoueur**, sans adresse à saisir.

```bash
epicube-craft-server --nom "Le Dojo" --mode survie --etiquettes francais,calme --joueurs 20
```

| Option | Rôle | Valeur par défaut |
|---|---|---|
| `--port` | port d'écoute (TCP) | 25600 |
| `--seed` | graine d'un monde neuf | 1337 |
| `--world` | nom du monde, dans le dossier `saves` | `serveur` |
| `--comptes` | `obligatoires` ou `facultatifs` : exiger un compte vérifié | celui de `comptes.txt` |
| `--nom` | nom du serveur dans la liste | `Serveur Epicube Craft` |
| `--mode`, `--nom-du-mode` | mode de jeu : son identifiant, et son nom affiché | `survie` |
| `--etiquettes` | étiquettes, séparées par des virgules | aucune |
| `--joueurs` | nombre de places | 20 |
| `--prive` | le serveur figure dans la liste avec un cadenas | public |
| `--sans-annuaire` | le serveur ne s'annonce pas | il s'annonce |

Les [mods](mods.md) déposés dans le dossier `mods`, à côté du serveur, sont chargés au démarrage et envoyés automatiquement aux joueurs qui le rejoignent.

`Ctrl+C` sauvegarde le monde puis arrête le serveur. Pour jouer depuis Internet, il faut ouvrir ce port sur le routeur de l'hôte.

Le serveur peut n'admettre que les joueurs connectés à leur **compte** : c'est le fichier `comptes.txt`, créé à côté de lui au premier démarrage. Voir [Comptes des joueurs](comptes.md).

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
| `--heberger mode` | héberge directement une partie de ce mode (`--prive` : privée, `--world` : son monde) |
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
| `joueurs/<uuid>.bin` | position, vie, faim, inventaire, équipement, mode de jeu ; un fichier par compte (ou par pseudo, pour un joueur sans compte) |

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

- Une partie hébergée derrière une box passe par le relais d'Epicube Hub : tout son trafic transite par lui, avec un peu plus de délai qu'une connexion directe.
- Pas de liste blanche, de bannissement ni de rôles : tout joueur connecté a les mêmes droits, ceux que donnent les règles du monde.
- Les échanges ne sont pas chiffrés. Sans `comptes=obligatoires`, les pseudos des joueurs sans compte ne sont pas authentifiés.
- La voix passe par le serveur, sur la même connexion que le reste : environ 16 ko par seconde et par joueur qui parle.
