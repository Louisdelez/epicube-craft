# Sons d'Epicube Craft : sources et licences

Tous les fichiers de ce dossier viennent de banques publiees sous licence **CC0 1.0** (domaine public) :
ils peuvent etre utilises, modifies et redistribues sans condition. Les auteurs sont cites ici par courtoisie.
La licence de chaque page a ete relevee au moment du telechargement, le 27 novembre 2026.

| Banque | Auteur | Licence | Utilisation dans le jeu | Source |
|---|---|---|---|---|
| Kenney impact-sounds | Kenney | CC0 | pas, coups et casse des blocs, frappes, pose | https://kenney.nl/assets/impact-sounds |
| Kenney rpg-audio | Kenney | CC0 | coffres, bouton, piston, pieces, tissu, grincement | https://kenney.nl/assets/rpg-audio |
| Kenney interface-sounds | Kenney | CC0 | interface (clics, cases, fenetres), ramassage | https://kenney.nl/assets/interface-sounds |
| asianoriental1 | Tozan | CC0 | musique de la surface | https://opengameart.org/content/asianoriental1 |
| asianoriental2 | Tozan | CC0 | musique de la surface | https://opengameart.org/content/asianoriental2 |
| jaoan | Tozan | CC0 | musique de la surface | https://opengameart.org/content/jaoan |
| treacherous-path-loop | beardalaxy | CC0 | musique du menu | https://opengameart.org/content/treacherous-path-loop |
| hot-springs-town | Kistol | CC0 | musique de la surface | https://opengameart.org/content/hot-springs-town |
| zen-theme | beardalaxy | CC0 | musique de Takamagahara | https://opengameart.org/content/zen-theme |
| dark-zen | Tsorthan Grove | CC0 | musique de Yomi | https://opengameart.org/content/dark-zen |
| orient-peace-valley | Tozan | CC0 | musique de la surface | https://opengameart.org/content/orient-peace-valley |
| bosskoto | G_P | CC0 | musique du Ryujin | https://opengameart.org/content/bosskoto |
| death-of-a-ninja-game-over | Joth | CC0 | musique de la mort | https://opengameart.org/content/death-of-a-ninja-game-over |
| koto-short | Tozan | CC0 | musique de la surface | https://opengameart.org/content/koto-short |
| 80-cc0-creature-sfx | rubberduck | CC0 | voix des creatures | https://opengameart.org/content/80-cc0-creature-sfx |
| 40-cc0-water-splash-slime-sfx | rubberduck | CC0 | eclaboussures, eau, lave, sous l'eau | https://opengameart.org/content/40-cc0-water-splash-slime-sfx |
| 100-cc0-sfx | rubberduck | CC0 | explosion, levier, gong, squelette | https://opengameart.org/content/100-cc0-sfx |
| 100-cc0-sfx-2 | rubberduck | CC0 | pas dans l'eau, pierre cassee, tir, distributeur, ambiance de Yomi | https://opengameart.org/content/100-cc0-sfx-2 |
| mild-wind-background-noise | Bashar3A | CC0 | vent (extrait) | https://opengameart.org/content/mild-wind-background-noise |
| fire-crackling | AntumDeluge | CC0 | feu | https://opengameart.org/content/fire-crackling |
| loopable-dungeon-ambience | JaggedStone | CC0 | grottes | https://opengameart.org/content/loopable-dungeon-ambience |
| crickets-ambient-noise-loopable | Wolfgang_ | CC0 | nuit | https://opengameart.org/content/crickets-ambient-noise-loopable |
| 20-sword-sound-effects-attacks-and-clashes | StarNinjas | CC0 | lames et parades | https://opengameart.org/content/20-sword-sound-effects-attacks-and-clashes |
| 7-eating-crunches | StarNinjas | CC0 | manger | https://opengameart.org/content/7-eating-crunches |
| chunky-explosion | Joth | CC0 | explosion | https://opengameart.org/content/chunky-explosion |
| park-ambiences | Thimras | CC0 | oiseaux du jour (extrait) | https://opengameart.org/content/park-ambiences |

## Organisation

- `sfx/<nom>_<n>` : bruitages ; les variantes d'un meme nom sont tirees au hasard.
- `ambiance/<nom>` : boucles d'ambiance.
- `musique/<lieu>_<n>` : `menu`, `jour`, `ciel`, `yomi`, `boss`, `mort`.

Pour remplacer ou ajouter un son, deposez un fichier ogg, wav, mp3 ou flac en gardant le nom :
une variante de plus (`pas_herbe_6.ogg`) ou un morceau de plus (`jour_7.ogg`) sont pris en compte au lancement.

Les extraits `ambiance/jour.wav` et `ambiance/vent.wav` ont ete coupes dans les prises d'origine, passes en mono
et fermes en boucle par un fondu enchaine. Le tambour taiko des blocs a musique est fabrique par le programme.
