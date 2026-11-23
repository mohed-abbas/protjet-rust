# D-007 — Décodage dans `pbc`, table de gaz dans `vm`

| Champ    | Valeur                          |
|----------|---------------------------------|
| Id       | D-007                           |
| Date     | 2026-11-23                      |
| Auteurs  | murx                            |
| Statut   | proposée                        |
| Commits  | *à compléter au premier commit* |

## 1. Contexte

B.2 découpe chaque pas en six étapes ; les trois premières (pc dans le code, opcode connu, opérande complet) ne
regardent que les octets, la quatrième prélève le gaz. B.3 compte **43** instructions (et non 42, comme l'écrivaient
nos premiers documents d'équipe). A.3 impose qu'un décodage à n'importe quel décalage fonctionne, y compris au milieu
d'un opérande, sans analyse préalable du code.

Il faut décider où vivent la forme des instructions (octet, opérande) et leur prix.

## 2. Options envisagées

### Option A — `enum Instruction` et `decode_at` dans `pbc`, `gas::cost` dans `vm`

- `pbc` reste « le format » : octets ⇄ instructions. Le vérificateur statique (bonus) et un futur désassembleur
  peuvent l'utiliser sans dépendre du moteur.
- `vm::gas::cost(&Instruction)` est un `match` **exhaustif** : ajouter une instruction sans lui donner de prix ne
  compile pas.

### Option B — Décoder directement dans la boucle du moteur (un `match` sur l'octet)

- Un niveau d'indirection de moins.
- Le décodage n'est plus testable seul, et le vérificateur devrait le réécrire.

### Option C — Table de 256 entrées `(taille, gaz)` indexée par l'octet

- Rapide et compacte, mais les cases vides doivent être distinguées, et l'exhaustivité n'est plus vérifiée par le
  compilateur.

## 3. Ce que l'assistant IA a proposé

L'assistant (Claude) a proposé l'option A et l'a écrite :

- `pbc::Instruction` (43 variantes, opérandes typés `u16`/`i16`/`u8`), `opcode()`, `size()`, `mnemonic()`,
  `encode_into()` ;
- `pbc::decode_at(code, pc) -> Result<Instruction, DecodeError>`, qui vérifie dans l'ordre de B.2 et lit les
  opérandes en gros-boutiste ;
- `vm::gas::cost`, et un test qui recopie la colonne « Gaz » de B.3 opcode par opcode.

Erreur de l'assistant à consigner : dans le guide HTML et les fichiers de tâches qu'il avait rédigés plus tôt, il
annonçait « 42 opcodes ». En écrivant les tests, il a compté 43 lignes dans B.3 et corrigé ; le test
`exactly_43_bytes_are_opcodes` décode les 256 octets et en attend exactement 43.

Vérifié : tests d'aller-retour `decode(encode(i)) == i` pour les 43 instructions et pour chaque octet ; décodage au
milieu d'un opérande (A.3) ; les 65 cas du corpus.

## 4. Ce que nous avons décidé, et pourquoi

> *Brouillon proposé par l'assistant — à relire et reformuler par l'équipe avant de passer en « adoptée ».*

Option A. Le décodage est la même chose pour le moteur et pour le vérificateur, il doit donc exister une seule fois,
dans le crate du format. Le prix, lui, est de la sémantique d'exécution. Le `match` exhaustif transforme un oubli de
la table en erreur de compilation.

## 5. Ce qui nous ferait changer d'avis

> *Brouillon — à compléter par l'équipe.*

- Si un profil `criterion` montre que le décodage à chaque pas domine le temps d'un tour : pré-décoder le programme
  au chargement (en gardant le décodage à n'importe quel décalage, A.3).
- Une v2 de la spec où le prix dépend des opérandes.
