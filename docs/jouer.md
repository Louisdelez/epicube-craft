# Guide du joueur

## Lancer une partie

À l'écran titre :

- **Jouer** : la liste des modes de jeu ; pour chacun, les parties à rejoindre et un « + » pour créer aussitôt la sienne, publique ou privée (voir [Jouer et serveurs](serveur.md)).
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

**Le corps.** Le personnage se déplace comme dans Minecraft : 4,3 blocs par seconde à la marche, 5,6 en courant, un saut d'un bloc et quart. Au sol il s'arrête net ; en l'air il garde son élan ; sur la glace il glisse ; dans l'eau tout est freiné, et `Espace` fait remonter. Un saut pris en courant porte plus loin, et `Espace` tenu fait ressauter. On court avec `Maj`, ou en appuyant deux fois de suite sur « avancer ». Tout cela ne dépend pas de la cadence d'affichage. Les autres joueurs et les créatures glissent d'une position à l'autre sans à-coups : le jeu estime leur vitesse et prolonge leur course entre deux nouvelles du serveur.

**La vue.** `F5` change de vue : à la première personne (on voit son bras, tel que son skin le dessine), de dos, puis de face. De l'extérieur on se voit marcher, courir, sauter, frapper, exactement comme les autres nous voient : la tête suit le regard pendant que le buste tourne avec un temps de retard, les bras respirent au repos. Le champ de vision s'élargit en courant, et un coup reçu fait pencher la tête un instant.

**S'accroupir** (`Ctrl` gauche maintenu) : on avance à petits pas sans jamais tomber d'un bord, et le regard s'abaisse. Les autres joueurs voient la posture, comme ils voient l'objet tenu, les coups de bras, le bouclier levé, l'arc tendu, la course et les sauts.

En vol (créatif et spectateur) : `Espace` monte, `Ctrl` gauche descend, `Maj` accélère. En créatif, deux appuis rapides sur `Espace` font décoller ou atterrir.

### Armes

- **Arc yumi** : maintenir le clic droit pour tendre la corde, relâcher pour tirer. Une jauge sous le viseur indique la tension.
- **Lance yari, kunai** : clic droit pour lancer.
- **Bouclier tate** : maintenir le clic droit pour parer ce qui vient de face.
- Une arme frappe à pleine force quand la jauge sous le viseur est pleine ; frapper trop vite ne fait qu'une fraction des dégâts.

### Combat

Les mêmes règles valent contre les créatures et, quand la partie l'autorise, entre joueurs.

- **Retour en garde** : après un coup, un coup dans le vide ou un changement d'arme, l'arme met un moment à revenir en garde (la jauge sous le viseur). Un katana frappe vite, une hache lentement mais fort.
- **Coup critique** : porté en retombant d'un saut, l'arme en garde et sans courir, il fait une fois et demie les dégâts.
- **Coup en courant** : il repousse bien plus loin, mais n'est jamais critique.
- **Fauchage** : un katana en garde, les pieds au sol et sans courir, effleure aussi les voisins de la cible.
- **Recul** : un coup soulève qui a les pieds au sol ; en l'air, il ne fait que pousser, ce qui permet d'enchaîner.
- **Invulnérabilité** : une demi-seconde après un coup reçu ; seul un coup plus fort passe, pour la différence.
- **Bouclier** : il pare un quart de seconde après avoir été levé, et renvoie un peu l'attaquant. Une **hache** l'abat : cinq secondes sans pouvoir le relever.
- **Armure** : elle retire une part des dégâts et amortit le recul ; elle s'use à chaque coup.

### Skin

**Paramètres → Skin** ouvre l'atelier où l'on dessine son personnage. Un skin est une image de 64 × 64 pixels, au format de Minecraft : un skin de Minecraft s'y importe tel quel.

**L'éditeur.** À gauche, le personnage (on le fait tourner en le saisissant) ; au milieu, la face que l'on peint, pixel par pixel ; à droite, les outils et les couleurs.

- On choisit une **partie** (tête, buste, bras, jambes) puis une **face** (devant, derrière, droite, gauche, dessus, dessous).
- Chaque partie a deux **couches** : la première est le corps, la seconde se pose par-dessus (cheveux qui dépassent, chapeau, veste, manches). Les pixels vides de la seconde laissent voir la première.
- **Pinceau**, **gomme** (elle n'efface que la seconde couche), **pot** (remplit une plage de même couleur), **pipette** (reprend une couleur du dessin), **miroir** (peint aussi le pixel symétrique), **annuler**.
- **Enregistrer** garde le skin (`saves/skin.png`) ; en partie, les autres joueurs le voient aussitôt. **Retour** sans enregistrer abandonne les changements.
- **Importer** lit le fichier `skin.png` posé dans le dossier du jeu ; **Exporter** l'y écrit. **Skin de départ** revient au voyageur d'origine.

**La bibliothèque.** L'autre onglet montre les skins que les joueurs ont publiés : on cherche par nom ou par auteur, on en choisit un, **Porter ce skin** le met sur son personnage (il reste à l'enregistrer dans l'éditeur). **Publier mon skin** partage le sien sous le nom que l'on donne ; il faut être connecté à son compte, donc avoir lancé le jeu depuis Epicube Hub. On retire ses propres skins avec **Retirer** (trente au plus par compte).

### Expérience et enchantements

Vaincre une créature ou miner un minerai (charbon, reiseki, diamant, cristal) donne de l'**expérience** : la barre verte au-dessus de la barre rapide, et le niveau en son milieu. La mort l'emporte.

**L'autel d'enchantement** (un cristal de roche, deux diamants, quatre obsidiennes) enchante un objet encore vierge. On y pose l'objet et des cristaux de roche ; il propose trois offres :

- chaque offre affiche le niveau qu'il faut avoir atteint, et coûte 1, 2 ou 3 niveaux et autant de cristaux selon son rang ;
- elle montre un enchantement ; d'autres peuvent venir avec, d'autant plus que l'offre est chère ;
- des **bibliothèques** (six planches, trois cuirs) posées à deux blocs de l'autel, à sa hauteur ou juste au-dessus, rendent les offres plus fortes : quinze suffisent pour les offres de niveau 30.

**L'enclume** (trois blocs de fer, quatre lingots) réunit deux objets pareils : leur usure s'ajoute, leurs enchantements se combinent, et deux niveaux égaux donnent le suivant. Elle coûte un niveau par niveau d'enchantement obtenu, plus un.

| Enchantement | Sur | Effet |
|---|---|---|
| Tranchant I-V | lames, haches | plus de dégâts |
| Recul I-II | lames, haches | repousse plus loin |
| Butin I-III | lames, haches | les créatures laissent plus de butin |
| Fauchage I-III | katana, naginata | le fauchage frappe plus fort les voisins |
| Efficacité I-V | outils | mine plus vite |
| Fortune I-III | outils | les minerais donnent plus |
| Toucher de soie | outils | le bloc tombe tel quel |
| Solidité I-III | tout ce qui s'use | s'use moins souvent |
| Raccommodage | tout ce qui s'use | l'expérience gagnée répare l'objet (jamais proposé par l'autel) |
| Protection I-IV | armure | 4 % de dégâts en moins par niveau porté |
| Chute amortie I-IV | sandales | chutes moins dures |
| Épines I-III | cuirasse | peut blesser qui vous frappe |
| Apnée I-III | casque | plus de souffle sous l'eau |
| Puissance I-V | arc | flèches plus fortes |
| Infinité | arc | les flèches ne sont pas dépensées |

Fortune et Toucher de soie ne vont pas ensemble, ni Infinité et Raccommodage. Un objet enchanté luit dans l'inventaire, et son infobulle liste ses enchantements.

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
