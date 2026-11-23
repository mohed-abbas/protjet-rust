# D-017 — Ordre des vérifications là où la spec ne tranche pas

| Champ    | Valeur                          |
|----------|---------------------------------|
| Id       | D-017                           |
| Date     | 2026-11-23                      |
| Auteurs  | murx                            |
| Statut   | proposée                        |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

B.2 fixe l'ordre des étapes d'un pas (décodage, gaz, exécution), mais pas l'ordre des vérifications **à
l'intérieur** de l'étape 5 quand plusieurs fautes sont possibles en même temps. Or la chaîne de faute (B.5) et la
suite des rappels (K.5) sont normatives : deux machines qui choisissent un ordre différent divergent, et le jour
d'interopérabilité le montrera.

Le corpus tranche certains cas :

| Cas    | Situation                        | Attendu          |
|--------|----------------------------------|------------------|
| 047b   | `CONSTX` sur pile vide           | `StackUnderflow` |
| 054b   | `CALLX` sur pile vide            | `StackUnderflow`, avant la pile d'appels |
| 063    | `STORE k` sur pile vide          | `StackUnderflow` |
| 046b   | `ROT` avec deux valeurs          | `StackUnderflow` |
| 032    | deuxième `ACT`                   | `AlreadyActed`, l'action du premier reste consignée |

Restent ouverts : `CONST k` invalide sur pile pleine ; `SENSE`/`RAND` sur pile pleine (rappel ou pas ?) ; `ACT k`
avec `k` invalide **et** déjà agi ; `CALL` vers une mauvaise cible avec la pile d'appels pleine.

## 2. Options envisagées

### Option A — « La pile d'abord » : dépiler les opérandes, vérifier la place pour le résultat, puis le reste

- Une seule règle, qui prolonge ce que le corpus montre déjà (la pile avant l'index, avant la cible).
- Aucun rappel n'est posé pour une instruction qui ne pourra pas empiler la réponse.

### Option B — Suivre l'ordre « naturel » du code : calculer, puis empiler

- `CONST 9` sur pile pleine donnerait `BadConst` ; `SENSE` sur pile pleine poserait le rappel **puis** lèverait
  `StackOverflow` — un aller-retour réseau pour rien, et un transcript qui contient une question sans effet.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) a dérivé les cas tranchés en désassemblant le corpus, puis a proposé l'option A, appliquée ainsi :

- `CONST k`, `LOAD k`, `GAS`, `SENSE`, `RAND` : place sur la pile vérifiée en premier → `StackOverflow`, sans rappel ;
- `ACT k` : dépiler l'argument (`StackUnderflow`) → `k` hors de `0..=3` (`BadAction`, rien consigné) → déjà agi
  (`AlreadyActed`) → rappel → empiler l'issue (ne peut pas déborder, E.4) ;
- `CALL d` : empiler l'adresse de retour (`CallStackOverflow`) → vérifier la cible (`BadJump`) ;
- `CALLX` : dépiler la cible (`StackUnderflow`) → empiler l'adresse de retour → vérifier la cible ;
- `STOREX` : dépiler `addr` puis `v` → vérifier `addr` (`OutOfBounds`).

Il a signalé que l'ordre `BadAction` avant `AlreadyActed`, et `CallStackOverflow` avant `BadJump`, sont des choix
sans appui dans la spec ni le corpus. Chaque choix a un test unitaire nommé (`const_checks_the_room_before_the_index`,
`every_pushing_instruction_overflows_on_a_full_stack`, …).

## 4. Ce que nous avons décidé, et pourquoi

> *Brouillon proposé par l'assistant — à relire et reformuler par l'équipe avant de passer en « adoptée ».*

Option A : une règle unique est plus facile à expliquer et à comparer avec les autres équipes qu'une liste de cas, et
elle évite des rappels inutiles. À confronter au jour d'interopérabilité.

## 5. Ce qui nous ferait changer d'avis

> *Brouillon — à compléter par l'équipe.*

- Une nouvelle version du corpus ou une clarification de la spec qui tranche un de ces cas autrement.
- Une majorité d'implémentations d'autres équipes, au jour d'interopérabilité, qui fait l'autre choix sur un cas
  sans appui normatif : s'aligner coûte moins que diverger.
