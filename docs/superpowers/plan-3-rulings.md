# Plan 3 — keputusan yang diambil saat eksekusi

- Ruling: worker-thread execution introduced in T2 (run_procedure helper) rather than T6 — every later task depends on it — cost if wrong: refactor.
- Ruling: Plan 2 carry-overs (17/18 note narrowing, unique tmp name) done in T1 — small sav-io edits — none.
- Task 1: review Needs fixes: (F1) K-S lower-bound d moved to row header (per-value regression). Ruling: include Minor 2 (ANOVA b on Sig cell, SPSS placement) and K-S c back on Sig cell (.200c,d as SPSS) — cheap fidelity. Minor 3 (text renderer marker separator) deferred.
- Task 2: dispatched implementer opus (base b0c6fae). Ruling: jobs clone Dataset on start and swap back — safety over memory — cost if wrong: 2× RAM for huge datasets during a job.
- Ruling: accept egui 0.35 / egui_plot 0.35-0.36 for now — newer needs rustc 1.95 — cost if wrong: a version bump later.
- Ruling: all cargo runs use CARGO_HOME=D:\tmp\cargo-home TMP/TEMP=D:\tmp\tmp; local .cargo/config.toml [build] jobs = 4 to limit RAM — machine-local — none.
- Ruling: release profile panic = "unwind" (workspace) — brief requires isolation; +~0.5 MB — cost if wrong: slightly bigger exe.
- Ruling: include Minors 5 (job closure returns changed), 7 (message queue), 9 (non-circular menu test) — cheap, later tasks rely on them.
- Ruling: keep file-filter text "SPSS Statistics (*.sav)" — nominative description of the file format users know, not branding of the app — cost if wrong: one string.
- Task 3: dispatched implementer opus (base 6d11e6c). Ruling: Align/Role kept UI-side (not in model/.sav) — avoids widening sav-io scope — cost if wrong: Align/Role lost on save (KNOWN_GAPS).
- Ruling: procedure jobs that change data clear undo history (like SPSS, procedures aren't undoable) — Task 6 transforms push snapshots instead — cost if wrong: user can't undo saved residuals (they can delete the variables).
- Ruling: include Minors 3 (Excel quoted cells + KNOWN_GAPS for "1.234"), 4 (commit pending edit before menu Undo/Redo/Insert/Clear — data lands on wrong cell), 5 (single value fills selection like SPSS), 6+7 KNOWN_GAPS/cleanup.
- Ruling: include all cheap Minors — screenshot timeout, no-animation scroll before capture, chart num_f64 serde, open_output size cap, viewer errors visible in Output window, RTF control chars, single Log outline node. Defer: chart_number leading-zero rule location, E-notation residual mean.
- Task 5: review Needs fixes: (I1) Paired dialog double-click no-op. Ruling: include Minors 2 (CI message/check mismatch), 3 (EXAMINE /COMPARE only with plots), 4 (stale names styled), 5 (reject self-pairs and equal groups), 7 (virtualise source list). Defer 6 (small duplication).
- Ruling: I1 fixed in Task 6 (bounded sniff in sav-io: first ~1 MB / 1000 lines; full validation in the import job).
- Ruling: I2 handed to Task 7 — one "Save changes?" (Yes/No/Cancel) flow for New, Open, Import CSV/Excel and Exit — same dialog/code path — cost if wrong: none (Task 7 dispatch must include it).
- Ruling: include Minors 3 (no "ran" log on rev-conflict drop), 4 (open If sub-dialog when the condition has the error), 6 (sheet names off UI thread or verify cheap). Defer 5 (undo memory two columns), 7.
- Ruling: carry into Task 7 — pin Auto-detected delimiter/decimal from the preview into import_csv so preview, log and import agree — touches same import entry points — trivial.
- Ruling: fix I2 now with per-instance subfolders + lock file (students do open two windows) — cost if wrong: small complexity.
- Ruling: include Minors 1 (remove dead SaveKind::Autosave), 2 (string spacing), 3 (recover fallback to next-newest / keep files on failure), 4 (drop clock-based marker: any leftover autosave = crash signal), 6 (prune stray .tmp); 5 → KNOWN_GAPS line.
- Ruling: one fix wave = I1-I4 (I3: implement "Save contents of output viewer?" Yes/No/Cancel at Exit) + M1-M6, M8 + full-tutorial kittest. M7 (README) handled by controller in the final README pass.
- Ruling: manual checks (Word paste, release Data View perf) done by controller after the fix wave.
- User (2026-11-19): skip manual testing; after fix wave write final README with screenshots, then finish. Ruling: manual checks (Word paste, release perf) dropped per user; noted in README/KNOWN_GAPS as unverified.

## Temuan minor yang ditunda

- Task 1: minor (deferred): text renderer appends title/header markers without separator ("Coefficientsa").
- Task 2: minor (deferred): Saved arm sets path even if doc was replaced (non-UI callers only); AppState.doc pub allows rev reset; autosave shows busy spinner & modal on failure (Task 7).
- Task 3: minor (deferred): paste_fill builds whole fill string (big selections); block_values awkward; Cut/Paste key events bypass prepare's commit.
- Task 4: minor (deferred): chart_number leading-zero rule lives in app not output_model; residual mean "0.00" vs SPSS E-notation; native viewport + Word paste unchecked (manual run at the end).
- Task 5: minor (deferred): Dialogs get/get_mut + ok/paste preamble duplication; strikethrough stale names not visually tested.
- Task 6: minor (deferred): last_record_end ignores lone CR and uses all separators for field-start; first record >1 MB preview shows parse error; undo keeps two full columns per Compute.
- Task 7: minor (deferred): recovered_from set on rev-mismatch Recovered Ok; Instance::create folder-before-lock startup race (fails safe).
