# Plan 1 — keputusan yang diambil saat eksekusi

Dikumpulkan dari ledger SDD (urut kronologis). Tiap baris: keputusan — alasan — risiko kalau salah.

- - Ruling: T1 Step 1 (git init/branch/commit) skipped — controller did it during setup — none.
- - Ruling: T1 writes gen_refs.py producing datasets only; ref JSON for each analysis added by the task that consumes it — plan Task 1 Step 4 says "isinya dijelaskan di Task yang memakainya" — low.
- - Ruling: .gitignore must keep `.superpowers/` — SDD workspace — none.
- Ruling: added git-ignored .cargo/config.toml (linker = VS 14.44 link.exe, [env] LIB = ScopeCppSDK vc15 SDK+VC lib) so cargo builds from Git Bash — machine-local, not committed — if wrong, release builds may need a full Windows SDK install. Committed .gitignore `.cargo/`.
- Ruling: P3b carried into Task 5/7 dispatches (callers must not pass df<=0; or fix dist fns to return NaN when touching dist.rs) — cheap; if wrong, a crash on degenerate input. P3a goes to final review — low display impact.
- Ruling: "SPSS" in source comments is OK — constraint targets branding in UI/output; comments documenting compatibility are not branding — cost if wrong: trivial comment edit.
- Ruling: "Warning:" prefix stays English — spec §11 UI language is English — none.
- Ruling: Task 5 implementer on opus (hardest math: QR, collinearity diagnostics, leverage) — user's model config says hardest tasks: opus.
- Ruling: also entering loop — (F2) zero-variance y / perfect fit puts inf/NaN in cells: plan Review Focus line 1 says constant/zero-variance must give a note, not NaN in cells → treat as Important; (F3) test tolerances loosened to 1e-6/1e-8 violate Global Constraint 1e-9 → spec violation, trivial fix.
- Ruling: accept implementer's SPSS-memory decimals (Explore: mean/CI/trim/median/IQR dec+2; SE/var/SD dec+3; min/max/range dec; skew/kurt Fixed(3); K-S mean dec+2, SD dec+3) over my tentative fallback — my ruling allowed "if unsure, record it"; recorded in KNOWN_GAPS — cost if wrong: one-function edit after user screenshot.
- Ruling: K-S lower-bound marker 'd' (SPSS prints .200c,d) accepted; Explore keeps '*' — matches SPSS layouts — trivial if wrong.
- Ruling: fix at the root too — describe() sets variance/sd/se_mean exactly 0 when min == max (all procedures benefit) + explicit max==min guards in ttest — cost if wrong: none (constant data has zero variance by definition).
- Ruling: include Minor 2 (keep Paired Samples Correlations table when differences constant) — SPSS fidelity, same lines — cheap.
- Ruling: include Minor 2 (possible infinite loop in Z-name fallback) — a hang is worse than a crash — cheap.
- Ruling: fix wave includes I1, I2, Minors 1-8 (R² clamp + const predicted, Levene ssw tolerance, zero-variance threshold too loose, Welch df Fixed(3), footnote comma join + <sup> in HTML, -0.0 normalisation in OrderedValue, fixed() large-magnitude rounding, paired row headers) — all small, user-visible correctness — cost if wrong: one extra fix wave.
- Ruling: I3a/b/c deferred to the first task of Plan 3 (model extension: title/header footnotes, chart subtitle, VarId validation→Warning) — GUI is the consumer; doing it now widens this branch — cost if wrong: touching every procedure later (known, bounded).
- Ruling: Minors 9 (consolidate constant helpers — partially via I1 helper), 10, 11 (CSV decimals cap, header sanitising) → Plan 2 — CSV import belongs to Plan 2 I/O.

## Temuan minor yang ditunda

- Task 2: review Approved. Parked Minor (for final review / Plan 2 csv work): csv flexible(true) pads/drops cells silently; text cells trimmed; csv::Error flattened to String; missing tests for text cols/CSV errors.
- Task 3: review Approved. Parked Minor: (P3a) t_quantile inaccurate at df≥~1e4+ (statrs), needs Newton refinement or guard tests; (P3b) dist fns .expect() panic on df<=0/NaN — should return NaN; (P3c) SE tests tautological.
- Parked Minor: thin escaping tests; same-label adjacent parent merge (table.rs row_label); overflow >1e305.
- Task 5: minor (deferred): SPSS minimum-tolerance entry test (already-entered predictor tolerance) not implemented — add to KNOWN_GAPS at least.
- Task 5: minor (deferred): TooFewCases counts requested not retained predictors.
- Task 5: minor (deferred): footnote markers on table titles (T4 model limitation).
- Task 5: minor (deferred): text renderer misaligns grouped column headers (T4).
- Task 6: minor (deferred): npar_ks recomputes mean/sd separately from lilliefors (drift risk).
- Task 6: minor (deferred): swilk absolute 1e-19 range threshold (faithful to AS R94).
- Task 7: minor (deferred): df_cell prints Welch df as Count when fract()==0.
- Task 8: minor (deferred): runs footnote 'a' on cell not row header (logged in KNOWN_GAPS); histogram title has embedded newline (check renderers in Plan 3); group value label "Total" collides with Total row.
- Task 9: minor (deferred): CLI accepts unknown flags silently (--htlm typo), duplicate flags last-wins, flag-looking values bound as values, dep listed as indep not rejected.
