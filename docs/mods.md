# Mods

Un mod ajoute des choses au jeu : blocs, objets, outils, nourriture, recettes, minerais semés dans le monde, créatures, commandes, règles, fenêtres et sons. Il n'y a qu'une sorte de mod, valable pour le solo comme pour les serveurs.

- **Sur un serveur**, les mods sont ceux du serveur. Le joueur qui le rejoint les reçoit automatiquement, par la connexion du jeu, avant d'entrer. Il n'a rien à installer.
- **En solo**, les mods sont ceux que le joueur a installés depuis **Paramètres → Mods**.

## Installer des mods

| Où | Comment |
|---|---|
| Dans le jeu | **Paramètres → Mods** : la bibliothèque d'Epicube Hub. *Installer*, *Mettre à jour*, *Supprimer*. |
| À la main | Déposer l'archive `.zip` du mod (ou son dossier) dans le dossier `mods`, à côté du jeu. |
| Sur un serveur dédié | Déposer les mods dans le dossier `mods`, à côté de `epicube-craft-server`, puis redémarrer le serveur. |

Les mods reçus d'un serveur sont gardés dans `mods/cache` : ils ne sont pas retéléchargés la fois suivante, et ne s'appliquent pas au solo.

## Deux façons d'écrire un mod

| | Lua | Rust |
|---|---|---|
| Pour | les petits mods : quelques commandes, quelques règles | les gros mods : beaucoup de logique, de calcul, de données |
| À faire | écrire un fichier texte | compiler en WebAssembly |
| Vitesse | interprété | proche du code natif |
| Mémoire | 64 Mo | 512 Mo |

Les deux ont exactement les mêmes possibilités, et un même mod peut mélanger les deux (un script serveur en Rust, un script d'écran en Lua).

## Contenu d'un mod

```text
mon-mod/
  mod.json        identité du mod
  serveur.lua     ce qu'il fait dans le monde          (ou serveur.wasm)
  client.lua      ce qu'il fait à l'écran du joueur    (ou client.wasm)
  textures/       images PNG : blocs, objets, images de ses fenêtres
  sons/           bruitages (ogg, wav, mp3, flac)
  modeles/        modèles Blockbench de ses créatures
```

`mod.json` :

```json
{
  "id": "mon-mod",
  "name": "Mon mod",
  "version": "1.0.0",
  "description": "Ce que fait le mod, en une phrase.",
  "author": "Moi"
}
```

`id` : minuscules, chiffres et tirets, de 2 à 32 caractères. Un mod pèse 32 Mo au plus.

Pendant qu'on écrit un mod, on laisse son dossier tel quel dans `mods` : le jeu le lit directement. Les autres dossiers qu'il contient (sources Rust, compilation) ne sont pas envoyés aux joueurs.

## Un mod en Lua

`serveur.lua` :

```lua
local rubis = blocks.register{
  id = "rubis", name = "Bloc de rubis",
  texture = "rubis.png", hardness = 3, light = 9, tool = "pioche",
}

on("join", function(player)
  player:tell("Bienvenue " .. player.name .. " !")
end)

command("rubis", "Donne des blocs de rubis", function(player, args)
  player:give(rubis, tonumber(args) or 8)
  return "Voilà."
end)

on("break", function(player, x, y, z, block)
  if block == "pierre" then return false end   -- refuse : le bloc reste
end)
```

Un exemple complet : [`exemples/mods/bienvenue`](../exemples/mods/bienvenue).

### Côté serveur (`serveur.lua`)

| Événement | Reçoit | Renvoyer `false` |
|---|---|---|
| `join`, `leave` | le joueur | |
| `chat` | le joueur, le texte | le message n'est pas diffusé |
| `break` | le joueur, x, y, z, le bloc | le bloc reste en place |
| `place` | le joueur, x, y, z, le bloc | le bloc n'est pas posé |
| `interact` | le joueur, x, y, z, le bloc (clic droit sur un bloc) | le jeu n'en fait rien |
| `use` | le joueur, l'objet tenu (clic droit) | le jeu n'en fait rien |
| `hit` | le joueur qui frappe, le pseudo de celui qu'il frappe | le coup n'est pas porté |
| `death` | le joueur, le pseudo de celui qui l'a tué (ou vide) | |
| `respawn` | le joueur | |
| `mob_death` | la créature, x, y, z | |
| `message` | le joueur, un nom, un texte | (message du script d'écran) |
| `tick` | rien, 20 fois par seconde | |

| Fonction | Rôle |
|---|---|
| `on(événement, fonction)` | écoute un événement |
| `command(nom, aide, fonction)` | ajoute la commande `/nom` ; la fonction reçoit le joueur et le reste de la ligne, et ce qu'elle renvoie lui est affiché |
| `blocks.register{...}` | ajoute un bloc ; renvoie son nom complet, `mod:bloc` |
| `items.register{...}` | ajoute un objet ; renvoie son nom complet, `mod:objet` |
| `recipes.shaped{...}`, `recipes.smelt{...}` | ajoute une recette d'artisanat, une cuisson au four |
| `worldgen.ore{...}` | sème un minerai dans la roche du monde |
| `mobs.register{...}` | ajoute une créature ; renvoie son nom complet, `mod:creature` |
| `say(texte)` | écrit dans le chat de tous |
| `announce(texte)` | affiche un message au centre de l'écran de tous |
| `rules.set(règle, oui)` | règle de la partie : `"pvp"` (combat entre joueurs), `"regen"` (la vie remonte seule), `"mobs"` (les créatures naissent d'elles-mêmes) |
| `players()` | les joueurs connectés |
| `world.get(x, y, z)`, `world.set(x, y, z, bloc)` | lit et pose un bloc |
| `world.time()`, `world.set_time(heure)` | heure du jour, de 0 à 24 |
| `world.spawn(créature, x, y, z)` | fait naître une créature du jeu (`cerf`, `oni`...) ou d'un mod |
| `world.drop(objet, nombre, x, y, z)` | laisse tomber des objets |
| `world.explode(x, y, z, puissance)` | explosion |
| `world.sound(son, x, y, z, volume)` | bruitage entendu autour d'un endroit |
| `world.top(x, z)` | hauteur du bloc le plus haut de la colonne |
| `world.fill(x1, y1, z1, x2, y2, z2, bloc)` | remplit un pavé (200 000 blocs au plus) |
| `world.chest(x, y, z, objet, nombre)` | range des objets dans le coffre posé là (il est posé s'il n'y en a pas), dans une case libre au hasard |
| `store.get(clé)`, `store.set(clé, valeur)` | mémoire du mod, enregistrée avec le monde |
| `print(...)` | écrit dans le journal du serveur |

Un joueur : `player.name`, `player.id`, `player:tell(texte)`, `player:give(objet, nombre)`, `player:pos()`, `player:teleport(x, y, z)`, `player:health()`, `player:set_health(vie)`, `player:mode()`, `player:kick(raison)`, `player:send(nom, texte)` (vers le script d'écran), `player:online()`, `player:count(objet)`, `player:take(objet, nombre)`, `player:held()`, `player:set_mode(mode)`, `player:hunger()`, `player:set_hunger(faim)`, `player:dimension()`, `player:sound(son, volume)`, `player:damage(points)`, `player:set_home(x, y, z)` (où il revient après sa mort), `player:clear()` (vide son inventaire), `player:info(texte)` (message au centre de son écran), `player:enchant(enchantement, niveau)` (enchante l'objet qu'il tient : `tranchant`, `solidite`...).

Le monde se charge de lui-même là où un script lit ou écrit : un mod peut bâtir une arène loin de tout joueur.

Un bloc ou un objet se désigne par son mot dans la commande `/donner` (`pierre`, `planches`, `diamant`, `baton`...), ou par `mod:nom` pour ce qu'un mod ajoute (le nom seul suffit à l'intérieur du même mod). `air` retire un bloc. Un son se désigne par son nom dans le dossier `audio/sfx` du jeu, ou par `mod:fichier` pour un son du dossier `sons` d'un mod.

Un bloc (`blocks.register`) :

| Champ | Rôle | Par défaut |
|---|---|---|
| `id` | identifiant | obligatoire |
| `name` | nom affiché | l'identifiant |
| `texture` | image du dossier `textures`, ou `{ top = ..., side = ..., bottom = ... }` | `<id>.png` |
| `hardness` | dureté : 1 terre, 2 bois, 3 minerai ; négative = incassable | 1 |
| `light` | lumière émise, de 0 à 15 | 0 |
| `tool` | `"pioche"`, `"hache"` ou `"pelle"` | aucun |
| `transparent` | laisse passer la lumière | non |
| `drop` | objet laissé quand on le casse ; `false` = rien | le bloc |
| `shape` | `"plante"` : deux plans croisés, que l'on traverse | un cube |

Un objet (`items.register`) :

| Champ | Rôle | Par défaut |
|---|---|---|
| `id`, `name`, `texture` | comme pour un bloc | |
| `stack` | taille d'une pile | 64 |
| `food`, `saturation` | un aliment : faim rendue, réserve | ne se mange pas |
| `fuel` | un combustible : secondes de feu | 0 |
| `tool`, `tier`, `durability` | un outil : `"pioche"`, `"hache"`, `"pelle"`, `"epee"`, `"houe"` ; niveau de 0 (bois) à 4 ; nombre d'usages | pas un outil |
| `damage` | dégâts d'une arme | ceux de l'outil |

Recettes, cuisson et minerai :

```lua
recipes.shaped{ output = "pioche", count = 1,
  pattern = { "GGG", " B ", " B " }, key = { G = "gemme", B = "baton" } }
recipes.smelt{ input = "minerai", output = "gemme" }
worldgen.ore{ block = "minerai", veins = 5, size = 6, min_y = 4, max_y = 90 }   -- dimension = 1 : Yomi, 2 : Takamagahara
```

Une créature reprend le comportement et la silhouette d'une créature du jeu, avec ses propres caractéristiques :

```lua
mobs.register{ id = "golem", name = "Golem de rubis", base = "oni",
  health = 60, damage = 7, scale = 1.4, color = { 1, 0.45, 0.5 },
  rarity = 0.05,                                   -- un oni sur vingt naît golem
  model = "golem.bbmodel",                         -- facultatif : son propre modèle
  loot = { { item = "gemme", min = 2, max = 5 } } }
```

`model` est un fichier [Blockbench](https://www.blockbench.net) du dossier `modeles` du mod, avec sa texture incorporée (256 × 256 pixels au plus). Le plus simple est de partir d'un modèle du jeu : **Paramètres → Exporter les ressources** les écrit dans `assets/models`. Sans `model`, la créature garde la silhouette de sa base.

`base` : `cerf`, `sanglier`, `tanuki`, `grue`, `oni`, `squelette`, `araignee`, `lanterne`, `yurei` ou `wanyudo`.

Les textures sont des PNG carrés (16, 32 ou 64 pixels de côté). Une partie accepte 96 blocs, 1000 objets, 250 créatures (dont 16 avec leur propre modèle) et 768 textures de mods. Les numéros donnés aux blocs et aux objets sont retenus avec le monde : un bloc posé garde son sens d'une partie à l'autre.

### Côté client (`client.lua`)

| Événement | Reçoit |
|---|---|
| `message` | un nom, un texte (message du script serveur) |
| `hud` | rien, à chaque image : c'est là qu'on dessine |
| `key` | la touche enfoncée (`KeyG`, `F6`...) |
| `click` | x, y, bouton (0 gauche, 1 droit), quand la fenêtre du mod est ouverte |
| `text` | le texte tapé, quand la fenêtre du mod est ouverte |
| `close` | rien : la fenêtre vient d'être fermée par `Échap` |

| Fonction | Rôle |
|---|---|
| `send(nom, texte)` | envoie un message au script serveur |
| `chat(texte)` | écrit une ligne dans le chat du joueur |
| `player.pos()` | position du joueur |
| `draw.text(x, y, texte, r, v, b, a)` | écrit à l'écran |
| `draw.rect(x, y, largeur, hauteur, r, v, b, a)` | dessine un rectangle |
| `draw.image(x, y, largeur, hauteur, fichier, r, v, b, a)` | dessine une image du dossier `textures` |
| `draw.icon(x, y, taille, objet)` | dessine l'icône d'un objet ou d'un bloc |
| `draw.screen()`, `draw.text_width(texte)` | taille de l'écran, largeur d'un texte |
| `screen.open()`, `screen.close()` | ouvre une fenêtre : la souris est libérée, le jeu ne reçoit plus les gestes |
| `screen.mouse()` | position de la souris |
| `sound.play(son, volume)` | joue un bruitage |

Les mesures sont en points d'interface (l'écran en compte environ 640 × 360), les couleurs de 0 à 1.

## Un mod en Rust

Il faut Rust et sa cible WebAssembly :

```bash
rustup target add wasm32-unknown-unknown
```

`Cargo.toml` :

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
epicube-mod = { git = "https://github.com/Louisdelez/epicube-craft" }
```

`src/lib.rs` :

```rust
use epicube_mod::server::*;

epicube_mod::start!(init);

fn init() {
    let rubis = blocks::register(
        Block::new("rubis").name("Bloc de rubis").texture("rubis.png").hardness(3.0).light(9).tool(Tool::Pick),
    );
    on_join(|p| p.tell("Ce serveur a le mod Rubis : tapez /rubis."));
    command("rubis", "Donne des blocs de rubis", move |p, args| {
        p.give(&rubis, args.trim().parse().unwrap_or(8));
        Some("Voilà.".into())
    });
}
```

```bash
cargo build --release --target wasm32-unknown-unknown
```

Le fichier `.wasm` obtenu se range dans le dossier du mod sous le nom `serveur.wasm`. Un script d'écran s'écrit de la même façon avec `epicube_mod::client::*` et se range sous le nom `client.wasm`.

Les fonctions sont celles du Lua, sous forme Rust : `on_join`, `on_break`, `on_use`, `on_mob_death`..., `command`, `blocks::register(Block::new(...))`, `items::register(Item::new(...))`, `recipes::shaped`, `recipes::smelt`, `worldgen::ore(Ore::new(...))`, `mobs::register(Mob::new(...))`, `world::*`, `store::*`, et les méthodes de `Player`. Côté client : `on_hud`, `on_key`, `on_click`, `on_message`, `send`, `sound`, `screen::open`, `draw::text`, `draw::rect`, `draw::image`, `draw::icon`. La documentation du kit (`cargo doc -p epicube-mod --open`) les décrit toutes.

Un gros exemple, qui utilise tout : [`exemples/mods/rubis`](../exemples/mods/rubis) — minerai semé dans le monde, gemme, pioche, lame, tarte, recettes, cuisson, fleur, golem, compteur à l'écran, boutique en jeu (touche `G`) avec images, icônes et son. `sh construire.sh` le compile.

## Modes de jeu

Un mode de jeu (UHC, Bed Wars...) est un mod dont le `mod.json` porte une rubrique `mode`. Il apparaît alors à l'écran **Héberger une partie**, et les parties de ce mode forment une catégorie de l'écran **Multijoueur**.

```json
{
  "id": "uhc", "name": "UHC", "version": "1.0.0",
  "mode": {
    "name": "UHC",
    "about": "La vie ne remonte pas toute seule, la zone rétrécit, le dernier en vie gagne.",
    "tags": ["pvp", "survie"],
    "players": 16,
    "game": "aventure",
    "keep_world": false
  }
}
```

| Champ | Rôle |
|---|---|
| `name`, `about` | nom et phrase de présentation |
| `tags` | étiquettes des parties de ce mode dans la liste |
| `players` | nombre de places proposé |
| `game` | mode donné aux joueurs qui arrivent : `survie`, `creatif`, `aventure` ou `spectateur` |
| `keep_world` | `true` : le monde est gardé d'une partie à l'autre ; sinon chaque partie commence dans un monde neuf |

### La base d'un mini-jeu

`minigame{...}` tient pour vous l'attente des joueurs, le compte à rebours, les équipes, qui est encore en jeu, l'état affiché à l'écran, la commande `/<id> demarrer` et la victoire. Le jeu n'écrit que ce qui lui est propre :

```lua
minigame{ id = "pitchout", name = "PitchOut", min = 2, void = 14,   -- void : tomber de 14 blocs sous l'arène tue
  teams = { "Rouge", "Bleue" },                 -- facultatif : sans équipes, chacun pour soi
  build = function(g) world.fill(g.ox - 9, g.oy, g.oz - 9, g.ox + 9, g.oy, g.oz + 9, "pierre") end,
  spawn = function(g, p, i, team) p:teleport(g.ox + 0.5, g.oy + 1.2, g.oz + 0.5) end,
  second = function(g, clock) end,              -- chaque seconde de la partie ; tick = à chaque tick
  death = function(g, p, killer) return true end,  -- vrai : le joueur reste en jeu et reviendra
  respawn = function(g, p) end,
  status = function(g, p) return g.count .. " en jeu" end }
```

`g` donne `g.ox, g.oy, g.oz` (le centre de l'arène), `g.clock`, `g.count`, `g.kills`, `g:players()`, `g:alive(p)`, `g:team(p)`, `g:mates(n)`, `g:ring(i, rayon)`, `g:roll(n)`, `g:out(p)` et `g:win(texte)`. Autres réglages : `countdown`, `height`, `mode`, `pvp = false`, `endless = true` (pas de victoire au dernier debout), `limit` et `timeout`, `places`. Les jeux du dossier `modes` en sont autant d'exemples.


Le script du mode fait le reste avec les fonctions de cette page : attendre les joueurs, lancer la partie, fixer les règles (`rules.set`), bâtir une arène (`world.fill`), compter les survivants. Les deux modes livrés avec le jeu servent d'exemples : [`modes/uhc`](../modes/uhc) et [`modes/bedwars`](../modes/bedwars), chacun en un seul fichier Lua.

Un mode de jeu installé depuis la bibliothèque (dossier `mods`) s'héberge comme ceux du jeu.

## Publier un mod dans la bibliothèque

Avec l'outil de publication d'Epicube Hub, depuis le dossier du mod ou son archive :

```bash
epicube-hub-publish --server https://epicube.ch mod chemin/vers/mon-mod
```

Publier le même mod avec une nouvelle version le met à jour ; les joueurs voient *Mettre à jour*.

## Sécurité

Un mod reçu d'un serveur s'exécute sur l'ordinateur du joueur : il tourne donc dans un **bac à sable**. Il n'a accès ni aux fichiers, ni au réseau, ni au système ; il ne peut agir que par les fonctions de cette page. Sa mémoire est bornée, et un script qui met trop de temps à répondre est interrompu (100 ms par événement côté serveur, 20 ms côté client). L'archive reçue est vérifiée par son empreinte SHA-256 avant d'être lue.

Un mod qui échoue est signalé dans le journal (`[mod nom] ...`) et le jeu continue.

## Limites actuelles

- Une créature de mod a le comportement d'une créature du jeu : elle n'a pas sa propre intelligence, et n'est pas enregistrée à l'arrêt du serveur.
- Un bloc de mod est un cube ou une plante ; il n'ouvre pas de fenêtre de rangement comme un coffre.
