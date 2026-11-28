# Multijoueur et serveur

Le jeu fonctionne toujours avec un serveur, qui fait autorité sur le monde, les inventaires et la survie. En solo, ce serveur tourne à l'intérieur du jeu ; à plusieurs, les joueurs se connectent à une partie ouverte ou à un serveur dédié.

## Jouer

À l'écran titre, **Jouer** ouvre la liste des modes de jeu. Il n'y a pas de solo d'un côté et de multijoueur de l'autre : on choisit un mode, puis on rejoint une partie ou l'on crée la sienne.

- **À gauche**, les modes de jeu (Survie, Créatif, UHC, Bed Wars, et ceux qu'apportent les mods), avec leur nombre de parties en ligne et une recherche.
- **À droite**, les parties du mode choisi, qu'elles tournent dans un centre de données, sur un NAS ou dans le jeu d'un joueur, avec une recherche par nom, étiquette ou pseudo de l'hôte. Chaque partie montre son ping : quatre barres vertes et le temps de réponse en millisecondes. **Rejoindre** y entre.
- **Le « + »**, en haut à droite, crée sa propre partie de ce mode : elle tourne dans le jeu de celui qui la crée, et s'arrête quand il la quitte.

Il n'y a rien à régler : ni port à ouvrir, ni box à toucher. Une partie que l'on ne peut pas joindre directement (derrière une box, c'est presque toujours le cas) passe d'elle-même par le relais d'Epicube Hub.

### Publique, privée ou solo

En créant une partie, on choisit qui peut y venir :

- **Publique** : elle figure dans la liste, tout le monde peut la rejoindre.
- **Privée** : elle figure dans la liste avec un cadenas ; seuls les **amis** de celui qui l'héberge peuvent y entrer.
- **Solo** : personne d'autre ne la voit.

### Amis

Les amis sont ceux du compte Epicube : la même liste que dans Epicube Hub. Leur barre se déplie à droite de tous les écrans (bouton **Amis**), et en pleine partie avec la touche `O` (elle se change dans **Paramètres → Contrôles**) ; `Échap` la replie. On y demande quelqu'un par son pseudo, on accepte ou refuse les demandes, on retire un ami, on voit qui est en ligne et l'on s'écrit ; un message envoyé depuis le jeu se lit aussi dans le lanceur, et inversement. Il faut être connecté à son compte, donc avoir lancé le jeu depuis Epicube Hub.

### Modes de jeu

| Mode | En bref |
|---|---|
| Survie | récolter, fabriquer, bâtir, se défendre ; le monde est gardé d'une partie à l'autre |
| Créatif | tous les objets, vol libre ; le monde est gardé |
| PvP Faction | factions, terres revendiquées, puissance, pillages ; le monde est gardé |
| UHC | la vie ne remonte pas toute seule, la zone rétrécit, le dernier en vie gagne |
| Bed Wars | des îles dans le ciel, un lit par équipe, une boutique ; la dernière équipe gagne |
| Sky Wars | chacun sur son île, des coffres à vider, le centre mieux garni ; le dernier debout gagne |
| Hunger Games | les tributs partent de la corne d'abondance et s'arment dans les coffres de l'arène ; le dernier gagne |

Ces quatre modes commencent dès que deux joueurs sont là (ou tout de suite avec `/uhc demarrer`, `/bedwars demarrer`, `/skywars demarrer`, `/hungergames demarrer`), dans un monde neuf à chaque partie ; leur carte se bâtit toute seule. Qui arrive en cours de partie la regarde en spectateur. L'état de la partie s'affiche en haut de l'écran. En survie et en créatif, les mods installés par l'hôte accompagnent la partie ; les autres joueurs les reçoivent en entrant.

**UHC.** Chacun pour soi, dans un monde de survie. La vie ne remonte jamais seule : seules la **pomme dorée** (une pomme entourée de huit lingots d'or) et la **tête** que laisse un joueur vaincu la rendent. 90 secondes de paix, puis le combat s'ouvre et la zone se referme de 260 à 24 blocs en un quart d'heure ; en dehors, on perd de la vie, deux fois plus vite à la mort subite.

**Bed Wars.** Jusqu'à quatre équipes, chacune sur son île avec son lit, sa boutique et sa forge (du fer, un peu d'or). Tant que le lit tient, on revient en jeu cinq secondes après sa mort ; le lit détruit, la mort est définitive. Les îlots de diamant et le centre (cristaux de braise) paient les meilleurs articles ; six diamants doublent la forge de l'équipe. Seuls se cassent les blocs posés par les joueurs et les lits adverses. Après 6 minutes les diamants tombent deux fois plus vite, à 10 minutes tous les lits sont détruits, à 15 c'est la mort subite.

**Sky Wars.** Huit îles autour d'une île centrale. On attend huit secondes dans une cage au-dessus de la sienne ; les cages s'ouvrent, on vide ses deux coffres, on bâtit un pont. Le centre a quatre coffres mieux garnis. Tomber dans le vide tue, il n'y a pas de seconde vie. Les coffres se regarnissent après trois minutes ; à huit minutes vient la mort subite.

**Hunger Games.** Douze socles en cercle autour de la corne d'abondance et de ses huit coffres ; vingt-quatre autres coffres sont dispersés dans une arène de 180 blocs de côté. Dix secondes immobile sur son socle, puis le signal, et quinze secondes pour courir avant que le combat s'ouvre. On ne casse ni ne pose rien. Les coffres se regarnissent après cinq minutes ; à dix minutes, ou quand il ne reste que trois tributs, vient le match à mort : tout le monde est ramené à la corne, dans une zone de 36 blocs.

### PvP Faction

Un monde de survie **gardé d'une partie à l'autre**, où les joueurs se regroupent en factions. Tout passe par la commande `/f` (`/f aide` les rappelle).

| Commande | Rôle |
|---|---|
| `/f creer <nom>` | fonde une faction |
| `/f inviter <joueur>`, `/f rejoindre <faction>`, `/f quitter`, `/f exclure <joueur>` | les membres |
| `/f claim`, `/f unclaim` | revendique ou rend le chunk où l'on se tient |
| `/f sethome`, `/f home` | la base de la faction (retour après 5 secondes sans bouger) |
| `/f allie <faction>`, `/f ennemi <faction>`, `/f neutre <faction>` | les relations (le chef décide ; une alliance doit être acceptée) |
| `/f info [faction]`, `/f liste`, `/f carte`, `/f power` | renseignements ; la carte montre les chunks alentour |
| `/f chef <joueur>`, `/f dissoudre` | le commandement |
| `/f c <message>` | parle à sa faction |

- **Terres.** Sur ses terres, une faction est seule à bâtir, casser, ouvrir les coffres et actionner quoi que ce soit (ses alliés peuvent ouvrir et actionner). On est prévenu en changeant de territoire.
- **Puissance.** Chaque joueur a 10 de puissance au plus, en perd 4 à chaque mort et en regagne 1 toutes les 5 minutes en jeu. Une faction tient autant de chunks que la somme de la puissance de ses membres.
- **Prise de terres.** Une faction qui tient plus de terres qu'elle n'a de puissance peut se faire prendre ses chunks, un par un, par `/f claim`.
- **Combat.** Pas de coup entre membres ni entre alliés. Les explosifs ne respectent aucune frontière : c'est ainsi qu'on pille une base.
- **Zone sûre.** Trois chunks de côté autour du point de départ : on n'y bâtit pas, on ne s'y bat pas, on n'y revendique rien.

### Les jeux d'Epicube

Les jeux du serveur Epicube (2014-2018) sont refaits pour Epicube Craft, d'après les fiches de son site quand elles existent. Chacun se lance dès que deux joueurs sont là, ou tout de suite avec `/<nom> demarrer` (`/sheepwars demarrer`, `/totem demarrer`...).

| Jeu | En bref |
|---|---|
| SheepWars | deux navires dans le ciel, des arcs, et des moutons aux pouvoirs à lancer sur l'ennemi |
| PvPSwap | chacun sur son île, on s'équipe ; toutes les 40 secondes tout le monde change de place |
| Fallen Kingdoms | des royaumes en survie ; combat après 2 min, assauts après 5 ; on détruit le cœur des autres |
| Age Of Empire | Fallen Kingdoms qui traverse les âges : pierre, fer, diamant |
| PayDay | braqueurs contre policiers : dix lingots de la banque à rapporter au fourgon |
| PitchOut | une plateforme, cinq vies, un bâton qui envoie valser |
| Hunger Games | la corne d'abondance, les coffres de l'arène, le match à mort |
| Catch The Cow | capture de drapeau : la vache d'en face à ramener chez soi, trois fois |
| Domination | trois points à prendre et à tenir ; la première équipe à 200 gagne |
| TeamFortress | capture de mallette avec quatre classes (`/classe`) |
| MouTron | chacun laisse une traînée mortelle derrière lui |
| UHC, UHC Run, UHC Theme | l'UHC ; sa version accélérée ; sa version à thème tiré au sort |
| Survivor | ensemble contre dix manches d'oni |
| Arrow | un arc, une flèche qui tue d'un coup ; le premier à 10 gagne |
| Totem | deux équipes, un totem de cinq blocs chacune à abattre |
| IceRunner | un pont de glace vers le plateau central, à tenir 60 secondes |
| DiscoSheep | la piste de danse : restez sur la bonne dalle |
| EpicSmash | trois vies, des coups qui repoussent de plus en plus fort |
| Gladiators | deux équipes, des manches sans retour ; la première à trois gagne |
| FightClub | une arène, le même équipement pour tous ; le premier à 15 victoires gagne |
| SkyFall | le sol s'efface sous les pas, trois étages au-dessus du vide |
| BuildWar | un thème, une parcelle, trois minutes ; puis on note avec `/note` |
| Chaos | une arène où un événement tombe toutes les quinze secondes |
| King Of The Hill | seul au sommet de la colline, on marque ; le premier à 60 gagne |
| Rush | deux îles, deux lits : l'ancêtre du Bed Wars |

Onze de ces jeux avaient une présentation sur le site d'Epicube et la suivent (SheepWars, PvPSwap, Fallen Kingdoms, PayDay, PitchOut, Hunger Games, Catch The Cow, Domination, MouTron, Survivor, UHC), ainsi que Totem et IceRunner, décrits par des joueurs de l'époque. Les autres n'avaient pas de fiche : leurs règles sont reconstituées d'après leur nom et le genre. Ce que le moteur ne sait pas faire est adapté : MouTron se court à pied, les moutons de SheepWars vont d'eux-mêmes à l'ennemi le plus proche, la glace d'IceRunner se pose à la main.

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
| `--heberger mode` | crée directement une partie de ce mode (`--prive` : privée, `--seul` : solo, `--world` : son monde) |
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
