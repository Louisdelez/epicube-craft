# Comptes des joueurs

Le jeu sait reconnaître un joueur par son **compte**, sans jamais voir son mot de passe. Ce document décrit ce que le jeu attend, pour y brancher le service de comptes et le launcher, qui restent à écrire.

## Qui fait quoi

| Élément | Rôle | État |
|---|---|---|
| **Service de comptes** | inscription, connexion, mots de passe ; signe les jetons | à écrire |
| **Launcher** | connecte le joueur, obtient un jeton, lance le jeu | à écrire |
| **Jeu** (client) | reçoit le jeton du launcher, le présente aux serveurs, relève leur défi | fait |
| **Serveur de jeu** | vérifie le jeton et le défi, range la sauvegarde sous l'identifiant du compte | fait |

Le serveur de jeu ne contacte jamais le service de comptes : il lui suffit de sa **clé publique**. N'importe qui peut donc héberger un serveur sans détenir de secret, et une panne du service n'empêche pas de jouer.

## Déroulement d'une connexion

1. Le launcher crée une **paire de clés de session** (Ed25519), neuve à chaque connexion au compte.
2. Il envoie identifiant, mot de passe et **clé publique de session** au service de comptes.
3. Le service répond par un **jeton** signé, qui contient l'identifiant du compte, le pseudo, la date de fin et la clé publique de session.
4. Le launcher lance le jeu en lui remettant le jeton et la **clé privée de session**.
5. Le jeu présente le jeton au serveur de jeu.
6. Le serveur vérifie la signature et la date, puis envoie un **défi** : 32 octets tirés au hasard.
7. Le jeu signe le défi avec la clé privée de session. Le serveur vérifie cette signature avec la clé inscrite dans le jeton, et laisse entrer.

Un jeton intercepté ne sert à rien sans la clé privée de session, qui ne quitte jamais la machine du joueur.

## Le jeton

Un **JWT** signé en **EdDSA (Ed25519)**, forme compacte `en-tête.contenu.signature`, chaque partie en base64url sans remplissage. Toute bibliothèque JWT sait le produire.

En-tête :

```json
{"alg":"EdDSA","typ":"JWT"}
```

Contenu :

```json
{
  "sub": "0190a8e2-7d5c-4b1e-9f3a-5c2d8e4f6a1b",
  "name": "Hanako",
  "exp": 1795000000,
  "key": "q0Yv…43 caractères…"
}
```

| Champ | Sens | Règles |
|---|---|---|
| `sub` | identifiant du compte, un UUID | fixe pour toujours ; **jamais de version 3** (réservée aux joueurs hors ligne) : utiliser la version 4 ou 7 |
| `name` | pseudo affiché | 1 à 16 caractères, sans caractère de contrôle ; unique parmi les comptes ; le joueur peut en changer |
| `exp` | fin de validité, en secondes depuis 1970 | 24 h conseillées ; le serveur tolère 2 minutes d'écart d'horloge |
| `key` | clé publique de session, 32 octets en base64url | celle que le launcher a fournie |

Les autres champs sont ignorés. Le serveur n'accepte que `alg` = `EdDSA`.

## Du launcher au jeu

Le launcher lance l'exécutable avec deux variables d'environnement :

| Variable | Contenu |
|---|---|
| `EPICUBE_JETON` | le jeton |
| `EPICUBE_CLE_SESSION` | la clé privée de session : sa graine de 32 octets, en base64url (ou 64 chiffres hexadécimaux) |

Des variables plutôt que des arguments : les arguments d'un programme sont visibles des autres programmes de la machine.

Sans ces variables, le jeu démarre **hors ligne**, sous le pseudo des paramètres. Avec elles, le pseudo est celui du compte et ne se saisit plus ; l'écran titre affiche « Compte : … ».

Le jeton sert à toutes les connexions jusqu'à sa date de fin. Pour que le joueur puisse jouer en solo sans réseau, le launcher garde le dernier jeton et sa clé : **la partie solo accepte un jeton périmé**, pas les serveurs en ligne.

## Le défi

Le serveur envoie `Challenge { nonce }` (32 octets). Le jeu répond `Proof { address, sig }`, où `sig` est la signature Ed25519, par la clé de session, de :

```
"EpicubeCraft preuve 1\n"  +  nonce (32 octets)  +  adresse
```

`adresse` est l'adresse du serveur telle que le joueur l'a composée, en minuscules, port toujours écrit (`jeu.exemple.fr:25600`). Un serveur malveillant qui relaierait la connexion vers un autre serveur obtiendrait une signature pour sa propre adresse, inutilisable ailleurs — à condition que le serveur visé ait déclaré son adresse (voir ci-dessous).

## Régler un serveur

Le serveur dédié lit `comptes.txt`, à côté de lui ; il le crée avec ses explications au premier démarrage.

```
comptes=obligatoires
cle=j-wNG0J__1F4W-_hxpfP4qAyD-KQoVoxigSsld0YmyY
adresse=jeu.exemple.fr:25600
```

| Ligne | Effet |
|---|---|
| `comptes=facultatifs` (par défaut) | tout le monde entre ; un joueur sans compte est reconnu à son seul pseudo |
| `comptes=obligatoires` | seuls les comptes vérifiés entrent : plus besoin de `/register` ni de `/login` |
| `cle=` | clé publique du service de comptes, en base64url ou en hexadécimal |
| `adresse=` | adresse publique du serveur ; une ligne par adresse. Renseignée, elle bloque les connexions relayées |

L'option `--comptes obligatoires` du serveur a priorité sur le fichier. La clé peut aussi venir de la variable `EPICUBE_CLE_COMPTES`. Quand le service existera, sa clé publique s'inscrira dans le jeu (`SERVICE_KEY`, dans `shared/src/account.rs`) et plus personne n'aura à la saisir.

En mode facultatif :

- un joueur sans compte dont le pseudo est déjà pris reçoit un pseudo distinct ;
- si le propriétaire d'un pseudo se connecte avec son compte, le joueur sans compte qui le portait est déconnecté ;
- un jeton invalide est toujours refusé, même si le compte n'est pas exigé.

Un même compte ne peut être connecté qu'une fois à un serveur.

## Identifiants et sauvegardes

Chaque joueur a un identifiant de 16 octets :

- avec un compte, celui du jeton ;
- sans compte, un identifiant **hors ligne** calculé à partir du pseudo (UUID de version 3).

La sauvegarde d'un joueur est rangée sous cet identifiant : `saves/<monde>/joueurs/<uuid>.bin`. Changer de pseudo ne fait donc rien perdre. Les sauvegardes d'avant les comptes, rangées sous le pseudo, sont reprises à la première connexion du joueur qui porte ce pseudo.

Le serveur annonce l'identifiant de chaque joueur aux autres (`PlayerJoin`). Amis, messages, cosmétiques et préférences en ligne pourront s'y rattacher sans changer le protocole de connexion.

## Essayer sans le service

L'outil `epicube-craft-jeton` joue le rôle du service et du launcher :

```bash
cargo run -p server --bin epicube-craft-jeton -- cles
```

```bash
cargo run -p server --bin epicube-craft-jeton -- jeton --nom Hanako
```

La première commande crée une clé de service d'essai (`service-essai.cle`) et affiche la clé publique à mettre dans `comptes.txt`. La seconde affiche les deux variables à fournir au jeu. Options : `--id <uuid>`, `--heures <n>`, `--cle <fichier>`.

Ces clés d'essai ne doivent jamais servir à de vrais joueurs.

## Limites connues

- La connexion entre le jeu et le serveur n'est **pas chiffrée** : le jeton y passe en clair (sans danger, grâce à la clé de session), mais le chat et le reste de la partie aussi.
- Un serveur qui n'a pas déclaré son `adresse` ne peut pas détecter une connexion relayée par un serveur malveillant.
- Un jeton ne peut pas être révoqué avant sa date de fin : un compte banni du service peut encore jouer jusqu'à 24 h. Les bannissements par serveur restent à faire.
- La partie solo ouverte au réseau local accepte aussi les jetons périmés des autres joueurs.
- Le jeton et la clé de session sont lisibles par les programmes lancés par le jeu lui-même.
