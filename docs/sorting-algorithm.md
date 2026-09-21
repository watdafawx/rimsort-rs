# Sorting algorithm (spec)

Port of RimSort's `Sorter` (`sort_controller.py`) + `topo_sort.py`. Verified identical to the Python output on a real
584-mod list (`just golden`) and on 12 seeded shuffles/subsets of it (339–585 mods, `just golden-multi`), so the
result does not depend on input order.

## Inputs
- Every valid mod in the index (not just active ones) contributes rules; only **active** package ids are sorted.
- Rules per mod = About.xml ∪ community rules ∪ user rules (`loadBefore` / `loadAfter`; `loadTop` / `loadBottom` flags).
- `use_moddependencies_as_loadTheseBefore` (default off): declared `modDependencies` become extra loadAfter edges,
  except for tier 1/3 mods, and never when an explicit rule says the opposite (avoids creating cycles).

## Graph
`deps[A] = {B}` means B loads before A. `loadAfter: B` on A ⇒ `deps[A] ∋ B`; `loadBefore: B` on A ⇒ `deps[B] ∋ A`.
Edges to package ids that aren't installed are dropped.

## Tiers (each sorted independently, then concatenated 0 → 1 → 2 → 3, duplicates dropped keeping first)
| Tier | Members |
|------|---------|
| 0 | fixed list (Harmony, prepatcher, base game + DLC) + their transitive dependencies |
| 1 | fixed frameworks list + mods with `loadTop` + their transitive dependencies |
| 3 | mods with `loadBottom` + their transitive *reverse* dependencies (mods that must load after them) |
| 2 | every other active mod |

## Within a tier
Kahn levels (level N = mods whose deps are all in earlier levels). Each level is sorted by lowercase name
(package id as tiebreak — Python's tie order is arbitrary set order). A cycle aborts the whole sort and reports the
cycles (Tarjan SCCs); the list is left untouched.

## Deliberate RimSort quirks we mirror
- `<li MayRequire="…">pkg</li>` inside `loadBefore` / `loadAfter` / `incompatibleWith` (and force variants) is
  **ignored** (RimSort's XML→dict conversion yields a dict, which is then filtered out as "not a string").
  RimWorld itself does honor MayRequire, so this is arguably a RimSort bug; kept for identical results.
  `li` with only `IgnoreIfNoMatchingField` is kept.
- Community rule keys are matched case-insensitively here; Python matches exact-lowercase keys only.

## Not ported
Alphabetical mode (deprecated upstream).
