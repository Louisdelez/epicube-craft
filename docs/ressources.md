# Modifier les ressources

Textures, modèles et sons se modifient sans toucher au code.

## Textures et modèles

Par défaut, le jeu fabrique ses textures et ses modèles au démarrage. Pour les modifier :

1. **Paramètres → Exporter les ressources** (ou `epicube-craft --export-assets`). Le jeu écrit le dossier `assets/`.
2. Modifiez les fichiers.
3. **Paramètres → Recharger les ressources**, ou relancez le jeu.

Au lancement, les fichiers présents dans `assets/` remplacent ce que le jeu aurait fabriqué. Supprimez le dossier pour revenir à l'origine.

| Fichier | Contenu | Outil |
|---|---|---|
| `assets/textures/atlas.png` | toutes les textures, dans une image de 2048 × 2048 | éditeur d'images |
| `assets/models/<nom>.bbmodel` | créatures, joueur, chariot, villageois | [Blockbench](https://www.blockbench.net) |
| `assets/items/<numéro>_<nom>.bbmodel` | un modèle 3D par objet | Blockbench |

### L'atlas

Les blocs occupent des tuiles de 64 × 64 pixels, dessinées en 32 × 32 puis doublées. Le filtrage est au plus proche voisin : gardez des pixels francs. Chaque modèle de créature a sa case de 256 × 256 ; chaque objet, une case de 96 × 96.

### Les modèles

- Les fichiers `.bbmodel` embarquent leur texture ; elle est recopiée dans l'atlas au chargement.
- Seuls les cubes sont lus, avec leur rotation et leur pivot.
- Créatures : un cube dont le nom commence par `jambe_a` ou `jambe_b` se balance en marchant, autour de son pivot.
- Objets : le numéro en tête du nom de fichier désigne l'objet, ne le changez pas. **L'icône d'inventaire est redessinée d'après le modèle** : ce qu'on voit dans les cases est ce qu'on tient en main.
- Les blocs minces (rails, pousses, lanternes) n'ont pas de modèle : leur dessin de l'atlas reçoit un peu d'épaisseur.

## Sons

Le dossier `audio/` contient trois sous-dossiers. Les formats acceptés sont ogg, wav, mp3 et flac.

| Dossier | Règle de nommage |
|---|---|
| `audio/sfx/` | `<nom>_<n>` : les variantes d'un même nom sont tirées au hasard (`pas_herbe_1.ogg`, `pas_herbe_2.ogg`...) |
| `audio/ambiance/` | `<nom>` : boucles d'ambiance (`jour`, `nuit`, `grotte`, `vent`, `yomi`, `sous_eau`, `feu`, `eau`, `lave`) |
| `audio/musique/` | `<lieu>_<n>` : `menu`, `jour`, `ciel`, `yomi`, `boss`, `mort` |

Pour remplacer un son, déposez un fichier du même nom. Pour en ajouter, déposez une variante de plus (`pas_herbe_6.ogg`) ou un morceau de plus (`jour_7.ogg`) : ils sont pris en compte au lancement.

Les sources et licences des sons fournis sont dans [audio/CREDITS.md](../audio/CREDITS.md). Si vous redistribuez le jeu avec d'autres sons, vérifiez leur licence.

## Limites connues

- L'atlas est un fichier unique : deux modifications faites séparément ne se fusionnent pas toutes seules.
- Les sons d'un événement donné ne se choisissent pas par un fichier de configuration : c'est le nom du fichier qui fait le lien.
