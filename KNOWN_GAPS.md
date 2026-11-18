# Known gaps

Deviations from SPSS and unimplemented features are recorded here by the tasks that find them.

## Regression (Task 5)

- **Decimals — verify with the user's SPSS screenshots.** Model Summary: R, R Square, Adjusted R Square `Fixed(3)`; Std. Error of the Estimate `Fixed(5)`; Durbin-Watson `Fixed(3)`. ANOVA: Sum of Squares, Mean Square, F `Fixed(3)`; df `Count`; Sig. `Sig`. Coefficients: B, Std. Error, Beta, t `Fixed(3)`; Sig. `Sig`; Tolerance, VIF `Fixed(3)`. Excluded Variables: Beta In, t, Partial Correlation, Tolerance, VIF, Minimum Tolerance `Fixed(3)`; Sig. `Sig`. Collinearity Diagnostics: Eigenvalue, Condition Index `Fixed(3)`; Variance Proportions `Fixed(2)`. Residuals Statistics: Minimum, Maximum, Mean, Std. Deviation `Fixed(4)`; N `Count`. Saved variables: width 11, 5 decimals.
- **Normal P-P plot:** observed cumulative probability = `i/(n+1)` for the i-th smallest ZRESID; expected = `Φ(ZRESID)`. The exact plotting position SPSS REGRESSION uses was not confirmed against IBM SPSS Algorithms; verify visually.
- **Variables Entered / Predictors footnotes** list predictors last-entered first (as SPSS usually prints them); verify ordering.
- **Residuals Statistics** shows only Predicted Value, Residual, Std. Predicted Value, Std. Residual. SPSS adds rows (Std. Error of Predicted Value, Adjusted Predicted Value, Stud. Residual, Deleted Residual, Stud. Deleted Residual, Mahal./Cook's distance, Centered Leverage) when plots/saves involve SRESID etc.
- **Excluded Variables:** for an exact linear combination (tolerance < 1e-10) Beta In, t, Sig., Partial Correlation and VIF are shown as "."; the entered-variables footnote becomes "Tolerance = .000 limit reached." Wording and which cells SPSS blanks are unverified.
- Table-title footnote markers (e.g. "Model Summary^b", "ANOVA^a") are not attached to titles; the footnote text is still listed.
- No Descriptive Statistics / Correlations tables, no confidence intervals, no casewise diagnostics, no stepwise methods, no weights, no "no constant" option.
- SPSS's second entry criterion is not implemented. SPSS also refuses a candidate predictor when entering it would push an already-entered predictor's tolerance below 0.0001. Only the candidate's own tolerance against the predictors already entered is checked here.
- The `TooFewCases` check counts requested predictors, not those retained after the tolerance exclusion.
- Zero-variance and perfect-fit handling:
  - A constant dependent variable produces a single warning and no tables. The wording is SPSS-like but not verified.
  - A perfect fit shows "." for t, Sig., F, Durbin-Watson and Std. Residual, adds a warning, and skips the residual charts.
  - A predictor counts as constant when max == min or its centred SS ≤ 1e-12 × raw SS. Such a predictor is excluded with tolerance 0.

## Normality: Explore, 1-Sample K-S, Shapiro-Wilk (Task 6)

- **Monte Carlo Sig. in 1-Sample K-S.** SPSS terbaru mungkin juga menampilkan baris Monte Carlo Sig. di 1-Sample K-S; angka Monte Carlo tidak bisa identik karena generator acak SPSS tidak publik — konfirmasi dari screenshot pengguna. Statik shows no Monte Carlo row.
- **Lilliefors Sig. between .100 and .200.** The Dallal-Wilkinson formula is used for every p and capped at .200 (lower bound). The formula is only claimed accurate for p < .10. statsmodels (`pvalmethod="approx"`) switches to a table interpolation above .10, and R `nortest` switches to the Stephens formula. The method SPSS uses in this range was not confirmed, so for p in .10–.20 the third decimal may differ from SPSS.
- **Explore Descriptives decimals — unverified, check against a user's screenshot.** Mean, 95% CI bounds, 5% Trimmed Mean, Median and Interquartile Range use variable decimals + 2. Std. Error of Mean, Variance and Std. Deviation use + 3. Minimum, Maximum and Range use the variable's own decimals. Skewness, Kurtosis and their standard errors use `Fixed(3)`. All are capped at 16 decimals. This comes from recalled SPSS output (e.g. integer data: Mean 74.70, Variance 91.343, Minimum 50). The controller's fallback rule was "+2 for everything except skew/kurt". To change it, edit `descriptives()` in `procedures/src/explore.rs`.
- **1-Sample K-S decimals:** Mean uses variable decimals + 2 and Std. Deviation uses + 3. For a saved residual with 5 decimals this gives `.0000000` and 8 decimals, which matches recalled SPSS output. Most Extreme Differences and Test Statistic use `Fixed(3)`.
- **Footnote markers:** In Tests of Normality the lower-bound note is `*` and "Lilliefors Significance Correction" is `a`; the `a` is not attached to the "Kolmogorov-Smirnov" column header, because header markers are not supported. In the 1-Sample K-S table the Asymp. Sig. cell carries `c` and, for a lower bound, `d` (SPSS shows `.200c,d`); markers `a,b` ("Normal Parameters") and `c` are not attached to the row headers. The `c`/`d` placement in SPSS 27+ is not confirmed.
- **Missing values:** Explore excludes cases listwise across all dependents (the SPSS default); "exclude pairwise" and "report values" are not implemented. 1-Sample K-S excludes test by test.
- **Explore scope:** Only Descriptives, Tests of Normality and Normal Q-Q plots. There are no factor lists, M-estimators, outliers/extreme values, percentiles, stem-and-leaf, boxplot, detrended Q-Q, histogram or spread-vs-level output.
- **Q-Q plot:** Expected values use Blom's (r − 3/8)/(n + 1/4), with tied values taking their mean rank. SPSS's handling of ties was not verified.
- **Edge cases:**
  - A constant dependent gets a warning and is omitted from Tests of Normality and the Q-Q plot.
  - When n < 5 the K-S cells show "." and a warning is added. Shapiro-Wilk also shows "." outside 3 ≤ n ≤ 5000.
  - 1-Sample K-S omits a constant variable, or one with fewer than 5 cases, and adds a warning.
  - The warning wording is SPSS-like but not verified.
- Shapiro-Wilk for 4 ≤ n ≤ 11 returns p = 1e-99 when log(1 − W) ≥ γ, as AS R94 does. scipy returns NaN in that case.

## t tests (One-Sample, Independent, Paired)

- **Decimals — unverified, check against a user's screenshot.** Mean, Mean Difference and CI bounds use variable decimals + 2 (paired differences use the larger of the two variables' decimals). Std. Deviation, Std. Error Mean and Std. Error Difference use + 3. t, F and Correlation use `Fixed(3)`; df is a count, or `Fixed(3)` for Welch df. Sig. / p use the Sig format.
- **One-sided p** is the two-sided p divided by 2 (for the correlation as well), shown regardless of the sign of t.
- **Missing values:** One-Sample and Independent exclude analysis by analysis (the SPSS default); Paired excludes per pair. "Exclude cases listwise" is not implemented.
- **Warnings:** a variable/pair with sd 0 (or a constant difference) keeps its statistics table but gets the warning "t cannot be computed because the standard deviation is 0." instead of a test row. The wording of the other warnings (fewer than 2 cases, empty group) is SPSS-like but not verified.
- **Paired correlation** shows "." when either variable is constant; it is still printed when only the differences are constant.
- **Group labels** in Group Statistics use the value label, else the group value with the grouping variable's decimals. SPSS's "Group 1 / Group 2" cut-point option is not implemented.
- **Header footnotes:** none are attached.

## One-Way ANOVA, Runs Test, Descriptives, Frequencies

- **ONEWAY homogeneity:** only the "Based on Mean" row is implemented. The Median, "Median and with adjusted df" and "Trimmed mean" rows (formulas in *IBM SPSS Statistics Algorithms*, ONEWAY) are not implemented until the document is verified. Post hoc tests, contrasts, Welch/Brown-Forsythe and the plots are not implemented.
- **ONEWAY decimals — unverified:** Mean and CI use variable decimals + 2, Std. Deviation / Std. Error + 3, Minimum / Maximum the variable decimals, Sum of Squares / Mean Square / F / Levene `Fixed(3)`. Groups with n = 1 show "." for SD, SE and CI. When every group is constant, F and Sig. show "." and a warning is added. Group labels use the value label, else the value with the factor's decimals.
- **Runs test:** the Test Value uses variable decimals + 2 (unverified). The `a` ("Median") marker is attached to the Test Value cell, not the row header. A constant variable (all cases on one side) is omitted with a SPSS-like warning (wording unverified). The continuity correction for n < 50 follows the brief; the exact n threshold in SPSS is unverified. Only the median cut point is supported.
- **Descriptives:** Min/Max use the variable decimals, Mean + 2, Std. Deviation + 3, skewness/kurtosis `Fixed(3)` (unverified). Each variable uses its own valid cases; "Valid N (listwise)" uses complete cases. Z scores are computed per variable from its own valid cases. `Z<name>` is created; when the name is taken or invalid `ZSC001`, `ZSC002`, ... are used. A constant variable gets an all-system-missing Z column and a warning. The Z variable's width (11) and label ("Zscore(<name>)" / "Zscore:  <label>") are unverified.
- **Frequencies:** the table title is the variable label (else name); the SPSS "Statistics" table only has N Valid / Missing. Missing group lists user-missing values (labelled like valid values), "System" for NaN and a Total row, followed by the grand Total (Percent only). The histogram has no normal curve (option not offered). Percentiles, mean etc. are not implemented.

## .sav read/write (plan 2, Task 1)

- **Not read:** value labels and missing values of *string* variables (the data model only has numeric value labels/missing values), long-string value labels and missing values (record 7 subtypes 21/22), the weight variable, file label, documents (record 6), multiple-response sets, extended case count (subtype 16), and the Columns/Align/Role settings (no fields in `Variable`).
- **Formats:** only width and decimals of the print format are kept. DATE, DOLLAR, COMMA and other formats are read as plain numbers and written back as `F`. Numeric width is clamped to 1–40 and decimals to ≤16 (and < width) on write.
- **`.zsav`** (zlib compression) is reported as unsupported. Floating-point formats other than IEEE (VAX/IBM) are reported as unsupported.
- **Writing:** always UTF-8, little-endian, bytecode-compressed. Variable labels are cut to 255 bytes and value labels to 120 bytes (on a character boundary). More than three discrete missing values is an `Unsupported` error. A string variable's width is raised to the longest UTF-8 value if needed (SPSS widths are bytes, CSV widths are characters). Columns are written as the variable width (max 255), Align as right for numbers / left for strings, and no Role.
- **Very long strings (> 255 bytes):** the value is split into consecutive 255-byte pieces, as ReadStat writes and reads it; the PSPP manual's wording suggests 252 data bytes per segment. Verified only against ReadStat (pyreadstat), not against a file saved by SPSS itself.
- **Short names** are generated as the upper-cased first 8 bytes of the name, with `_1`, `_2`, ... on collisions or for reserved words (ALL, AND, BY, EQ, GE, GT, LE, LT, NE, NOT, OR, TO, WITH). The full name goes in record 7 subtype 13. A long name that is itself a reserved word is not changed.
- **Reading names:** a name that is not valid in Statik gets its invalid characters replaced by `_` (and a leading `v` if needed); a duplicate gets a `_1`, `_2`, ... suffix. Trailing spaces in string values are dropped.
- Compatibility of written files is tested with pyreadstat (ReadStat), not with SPSS itself.

## CSV / XLSX import (plan 2, Task 3)

- **Detection:** the delimiter is the one of `,` `;` tab that splits the first 20 lines into the same number (> 1) of fields, most fields winning. A `,` decimal separator is detected only when the delimiter is not `,` and some cell parses as a comma-decimal number. Files that are not valid UTF-8 are read as Windows-1252; UTF-16 (BOM) is supported. Rows with a different number of fields are an error ("baris N: ..."); blank lines are skipped.
- **Numbers:** plain numbers (with optional exponent) only, or with a decimal comma `1.234,5` (`.` accepted as thousands separator only between groups of 3 digits). Anything else (`1.234,5` without decimal comma, `1,5` with a comma delimiter, currency, percentages, dates) makes the column text. Integers with more than 15 significant digits (e.g. 16-digit NIK) would lose precision as numbers, so such a column is text. Leading zeros are lost (`007` becomes 7). Decimals = the maximum seen, capped at 4 (a value like 3.66666666666667 is shown with 4 decimals but keeps all digits). A column of only empty cells becomes numeric (all system-missing).
- **Names:** invalid characters become `_`, a non-letter start gets a `V` prefix, names are cut to 64 characters, empty headers become `VAR00001`, ... (first free number, not the column position) and duplicates (case-insensitive) get `_1`, `_2`. The original header is kept as the variable label when the name differs. SPSS reserved words are not changed. Text cells are not trimmed, except trailing spaces at the end of an unquoted line.
- **XLSX:** date and date-time cells are imported as Excel serial numbers (numeric, no date format, so they show as plain numbers; the 1900/1904 base is taken from the file). Booleans become the text TRUE/FALSE, error cells are empty, a column mixing numbers and text becomes a string variable (numbers are written as the spreadsheet shows them without trailing `.0`). Formulas are read as their cached values. Only the first sheet or the one named is read; the used range starts at the first non-empty cell. `.xls`/`.ods` are opened by `open_any` too (untested).
- **Decimal comma is file-wide:** it is decided once per file, so a `;` file mixing `1,234` thousands-style columns with `78.5` columns turns the columns that do not fit that choice into text (set the option explicitly to override). A file with one column and no delimiter is read as a single column (so `78,5` is 78.5). Import errors read "CSV tidak valid: ..." / "XLSX tidak valid: ..."; an unknown extension is `UnknownType`.
