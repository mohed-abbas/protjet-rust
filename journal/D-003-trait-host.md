# D-003 — Comment le moteur interroge le monde : le trait `Host`

| Champ    | Valeur                          |
|----------|---------------------------------|
| Id       | D-003                           |
| Date     | 2026-11-23                      |
| Auteurs  | murx                            |
| Statut   | proposée                        |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

`SENSE k`, `ACT k` et `RAND` demandent une réponse à l'extérieur de la machine. Deux appelants très différents
doivent la fournir :

- l'arène locale (`arena`), qui connaît le monde de l'annexe E et répond immédiatement ;
- le serveur distant (`exec`), qui envoie une trame `SENSE`/`ACT`/`RAND` au client et attend `SENSED`/`ACTED`/`DREW`
  (K.6). Cette attente peut échouer : connexion fermée, trame invalide, délai dépassé.

K.5 rend la suite des rappels normative : deux machines conformes posent les mêmes questions, dans le même ordre.
Le moteur ne doit donc poser une question que si l'instruction s'exécute réellement.

## 2. Options envisagées

### Option A — Un trait `Host` avec un type d'erreur associé

```rust
pub trait Host {
    type Error;
    fn sense(&mut self, k: u8) -> Result<SenseAnswer, Self::Error>;
    fn act(&mut self, action: Action, arg: i64) -> Result<bool, Self::Error>;
    fn rand(&mut self) -> Result<i64, Self::Error>;
}
pub fn run_turn<H: Host>(…, host: &mut H) -> Result<TurnResult, H::Error>;
```

- Le moteur reste synchrone, sans `async` (interdit par le README) : le serveur bloque dans `sense()` sur sa socket.
- L'erreur de l'hôte (réseau) est séparée des fautes du programme : `Err` = le tour n'a pas de résultat,
  `Ok(TurnResult { fault: Some(..) })` = le programme a fauté.
- Monomorphisé : aucun coût d'appel dynamique dans la boucle.

### Option B — Une machine à états « reprenable »

`step()` renvoie `NeedsSense(k)` / `NeedsAct(..)` / `NeedsRand` / `Done`, et l'appelant rappelle `resume(answer)`.

- Aucune dépendance du moteur vers l'appelant ; très testable.
- Chaque appelant doit écrire la boucle de reprise ; l'état du tour (piles, pc, gaz) doit vivre hors de la pile
  d'appels, ce qui complique `run_turn`.

### Option C — Des fermetures passées à `run_turn`

- Trois paramètres `FnMut` ; plus léger qu'un trait mais moins lisible, et le partage d'état entre les trois
  fermetures (la même socket) devient pénible.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) a proposé l'option A et l'a écrite dans `crates/vm/src/host.rs` :

- `Action { Move, Take, Drop, Eat }` avec `from_code(k)` (`None` hors de `0..=3`, ce qui donne `BadAction`),
  `as_str()` (`"MOVE"`…) et `from_name()` ;
- `ActRecord { action, arg, ok }`, conservé dans `TurnResult::act` même si une faute suit (E.2) ;
- `SenseAnswer::{Value(i64), Fault(Fault)}`, parce que K.6 permet à `SENSED` de porter une faute (le cas du corpus
  `SENSE 200` → `BadSensor`) : c'est l'hôte, pas le moteur, qui sait quels capteurs existent ;
- la garantie, documentée sur le trait et testée, qu'aucun rappel n'est fait pour une instruction refusée (gaz, pile,
  `BadAction`, `AlreadyActed`).

Vérifié : le corpus compare la liste exacte des rappels en premier ; les 65 cas passent. Un test unitaire vérifie
qu'une erreur de l'hôte remonte telle quelle en `Err`.

## 4. Ce que nous avons décidé, et pourquoi

> *Brouillon proposé par l'assistant — à relire et reformuler par l'équipe avant de passer en « adoptée ».*

Option A. Elle garde `run_turn` comme une simple boucle, reste synchrone, et permet à `arena` d'utiliser
`std::convert::Infallible` comme erreur. Le type associé évite d'imposer à `vm` un type d'erreur réseau qu'il ne
connaît pas.

## 5. Ce qui nous ferait changer d'avis

> *Brouillon — à compléter par l'équipe.*

- Si `exec` doit traiter plusieurs sessions sur un seul fil (sans `async`), un moteur reprenable (option B) deviendrait
  nécessaire : on ne peut pas bloquer dans `sense()`.
- Si le vérificateur statique (bonus) a besoin d'exécuter le code pas à pas.
