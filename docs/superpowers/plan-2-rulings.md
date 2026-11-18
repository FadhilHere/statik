# Plan 2 — keputusan yang diambil saat eksekusi

- Ruling: VarId stays positional; engine validates and warns (plan T2) — simplest for grid; GUI rebuilds dialog state after edits — cost if wrong: later refactor to stable ids.
- Ruling: include Minors 2 (subtype 11 per-var count), 3 (code page table), 4 (subtype 14 decoding) in the round — real misreads of real files, small — cheap.
- Task 3: review dispatched. Task 4: dispatched implementer opus (base b97570e), new crate transform, told not to touch sav-io. Ruling: ** left-assoc (2**3**2=64) and TO ranges in stat functions added — SPSS behaviour, common in questionnaires — cost if wrong: small parser change.
- Ruling: fix round includes F1 + Minor 3 (error prefix "invalid .sav file" on CSV/XLSX errors — user-visible) + Minor 4 (KNOWN_GAPS wording) + Minor 2 documented in KNOWN_GAPS.
- Ruling: leading-zero IDs stay numeric (SPSS import does the same); but integers with >15 significant digits (NIK) become text to avoid silent precision loss — cost if wrong: one rule change.
- Ruling: fix now in transform (lower limits to 64 nesting / 256 tree depth + worst-case 1 MB-stack test) AND Plan 3 GUI runs transforms/procedures on a worker thread with 16 MB stack — belt and braces — cost if wrong: an expression > 64 levels rejected (no student writes that).
- Ruling: implement SPSS zero exceptions (0*missing=0, 0/missing=0, MOD(0,missing)=0) — SPSS parity matters for dummy×x interaction terms; overrides plan brief "any missing → sysmis" — cost if wrong: trivial revert.
- Task 5: dispatched implementer sonnet (base 4f7b729). Ruling: recode rules checked on raw value; Value/Range match user-missing raw values; Missing matches NaN+user-missing; Else catches rest — SPSS semantics — cost if wrong: rule tweak.
- Ruling: one fix wave = Important 1-6 + Minors 7 (reserved import names), 8 (64-byte names), 9 (write error kind), 10 (set_type returns sysmis count), 11 (string width single source) + T2 set_type requested width + T5 NaN/lo>hi bounds. Defer 12 (NA tokens), 13, 14.
- Ruling: errors in English (spec §11 UI English) — overrides earlier Indonesian messages — cost if wrong: string edits.
- Ruling: weight variable — read and keep it in the model as dataset-level metadata + note "analyses are unweighted"; dates keep their format code in Variable so they round-trip (displayed as numbers until GUI supports dates) — avoids permanent loss on re-save — cost if wrong: small model field.
- Ruling: (breaker, no second wave) carry into Plan 3 Task 1 dispatch — narrow the 17/18 note to non-"$@" user attributes and use a unique tmp file name in replace_file — real, small, load-bearing for GUI warnings — cost if wrong: noisy warning until fixed.

## Temuan minor yang ditunda

- Task 1: minor (deferred): hand writer subtype 3 compression field 0 (harmless).
- Task 1: minor (deferred): encoding_rs duplicated in dev-dependencies.
- Task 2: minor (deferred): set_type Numeric→String "-0" text for tiny negatives; set_type ignores requested String width; regression id list built twice; Dataset::numeric/var still panic on stale ids by design (future run_* must check).
- Task 4: minor (deferred): non-0/1 values in logic treated as true (PSPP: missing) — document in KNOWN_GAPS; LAG of user-missing → sysmis unverified; reserved-word names unreferenceable (matches SPSS).
- Task 5: minor (deferred): empty pairs → Ok(vec![]); NaN bounds accepted silently; error test only asserts n_vars.
- Task (final): minor (deferred): set_string_width leaves longer string_value_labels/missing keys (writer cuts them).
