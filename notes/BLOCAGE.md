# BLOCAGE - E17b (2026-09-26)

RESOLU : ECHEC CONNU par decision humaine, voir E36.

## Symptome
`ppu_vbl_nmi/rom_singles/06-suppression.nes` (protocole blargg6000) echech : la ROM ecrit "Failed" a $6004.
Suite E17 = 30/31 ; toutes les autres ROMs passent, aucune regression.

## Commande de reproduction
```
cargo run -q -p puce8-cli --release -- suite --jusqua E17 --rapport out/rapport_e17.md
```

## Resultat ecrit par la ROM a $6004 (extrait)
```
00 - N
01 - N
02 - N
03 - N
04 - -
05 V -
06 V -
07 V -
08 V N
09 V N
1CDF33F8
06-suppression
Failed
```

## Hypotheses testees (seul parametre autorise par la sous-tache : PPU_DOTS_BEFORE_CPU)
| valeur | suite E17 | tests unitaires workspace | resultat 06-suppression |
|---|---|---|---|
| 3 (valeur d'origine) | 30/31 | 186 OK | ECHEC, motif ci-dessus |
| 2 | 30/31 | 186 OK | ECHEC, motif byte-identique |
| 1 | 30/31 | 186 OK | ECHEC, motif byte-identique |

Le dephasage intra-cycle PPU/CPU n'a aucun effet mesurable : le motif d'echec est identique sur les
trois valeurs. L'ecart ne vient donc pas de la phase du tick (crates/puce8-core/src/nes.rs:12) ; il
provient vraisemblablement de la logique NMI/suppression (E17a) ou d'un comportement que le parametre
autorise ne couvre pas. La sous-tache E17b interdit tout autre changement ("Seul parametre autorise",
"Ne jamais ajouter de decalage specifique a un test").

## Consequence
- Criteres E17 non atteints : `ppu_vbl_nmi 06` echech (les autres criteres sont OK : 01-04, vbl_clear_time,
  instr_misc/03-dummy_reads, aucune regression).
- E17 ne peut pas etre cloture FAIT avec les moyens de la sous-tache.

## A decider par l'humain (prochaine session)
- Soit autoriser une correction hors perimetre E17b (logique NMI/suppression, a partir du motif $6004),
  soit marquer `ppu_vbl_nmi/06-suppression` comme test optionnel / ECHEC CONNU et cloturer E17.
