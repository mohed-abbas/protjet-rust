# D-001 — Découpage du dépôt en workspace Cargo

| Champ    | Valeur                       |
|----------|------------------------------|
| Id       | D-001                        |
| Date     | 2026-11-22                   |
| Auteurs  | *à compléter*                |
| Statut   | proposée                     |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

Le README (§5, critère 1) évalue la « séparation nette entre le format PBC (`pbc`), le moteur d'exécution et la
couche réseau (`exec`) ». Le MVP exige en plus un match entre deux agents, donc un monde (annexe E), et la commande
`palestre exec --listen <adresse>` doit exister. Le moteur doit servir à deux appelants différents : l'arène locale,
qui répond elle-même à `SENSE`/`ACT`/`RAND`, et le serveur réseau, qui les renvoie au client (annexe K.1).

## 2. Options envisagées

### Option A — Un seul crate, découpé en modules

Un binaire `palestre` avec des modules `pbc`, `vm`, `exec`, `arena`.

- Plus simple : un seul `Cargo.toml`, aucun `pub` à exposer entre modules.
- Rien n'empêche `vm` d'importer du code réseau ou du monde par erreur : la séparation ne tient qu'à la discipline.

### Option B — Workspace de cinq crates

`crates/pbc` → `crates/vm` → `crates/exec` et `crates/arena`, plus `crates/palestre` (le binaire).

- Le sens des dépendances est vérifié par le compilateur : `vm` ne peut pas utiliser `exec`, puisqu'il ne le connaît pas.
- L'API publique de chaque crate est explicite (`pub`), ce que le critère « API publique claire et restreinte » regarde.
- Plus de fichiers `Cargo.toml` ; un changement d'API se propage sur plusieurs crates.

### Option C — Deux crates : une bibliothèque et un binaire

Toute la logique dans `palestre-lib`, la CLI dans `palestre`.

- Compromis entre A et B, mais la séparation `pbc` / `vm` / `exec` reste interne à la bibliothèque.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) a proposé l'option B et a créé le squelette :

- un workspace (`resolver = "3"`, édition 2024) avec `pbc`, `vm`, `exec`, `arena` et `palestre` ;
- `pbc::SPEC_VERSION` comme unique emplacement du numéro de spécification (annexe H.2) ;
- `#![forbid(unsafe_code)]` en tête de chaque crate ;
- des lints Clippy refusés dans tout le workspace : `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`,
  `unreachable` et `indexing_slicing`, avec une exemption pour les tests dans `clippy.toml` ;
- une CI GitHub Actions qui lance `cargo fmt --check`, `cargo clippy -- -D warnings` et `cargo test`.

Premier essai corrigé : l'assistant avait déclaré d'emblée toutes les dépendances internes (`vm` → `pbc`, etc.).
Cargo 1.100 avertit sur toute dépendance déclarée mais inutilisée ; elles ont donc été retirées, et chaque lien
du graphe prévu (`pbc` → `vm` → `exec` / `arena`) s'ajoute dans le `Cargo.toml` au premier usage dans le code.

Points que l'assistant a signalés comme à vérifier ou à décider par l'équipe :

- `indexing_slicing` est strict : il refuse aussi `mem[k]` quand `k` est un `u8` et `mem` un `[i64; 256]`, alors que
  ce cas ne peut pas paniquer. Il force `.get()` partout. À garder ou à assouplir après les premières lignes du moteur.
- L'endroit où vit `Fault` n'est pas tranché : `BadHeader` est une erreur de chargement (`pbc`), les quinze autres
  sont des fautes d'exécution (`vm`), mais la spec en fait une seule liste close.
- La façon dont `vm` demande `SENSE`/`ACT`/`RAND` à son appelant (un trait implémenté par `arena` et `exec` ?)
  n'est pas tranchée.
- Le crate `toml` n'est pas dans la liste autorisée du §3 : lire les `case.toml` du corpus demande soit une
  validation préalable, soit un petit lecteur écrit à la main.

## 4. Ce que nous avons décidé, et pourquoi

*À compléter par l'équipe.*

## 5. Ce qui nous ferait changer d'avis

*À compléter par l'équipe.* Pistes : un changement de la v2.0.0 qui oblige à modifier tous les crates à la fois ;
des types qu'on duplique entre crates pour éviter une dépendance.
