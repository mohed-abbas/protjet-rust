# D-002 — Où vivent les fautes : `vm::Fault` et `pbc::LoadError`

| Champ    | Valeur                          |
|----------|---------------------------------|
| Id       | D-002                           |
| Date     | 2026-11-23                      |
| Auteurs  | murx                            |
| Statut   | proposée                        |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

B.5 donne une liste **close** de seize chaînes de faute. La première, `BadHeader`, est une erreur de chargement :
« un programme qui ne charge pas n'est pas exécuté » (B.5, K.8). Les quinze autres ne surviennent que pendant un tour.
Les chaînes sont normatives : elles apparaissent telles quelles dans `RESULT` (K.3) et dans les comptes-rendus.

Le chargeur vit dans `pbc`, le moteur dans `vm`, et `pbc` ne dépend de rien (D-001). Il faut donc décider où mettre un
type qui couvre les deux.

## 2. Options envisagées

### Option A — Un seul `enum Fault` à seize variantes, dans `pbc`

- Une seule liste, calquée sur B.5.
- `vm` devrait traiter `BadHeader` dans chaque `match` alors qu'un tour ne peut jamais le produire : une variante
  inatteignable, que le type n'empêche pas.
- `pbc` porterait des notions d'exécution (`OutOfGas`, `AlreadyActed`) qui ne le concernent pas.

### Option B — Deux types : `pbc::LoadError` (chargement) et `vm::Fault` (quinze fautes d'exécution)

- Chaque type ne contient que ce qui peut réellement arriver à son étage. `TurnResult::fault` ne peut pas valoir
  `BadHeader`, par construction.
- `LoadError` peut être plus précis que la spec (`BadMagic`, `TrailingBytes`…) pour nos messages, tout en se
  rapportant sous la seule chaîne `BadHeader` (`LoadError::FAULT_NAME`).
- La liste de B.5 est répartie sur deux fichiers ; un test doit garantir qu'on n'en perd pas une.

### Option C — `Fault` à seize variantes dans un crate commun `palestre-types`

- Même inconvénient que A pour `vm`, plus un sixième crate.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) avait signalé la question dans D-001 sans la trancher. Il a ensuite proposé l'option B et l'a
écrite :

- `vm::Fault` : quinze variantes, `Fault::ALL`, `as_str()` (chaîne exacte de B.5), `from_name()`, `Display`, et
  `From<pbc::DecodeError>` pour que les étapes 1 à 3 de B.2 deviennent `PcOutOfRange`, `BadOpcode`, `Truncated` ;
- `pbc::LoadError` : sept variantes détaillées et `LoadError::FAULT_NAME = "BadHeader"` ;
- un test qui vérifie que `Fault::from_name("BadHeader")` vaut `None`, et que chaque nom fait l'aller-retour.

Vérifié : les 65 cas du corpus officiel passent, dont les 15 fautes d'exécution qu'ils exercent, comparées par leur
chaîne.

## 4. Ce que nous avons décidé, et pourquoi

> *Brouillon proposé par l'assistant — à relire et reformuler par l'équipe avant de passer en « adoptée ».*

Option B. Le type d'un résultat de tour ne doit pas pouvoir exprimer un état impossible : avec A, chaque appelant de
`run_turn` devrait gérer un `BadHeader` qui n'arrive jamais. La spec elle-même sépare les deux natures (B.5, dernier
paragraphe). Ce qui est normatif, ce sont les chaînes : `as_str()` et `FAULT_NAME` les fixent à un seul endroit
chacune.

## 5. Ce qui nous ferait changer d'avis

> *Brouillon — à compléter par l'équipe.*

- Une version de la spec où une faute peut survenir aux deux étages (par exemple une vérification de code au
  chargement qui lève `BadJump`).
- Si `exec` et `arena` finissent par convertir les deux types vers un troisième identique, c'est que l'option A ou C
  aurait été plus simple.
