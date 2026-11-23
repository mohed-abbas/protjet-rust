# D-004 — API publique : `Program` validé et `run_turn`

| Champ    | Valeur                          |
|----------|---------------------------------|
| Id       | D-004                           |
| Date     | 2026-11-23                      |
| Auteurs  | murx                            |
| Statut   | proposée                        |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

Le README (§5) évalue une « API publique claire et restreinte ». Trois personnes écrivent du code qui se rencontre à
deux frontières : Ilia (`pbc::load`, `exec`) produit des `Program` et appelle le moteur ; Wilson (`arena`, corpus)
l'appelle aussi. Ces frontières doivent être figées tôt, pour que chacun avance sans attendre l'autre.

Contraintes de la spec : `const_count` dans `0..=4095`, `code_len` dans `1..=65535` (A.1) ; la mémoire de 256 cellules
persiste d'un tour à l'autre, les piles et `pc` repartent de zéro (B.1) ; sur une faute, les écritures mémoire déjà
faites restent, la restauration est l'affaire de l'arène (E.2, FORMAT.md du corpus).

## 2. Options envisagées

### Option A — `Program` à champs privés, construit par `from_parts` ; `run_turn` fonction libre

```rust
impl Program {
    pub fn from_parts(constants: Vec<i64>, code: Vec<u8>) -> Result<Program, LoadError>;
    pub fn constants(&self) -> &[i64];
    pub fn code(&self) -> &[u8];
}
pub fn run_turn<H: Host>(program: &Program, mem: &mut Memory, budget: u16, host: &mut H)
    -> Result<TurnResult, H::Error>;
```

- Tout `Program` existant respecte A.1 : le moteur n'a pas à revérifier `code_len >= 1`.
- La mémoire appartient à l'appelant (`&mut [i64; 256]`) : l'arène peut la copier avant le tour et la restaurer.
- Aucun état caché entre deux tours dans `vm`.

### Option B — Un objet `Vm` qui possède le programme et la mémoire

`Vm::new(program)`, puis `vm.run_turn(budget, host)`.

- Plus « objet », et les tampons (trace) pourraient être réutilisés d'un tour à l'autre.
- La restauration de la mémoire après une faute demande un accesseur supplémentaire ; deux agents = deux `Vm`.

### Option C — `Program` à champs publics

- Le plus simple, mais un `Program { code: vec![] }` devient constructible et le moteur doit se défendre partout.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) a proposé l'option A et l'a écrite :

- `pbc::Program` (champs privés) et `from_parts`, que le futur `pbc::load` d'Ilia appellera après avoir lu l'en-tête ;
- `vm::run_turn`, `vm::TurnResult { gas_used, fault, act, trace }` et les constantes `MEMORY_CELLS`,
  `DATA_STACK_SLOTS`, `CALL_STACK_DEPTH` ;
- l'aide de test `pbc::encode_all(&[Instruction])`, pour écrire des programmes à la main sans fichier `.pbc`.

Il a aussi relevé que la répartition initiale des tâches confiait le décodage (`decode_at`) à Ilia alors que le moteur
en a besoin dès le premier jour ; il a proposé de le déplacer dans ce travail sur les interfaces (voir D-007), Ilia
gardant `load`.

Vérifié : un faux chargeur écrit hors du dépôt (à la place de `load`) + `from_parts` + `run_turn` passent les 65 cas.

## 4. Ce que nous avons décidé, et pourquoi

> *Brouillon proposé par l'assistant — à relire et reformuler par l'équipe avant de passer en « adoptée ».*

Option A. Un `Program` invalide ne peut pas exister, ce qui supprime une classe d'erreurs dans le moteur. Laisser la
mémoire à l'appelant colle à E.2 : c'est l'arène qui décide de restaurer, le moteur ne fait que l'exécution.

## 5. Ce qui nous ferait changer d'avis

> *Brouillon — à compléter par l'équipe.*

- Si les mesures (`criterion`) montrent que l'allocation de la trace à chaque tour pèse : passer à une variante qui
  réutilise un tampon fourni par l'appelant.
- Si `arena` ou `exec` finissent par dupliquer de la logique de validation que `Program` aurait dû porter.
