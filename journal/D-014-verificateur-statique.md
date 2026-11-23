# D-014 — Vérificateur statique : ce qu'on prouve, ce qu'on refuse

| Champ    | Valeur                          |
|----------|---------------------------------|
| Id       | D-014                           |
| Date     | 2026-11-23                      |
| Auteurs  | murx                            |
| Statut   | proposée                        |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

Bonus du README (§6) : garantir *avant toute exécution* qu'aucun saut n'atterrit hors du code ou sur une cible
invalide, et borner la hauteur maximale de pile.

Trois traits de `PBC v1` rendent le problème difficile :

- **Pas de table de destinations** (A.3) : une cible est valide si l'octet visé vaut `0x43`, y compris au milieu
  de l'opérande d'une autre instruction. Un balayage linéaire du code ne dit donc pas quelles instructions existent.
- **Sauts calculés** : `JMPX` et `CALLX` dépilent leur cible, qui n'est connue qu'à l'exécution.
- **Mémoire persistante** (B.1) : ce qu'on lit avec `LOAD` a été écrit par un tour précédent, donc on ne peut rien
  en savoir statiquement.

Le besoin : refuser avant de les servir les programmes qui fautent sur un saut ou sur la pile, et donner à l'arène
une borne de pile vérifiée.

## 2. Options envisagées

### Option A — Balayage linéaire, à la manière de l'EVM

- On décode tout le code depuis l'octet 0, puis on vérifie chaque saut statique.
- C'est faux pour `PBC v1` : cela refuse le saut dans un opérande, que la spec autorise (A.3, corpus 006), et cela
  examine du code inatteignable.

### Option B — Interprétation abstraite depuis `pc = 0`, hauteur de pile unique par adresse

- C'est le modèle des vérificateurs JVM et WASM : chaque adresse a une seule hauteur de pile, et deux chemins qui
  arrivent avec des hauteurs différentes sont refusés.
- C'est simple, mais cela refuse un `if/else` qui empile un nombre différent de valeurs avant un `HALT` commun.

### Option C — Interprétation abstraite, état = (`pc`, pile d'appels exacte, hauteur exacte) + ensembles de valeurs

- Seules les valeurs fusionnent entre chemins ; les hauteurs restent exactes. Un débordement est donc trouvé
  exactement là où le moteur le lèverait.
- Chaque case de pile porte un petit ensemble de valeurs connues (au plus 8), ou « n'importe quoi ». Cela suit une
  table de dispatch lue par `CONSTX` puis appelée par `CALLX` (corpus 210).
- Coût : plus d'états. On le borne par `MAX_SLOTS` (2¹⁸ cases au total) ; au-delà, le programme est refusé comme
  trop complexe.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) a proposé l'option C et l'a écrite dans `crates/vm/src/verify.rs`. Avant cela, il a extrait
les opérations pures du moteur dans `ops.rs`, pour que le vérificateur calcule les valeurs avec exactement les mêmes
fonctions que `run_turn`.

Ce qui a changé en route, à retranscrire fidèlement :

- **Sa première idée était de refuser tout `JMPX`/`CALLX`** dont la cible n'est pas une constante empilée juste
  avant. En désassemblant le corpus, il a vu que le cas nominal 210 appelle une adresse lue dans la table des
  constantes par un index capté (`SENSE`). Cette règle aurait refusé un programme nominal ; elle a été remplacée
  par les ensembles de valeurs.
- **Il a d'abord pensé à l'option B** (hauteur unique par adresse, refus si les hauteurs diffèrent). Il l'a écartée
  au profit de la hauteur dans la clé d'état, plus permissive et tout aussi bornée (la hauteur est dans `0..=64`).

Ce qui a été vérifié :

- 25 tests unitaires, une règle par test : saut dans un opérande accepté (A.3), code inatteignable ignoré,
  condition connue qui ne suit qu'un côté, table de dispatch, récursion non bornée, profondeur d'appel 16 acceptée
  et 17 refusée, chemin qui se termine sur une faute certaine, limite de complexité.
- **Correction (soundness), par test de propriété** : sur 200 000 programmes aléatoires, environ 55 000 sont
  acceptés. Chacun est exécuté 20 fois avec des mémoires, budgets et hôtes aléatoires : aucune exécution ne lève
  une faute exclue.
- **Corpus** : les 11 cas nominaux sont acceptés. Tous les cas dont l'exécution lève une faute exclue sont refusés,
  et pour la même faute. Les cas 016 et 026 sont aussi refusés, alors que leurs exécutions fautent sur `OutOfGas`.
  C'est attendu : avec plus de gaz, ces programmes sortiraient du code (016) ou déborderaient la pile (026).

## 4. Ce que nous avons décidé, et pourquoi

> *Brouillon proposé par l'assistant — à relire et reformuler par l'équipe avant de passer en « adoptée ».*

Option C. Un programme accepté ne peut lever aucune des 8 fautes de `EXCLUDED_FAULTS` (`PcOutOfRange`,
`BadOpcode`, `Truncated`, `BadJump`, `StackUnderflow`, `StackOverflow`, `CallStackOverflow`, `CallStackUnderflow`),
quels que soient sa mémoire, son budget et son hôte. Le vérificateur rend aussi la hauteur de pile et la
profondeur d'appel maximales.

Ce qu'il ne prouve pas, volontairement : les fautes qui dépendent des données ou du monde (`OutOfGas`,
`DivByZero`, `BadConst`, `OutOfBounds`, `BadSensor`, `BadAction`, `AlreadyActed`). Un chemin qui faute à coup sûr
(division par un zéro connu, `ACT` d'un code inconnu) s'arrête là : ce qui suit ne s'exécute jamais.

Ce qu'il refuse alors que ce serait peut-être sans danger (il est correct, pas complet) :

- une cible de `JMPX`/`CALLX` qu'il ne sait pas réduire à des valeurs connues ;
- une boucle dont la pile grandit à chaque tour, même si un compteur l'arrêterait à temps ;
- une récursion plus profonde que 16, même avec un cas de base ;
- un programme qui dépasse `MAX_SLOTS`.

C'est la leçon que le README annonce : sans saut structuré, la vérification ne peut être que conservatrice. WASM a
renoncé au saut libre et l'EVM a ajouté `JUMPDEST` pour cette raison.

## 5. Ce qui nous ferait changer d'avis

> *Brouillon — à compléter par l'équipe.*

- Si un agent réel est refusé pour `UnknownTarget` : suivre aussi les valeurs écrites en mémoire *pendant* le tour,
  avant toute lecture.
- Si `MAX_SLOTS` refuse des programmes réels : stocker les ensembles de valeurs de façon plus compacte, ou fusionner
  les états qui ne diffèrent que par la pile d'appels.
- Si l'arène veut refuser les agents non vérifiés : brancher `verify` au chargement, puis mesurer combien d'agents
  du jour d'interopérabilité passent.
