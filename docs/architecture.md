# Architecture

Le projet est un espace de travail Cargo de trois paquets, environ 27 000 lignes de Rust.

| Paquet | Rôle |
|---|---|
| `shared` | tout ce que client et serveur doivent comprendre de la même façon : blocs, objets, recettes, génération du monde, règles des circuits, physique, protocole, commandes, voix |
| `server` | la simulation : il fait autorité sur le monde, les inventaires, les créatures et la survie |
| `client` | la fenêtre, le rendu, le son, l'interface ; il embarque aussi le serveur pour le solo |

## shared

| Module | Contenu |
|---|---|
| `blocks` | les blocs, leur forme, leurs tuiles de texture |
| `items` | objets, outils, armes, armures, nourriture, durabilité |
| `craft` | recettes, cuisson, combustibles |
| `gen` | génération : relief, biomes, grottes, minerais, mondes de Yomi et de Takamagahara |
| `structures` | villages, sanctuaires, tombes, temples |
| `chunk` | stockage d'un chunk (16 × 384 × 16) et son codage |
| `signal` | règles des circuits de reiseki |
| `physics` | collisions, en simple et double précision |
| `proto` | messages échangés et leur codage |
| `rules` | modes de jeu et difficultés |
| `commands` | table des commandes, analyse et complétion |
| `voice` | format de la voix : compression, rééchantillonnage, portée |
| `trade`, `mobs`, `seed`, `util` | commerce, créatures, graines, bruit et hasard |

## Le serveur

- **20 ticks par seconde**. Chaque tick traite les messages reçus, puis fait avancer la simulation : fluides, chutes de blocs, pousse des plantes, fours, créatures, projectiles, circuits, survie des joueurs.
- **Trois mondes** en mémoire ; à chaque tick, chacun avance à son tour tant qu'il a des joueurs ou des chunks chargés.
- **Double précision** partout : positions et physique restent exactes jusqu'à la bordure du monde.
- **Génération en parallèle** : les chunks à envoyer sont générés sur plusieurs cœurs avant d'être chargés, pour ne pas ralentir le tick.
- **Circuits** : un moteur à « battements » (deux ticks), avec événements programmés, verrouillage des relais et extinction des lanternes qui clignotent trop vite.

## Le protocole

TCP, messages codés avec `bincode` et préfixés par leur longueur. Le client annonce sa version à la connexion ; une version différente est refusée.

- Le client envoie ses intentions : position, creuser, utiliser, frapper, cliquer une case, ligne de chat, tranche de voix.
- Le serveur répond par l'état : chunks, changements de blocs, positions, inventaire, statistiques, créatures, sons, lignes de chat, voix des joueurs proches.

Le serveur vérifie ce qu'il reçoit : portée des actions, durée de minage, mode de jeu, droits des commandes, débit de la voix.

## Le client

| Module | Contenu |
|---|---|
| `main` | boucle de jeu, entrées, déplacement, réception des messages |
| `gfx` | rendu avec `wgpu` : scène, contours à l'encre, interface |
| `mesh`, `mesher` | maillage des chunks et lumière, sur des fils de travail |
| `atlas`, `pixels` | textures dessinées par le code, en pixel art |
| `models`, `items3d`, `itemdefs` | modèles des créatures et des objets |
| `assets` | export et relecture des ressources (PNG, Blockbench) |
| `hud`, `ui`, `menu`, `chatbox` | interface |
| `audio`, `sounds`, `mic` | sons, musique, voix, micro |
| `net`, `settings`, `prof` | réseau, réglages, mesure du temps par étape |

### Le rendu

1. **Scène** dans une image hors écran : ciel, blocs pleins du plus proche au plus lointain (faces arrière écartées), formes à deux faces, créatures et objets, puis transparents du plus lointain au plus proche.
2. **Contours à l'encre** : une passe plein écran repère les ruptures de profondeur et les arêtes.
3. **Interface** par-dessus.

Seuls les chunks qui coupent le champ de vision sont dessinés. La scène peut être calculée plus petite que l'écran puis agrandie (réglage « Résolution »).

**Origine flottante** : les coordonnées affichées sont relatives à une origine qui suit le joueur, pour garder la précision des nombres en simple précision à des millions de blocs du centre.

### Le son

Les bruitages sont décodés au démarrage et placés dans l'espace (distance et gauche/droite). Les ambiances sont des boucles dont le niveau glisse selon le lieu, l'heure et la lumière. La musique dépend du monde et de la situation. Les voix des joueurs sont des flux séparés, chacun placé à la position de celui qui parle.

## Formats de sauvegarde

Voir [Multijoueur et serveur](serveur.md#sauvegardes). Les fichiers portent un en-tête de version ; les sauvegardes des versions précédentes (mondes de 128 blocs de haut, positions en simple précision) sont relues et converties.

## Atlas de textures

Une seule image de 2048 × 2048 :

| Zone | Contenu |
|---|---|
| haut | tuiles des blocs et icônes, 64 × 64 |
| milieu | cases des modèles de créatures, 256 × 256 |
| ensuite | police de l'interface |
| bas | cases des modèles d'objets, 96 × 96 |
