# D-008 — Piles de taille fixe et trace réservée d'avance

| Champ    | Valeur                          |
|----------|---------------------------------|
| Id       | D-008                           |
| Date     | 2026-11-23                      |
| Auteurs  | murx                            |
| Statut   | proposée                        |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

Le README (§3) interdit toute allocation dans la boucle d'un tour. B.1 fixe les tailles : 64 valeurs de pile de
données, 16 adresses de retour. B.3 borne la trace : `TRACE` coûte 1, donc un tour en produit au plus `gas_budget`
(65535 au maximum), et « le tampon de trace est dimensionné une fois pour le budget de la partie, jamais pendant le
tour ». Le lint `indexing_slicing` (D-001) interdit `tab[i]` hors des tests.

## 2. Options envisagées

### Option A — `BoundedStack<T, const N: usize>` sur un tableau, trace en `Vec::with_capacity(budget)`

- Aucune allocation pour les piles ; la capacité est dans le type.
- La trace est allouée une fois, avant la première instruction ; `push` reste sous la capacité.

### Option B — `Vec::with_capacity(64)` pour les piles, avec vérification de longueur

- Plus court, mais une allocation par tour et par pile, et rien dans le type n'empêche de dépasser.

### Option C — `ArrayVec` / `heapless`

- Exactement le besoin, mais ces crates ne sont pas dans la liste autorisée du §3.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) a proposé l'option A et l'a écrite dans `crates/vm/src/stack.rs` : `push` renvoie
`Err(Full)` sans modifier la pile, `pop` renvoie `Option`. Tous les accès (piles, mémoire, table de constantes, code)
passent par `.get()` / `.get_mut()`, ce qui satisfait `indexing_slicing` sans exception.

Il a noté une limite : la trace est allouée **à chaque tour** (une fois, avant la boucle), pas une fois par partie
comme le suggère B.3. C'est conforme à la règle « rien dans la boucle », mais pas à la lettre de la phrase de B.3.

Vérifié : un test vérifie que la capacité de la trace couvre le budget ; les tests de débordement (65ᵉ valeur, 17ᵉ
appel) et le corpus (cas 008, 023) passent.

## 4. Ce que nous avons décidé, et pourquoi

> *Brouillon proposé par l'assistant — à relire et reformuler par l'équipe avant de passer en « adoptée ».*

Option A. Les bornes de B.1 deviennent des constantes du type, l'interdiction d'allocation est tenue par
construction, et on n'ajoute aucune dépendance.

## 5. Ce qui nous ferait changer d'avis

> *Brouillon — à compléter par l'équipe.*

- Si l'arène veut tenir la lettre de B.3 (un tampon par partie) : ajouter une variante de `run_turn` qui écrit dans
  un `&mut Vec<i64>` fourni et vidé par l'appelant.
- Si un profil montre que l'initialisation des 64 + 16 cases à chaque tour coûte quelque chose de mesurable.
