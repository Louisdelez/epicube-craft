# Guide du joueur

## Lancer une partie

À l'écran titre :

- **Jouer en solo** : choisissez un nom de monde et, si vous le souhaitez, une graine (des chiffres ou un texte). La même graine redonne le même monde. Un monde déjà créé garde sa graine et ses règles.
- **Multijoueur** : toutes les parties en ligne, la recherche, les amis, et le bouton pour héberger la vôtre (voir [Multijoueur et serveur](serveur.md)).
- **Paramètres** : affichage, son, micro, performances.

À la création d'un monde, vous réglez aussi le [mode de jeu](#modes-de-jeu), la [difficulté](#difficultés) et la **triche** (le droit de changer de mode et d'utiliser les commandes qui modifient la partie).

## Commandes

Les touches sont données pour un clavier AZERTY ; elles occupent la même place sur un clavier QWERTY (`WASD`, `Q` pour jeter).

Toutes ces touches se changent dans **Paramètres → Contrôles** : on clique sur la touche d'une action, puis on appuie sur la nouvelle. Une touche donnée à deux actions s'affiche en rouge ; **Réinitialiser** remet les touches d'origine.

| Action | Touche |
|---|---|
| Avancer, reculer, aller à gauche, à droite | `Z` `S` `Q` `D` |
| Sauter, nager vers le haut | `Espace` |
| Courir (si la faim le permet) | `Maj` gauche |
| Casser un bloc, frapper | clic gauche (maintenu pour miner) |
| Poser un bloc, utiliser, manger | clic droit |
| Choisir une case de la barre rapide | molette ou `1` à `9` |
| Inventaire | `E` |
| Jeter l'objet tenu | `A` (`Ctrl` + `A` : toute la pile) |
| Chat | `T` |
| Commande | `/` |
| Parler (micro en mode « appuyer pour parler ») | `V` |
| Informations (coordonnées, images par seconde, ping) | `F3` |
| Changer de mode de jeu (si la triche est autorisée) | `F4` |
| Afficher les frontières des chunks | `F6` |
| Menu pause, fermer une fenêtre | `Échap` |
| Réapparaître après la mort | `Entrée` |

En vol (créatif et spectateur) : `Espace` monte, `Ctrl` gauche descend, `Maj` accélère. En créatif, deux appuis rapides sur `Espace` font décoller ou atterrir.

### Armes

- **Arc yumi** : maintenir le clic droit pour tendre la corde, relâcher pour tirer. Une jauge sous le viseur indique la tension.
- **Lance yari, kunai** : clic droit pour lancer.
- **Bouclier tate** : maintenir le clic droit pour parer ce qui vient de face.
- Une arme frappe à pleine force quand la jauge sous le viseur est pleine ; frapper trop vite ne fait qu'une fraction des dégâts.

### Inventaire

- Clic gauche : prendre ou poser une pile. Clic droit : en prendre la moitié, ou poser un seul objet.
- `Maj` + clic : envoyer la pile dans l'autre zone (sac, barre rapide, coffre, four).
- À gauche, quatre cases d'**équipement** (tête, torse, jambes, pieds) et votre personnage, qui porte ce que vous équipez. `Maj` + clic sur une pièce d'armure l'enfile ; en jeu, le clic droit avec une pièce en main fait de même.
- À droite, les **recettes** que vous pouvez fabriquer (en vert) ou non. L'inventaire propose celles qui tiennent dans une grille de 2 × 2 ; l'établi, toutes.

## Modes de jeu

| Mode | Ce qui change |
|---|---|
| **Survie** | Le jeu normal : on récolte, on fabrique, on a faim, on peut mourir. |
| **Créatif** | Tous les objets à volonté, casse instantanée sans butin ni usure, les blocs posés ne se consomment pas, aucun dégât ni faim, les créatures vous ignorent, vol libre. |
| **Aventure** | Comme la survie, mais on ne casse ni ne pose de blocs. Coffres, leviers, combat et commerce restent possibles. |
| **Spectateur** | On vole à travers les murs, invisible pour les autres, sans rien pouvoir toucher ni ramasser. |
| **Extrême** | Survie verrouillée en difficile, une seule vie : après la mort, on reste spectateur. Se choisit à la création du monde. |

On change de mode avec `F4`, le bouton « Mode » du menu pause ou la commande `/mode`, si le monde autorise la triche.

## Difficultés

| Difficulté | Créatures | Faim |
|---|---|---|
| **Paisible** | aucune créature hostile | ne descend pas ; la vie remonte seule |
| **Facile** | coups réduits (moitié plus un) | cesse de blesser à cinq cœurs |
| **Normale** | coups normaux | cesse de blesser à un demi-cœur |
| **Difficile** | coups une fois et demie plus forts | peut tuer |

La difficulté règle la force des créatures, des projectiles et des explosions ; la chute et la noyade n'en dépendent pas. Elle se change dans le menu pause ou avec `/difficulte`, sauf en mode extrême.

## Chat et commandes

`T` ouvre le chat, `/` l'ouvre sur une commande. `Entrée` envoie, `Échap` ferme.

Pendant la saisie d'une commande, le jeu propose la suite : noms de commandes, puis valeurs possibles de chaque argument (modes, objets, créatures, joueurs connectés).

- `Tab` complète avec la suggestion choisie.
- Les flèches parcourent les suggestions ; sans suggestion, elles rappellent les lignes déjà envoyées.
- Le mode d'emploi de la commande s'affiche au-dessus ; une commande inconnue passe en rouge.

| Commande | Effet | Triche |
|---|---|---|
| `/aide [commande]` | liste les commandes, ou en explique une | |
| `/mode <mode> [joueur]` | `survie`, `creatif`, `aventure`, `spectateur` | oui |
| `/difficulte <difficulté>` | `paisible`, `facile`, `normale`, `difficile` | oui |
| `/heure <moment ou heure>` | `aube`, `jour`, `midi`, `crepuscule`, `nuit`, `minuit`, ou une heure de 0 à 24 | oui |
| `/tp <x> <y> <z>` | téléporte ; `~` vaut « ici », `~5` « cinq blocs plus loin » | oui |
| `/tp <joueur>` | téléporte auprès d'un joueur | oui |
| `/donner <objet> [quantité] [joueur]` | donne des objets, par nom (`katana_en_fer`) ou numéro | oui |
| `/vider [joueur]` | vide l'inventaire | oui |
| `/tuer [joueur]` | tue un joueur (soi-même par défaut) | oui |
| `/invoquer <créature>` | fait apparaître une créature devant soi | oui |
| `/apparition` | fixe son point de réapparition | oui |
| `/graine` | affiche la graine du monde | |
| `/liste` | liste les joueurs connectés | |
| `/dire <texte>` | annonce un message à tous | |
| `/moi <texte>` | décrit une action : « * Joueur ... » | |
| `/msg <joueur> <texte>` | message privé | |

Les noms anglais sont acceptés (`/gamemode`, `/give`, `/time`, `/tp`, `/kill`...), de même qu'un début de mot sans ambiguïté (`/mode spec`). Les commandes marquées « triche » sont refusées, et ne sont pas proposées, dans un monde qui ne l'autorise pas.

## Chat vocal de proximité

On entend les joueurs situés à moins de 48 blocs, dans le même monde. La voix sort de l'endroit où se trouve celui qui parle : plus forte de près, à gauche ou à droite selon sa position. Les spectateurs ne sont entendus que des autres spectateurs.

Dans **Paramètres → Son**, le bouton **Micro** propose :

- **Coupé** (par défaut) : le micro est fermé.
- **Appuyer sur V pour parler**.
- **Ouvert en permanence** : tout ce que capte le micro part vers les joueurs proches.
- **Se déclenche à la voix** : la transmission démarre quand le niveau dépasse le seuil réglable.

Le test **S'entendre** vous renvoie votre propre voix sans rien envoyer : c'est le moyen de vérifier son micro seul. À la première activation, le système demande l'autorisation d'accéder au micro.

En jeu, le nom de chaque joueur qui parle s'affiche en haut à droite. Un casque évite que la voix des autres reparte dans votre micro.

## Réglages

- **Paramètres** : distance de vue (4 à 12 chunks), champ de vision, sensibilité de la souris, taille de l'interface, plein écran, nuages, balancement de la caméra, contours à l'encre.
- **Son** : volume général, musique, bruitages, ambiance, voix des joueurs, micro.
- **Performances** :
  - *Résolution* : celle de la scène 3D. `auto` la calcule à la résolution de l'écran sans sa haute densité ; `100 %` rétablit le rendu natif.
  - *Interface basse résolution* (macOS) : dessine aussi l'interface en basse résolution, ce qui est bien plus rapide. Le texte garde la même taille et la même netteté.
  - *Images par seconde* : limite réglable de 30 à 240, ou illimitées.
  - *Synchro verticale* : à couper pour dépasser la fréquence de l'écran.

Les réglages sont enregistrés dans `epicube-craft.cfg`, à côté du jeu.

Si le jeu se ferme tout seul, il note la raison dans `plantage.txt`, à côté du jeu : c'est le fichier à joindre pour signaler le problème.

## Limites connues

- Pas de choix du périphérique d'entrée pour le micro, ni de réduction de bruit ou d'annulation d'écho.
- En créatif, seuls les blocs posés ne se consomment pas : nourriture, seaux et graines se consomment encore.
- Pas de sélecteurs de cibles (`@p`, `@a`) dans les commandes.
- Les touches ne sont pas reconfigurables.
