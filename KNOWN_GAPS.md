# Known gaps

Deviations from SPSS and unimplemented features are recorded here by the tasks that find them.

## Regression (Task 5)

- **Decimals — verify with the user's SPSS screenshots.** Model Summary: R, R Square, Adjusted R Square `Fixed(3)`; Std. Error of the Estimate `Fixed(5)`; Durbin-Watson `Fixed(3)`. ANOVA: Sum of Squares, Mean Square, F `Fixed(3)`; df `Count`; Sig. `Sig`. Coefficients: B, Std. Error, Beta, t `Fixed(3)`; Sig. `Sig`; Tolerance, VIF `Fixed(3)`. Excluded Variables: Beta In, t, Partial Correlation, Tolerance, VIF, Minimum Tolerance `Fixed(3)`; Sig. `Sig`. Collinearity Diagnostics: Eigenvalue, Condition Index `Fixed(3)`; Variance Proportions `Fixed(2)`. Residuals Statistics: Minimum, Maximum, Mean, Std. Deviation `Fixed(4)`; N `Count`. Saved variables: width 11, 5 decimals.
- **Normal P-P plot:** observed cumulative probability = `i/(n+1)` for the i-th smallest ZRESID; expected = `Φ(ZRESID)`. The exact plotting position SPSS REGRESSION uses was not confirmed against IBM SPSS Algorithms; verify visually.
- **Variables Entered / Predictors footnotes** list predictors last-entered first (as SPSS usually prints them); verify ordering.
- **Residuals Statistics** shows only Predicted Value, Residual, Std. Predicted Value, Std. Residual. SPSS adds rows (Std. Error of Predicted Value, Adjusted Predicted Value, Stud. Residual, Deleted Residual, Stud. Deleted Residual, Mahal./Cook's distance, Centered Leverage) when plots/saves involve SRESID etc.
- **Excluded Variables:** for an exact linear combination (tolerance < 1e-10) Beta In, t, Sig., Partial Correlation and VIF are shown as "."; the entered-variables footnote becomes "Tolerance = .000 limit reached." Wording and which cells SPSS blanks are unverified.
- Footnote markers sit where SPSS shows them: table titles carry `title_footnotes` (Variables Entered/Removed^a, Model Summary^b, ANOVA^a, Coefficients^a, Excluded Variables^a, Collinearity Diagnostics^a, Residuals Statistics^a); the Predictors note `b` of ANOVA is on the Sig. cell of the Regression row (`.000b`). Chart titles are one line; "Dependent Variable: ..." is the chart subtitle.
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
- **Footnote markers:** In Tests of Normality the lower-bound note is `*` and "Lilliefors Significance Correction" is `a`; the `a` is attached to the "Kolmogorov-Smirnov" column header. In the 1-Sample K-S table `a,b` is on the "Normal Parameters" row header and the Asymp. Sig. cell of each variable carries `c` and, only for a lower bound, `d` (SPSS shows `.200c,d`, `.041c`). The `c`/`d` placement in SPSS 27+ is not confirmed.
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
- **Runs test:** the Test Value uses variable decimals + 2 (unverified). The `a` ("Median") marker is on the "Test Value" row header. A constant variable (all cases on one side) is omitted with a SPSS-like warning (wording unverified). The continuity correction for n < 50 follows the brief; the exact n threshold in SPSS is unverified. Only the median cut point is supported.
- **Descriptives:** Min/Max use the variable decimals, Mean + 2, Std. Deviation + 3, skewness/kurtosis `Fixed(3)` (unverified). Each variable uses its own valid cases; "Valid N (listwise)" uses complete cases. Z scores are computed per variable from its own valid cases. `Z<name>` is created; when the name is taken or invalid `ZSC001`, `ZSC002`, ... are used. A constant variable gets an all-system-missing Z column and a warning. The Z variable's width (11) and label ("Zscore(<name>)" / "Zscore:  <label>") are unverified.
- **Frequencies:** the table title is the variable label (else name); the SPSS "Statistics" table only has N Valid / Missing. Missing group lists user-missing values (labelled like valid values), "System" for NaN and a Total row, followed by the grand Total (Percent only). The histogram has no normal curve (option not offered). Percentiles, mean etc. are not implemented.

## .sav read/write (plan 2, Task 1)

- **Read report:** `read_sav` / `open_any` return `(Dataset, ReadReport)`; `ReadReport.notes` are English sentences for the user about anything Statik shows differently or does not keep (see below). CSV/XLSX imports return an empty report.
- **Kept and written back:** the weight variable (`Dataset::weight`), file label, documents (record 6), string value labels (record 3 for strings up to 8 bytes, record 7 subtype 21 for longer ones), string missing values (record 2 / subtype 22; at most 3, each at most 8 bytes, otherwise saving is an `Unsupported` error), and the format type of numeric variables (DATE, ADATE, DOLLAR, COMMA, ...; `Variable::format_type`). Only the print format is read; it is written as both print and write format.
- **Weight is not applied:** analyses are unweighted. Reading a file with a weight adds the note "Weight variable X is ignored by Statik analyses (results are unweighted)."
- **Dates and times are shown as numbers:** a date format adds the note "Variable X has a date format; dates are shown as numbers (seconds since 1582-10-14)."; TIME/DTIME/MTIME add a similar note. The format is still written back unchanged. COMMA/DOLLAR and other display formats are kept without a note.
- **Not kept (with a note):** multiple response sets (subtypes 7/19), custom (user) file and variable attributes (subtypes 17/18; the note appears only when an attribute name does not start with `$@`, so SPSS-internal attributes such as `$@Role` are ignored silently), variable sets (subtype 5). **Not kept (silently):** extended case count (subtype 16), Columns and Align from subtype 11 (Columns is written as the variable's display width).
- **Formats:** numeric width is clamped to 1–40 and decimals to ≤16 (and < width) on write.
- **`.zsav`** (zlib compression) is reported as unsupported. Floating-point formats other than IEEE (VAX/IBM) are reported as unsupported.
- **Writing:** always UTF-8, little-endian, bytecode-compressed. Variable labels are cut to 255 bytes, value labels to 120 bytes, the file label to 64 bytes and document lines to 80 bytes (on a character boundary). More than three discrete missing values is an `Unsupported` error. The string storage width is `VarType::String { width }` (bytes); it is raised to the longest UTF-8 value if a value is longer (`set_text` does not cut values; `Dataset::set_string_width` does). `Variable::width` is the display width (Columns, max 255), Align is right for numbers / left for strings, and no Role.
- **Saving is atomic:** the file is written to a uniquely named `<name>.<pid>-<nanos>.tmp` in the same folder (a user's own `<name>.tmp` is never touched), synced and renamed over the target. If anything fails (including a target locked by another program) the old file is left as it was and the temporary file is removed. A dataset whose column does not match its variable type is `SavError::Internal` ("Cannot save: ...").
- **Very long strings (> 255 bytes):** the value is split into consecutive 255-byte pieces, as ReadStat writes and reads it; the PSPP manual's wording suggests 252 data bytes per segment. Verified only against ReadStat (pyreadstat), not against a file saved by SPSS itself.
- **Short names** are generated as the upper-cased first 8 bytes of the name, with `_1`, `_2`, ... on collisions or for reserved words (ALL, AND, BY, EQ, GE, GT, LE, LT, NE, NOT, OR, TO, WITH). The full name goes in record 7 subtype 13. Reserved words are not valid variable names in Statik (`validate_name`), so a long name is never one.
- **Reading names:** names follow the import rules (`sanitize_name`): invalid characters become `_`, a non-letter start gets a `V` prefix, names are cut to 64 bytes, and a duplicate or reserved word gets `_1`, `_2`, ... Every renamed variable adds the note "Variable name 'X' is not valid in Statik; it was renamed to 'Y'." Trailing spaces in string values, value-label values and missing values are dropped.
- Compatibility of written files is tested with pyreadstat (ReadStat), not with SPSS itself.

## CSV / XLSX import (plan 2, Task 3)

- **Detection:** the delimiter is the one of `,` `;` tab that splits the first 20 lines into the same number (> 1) of fields, most fields winning. Files that are not valid UTF-8 are read as Windows-1252; UTF-16 (BOM) is supported. Rows with a different number of fields are an error ("line N has M fields, expected K"); blank lines are skipped. `sniff_csv` reports what was detected (delimiter, decimal comma, names, first rows) for a preview.
- **Decimal separator rule** (when not set in `CsvOptions`):
  - `,` delimiter: `.` is the decimal point.
  - `;` or tab delimiter: `,` is the decimal separator and `.` the thousands separator (Indonesian Excel), so `750.000` is 750000 and `78,5` is 78.5. The file switches to decimal points only if some cell is valid only with a decimal point (`78.5`, `3.14159`: not groups of three digits) **and** no cell is valid only with a decimal comma (`78,5`, `1.234,5`). So a `;` file containing just `1.250` and integers reads `1.250` as 1250, never as 1.25.
  - No delimiter (one column): `,` is the decimal separator only if some cell needs it (`78,5`); otherwise `.` is the decimal point.
  - The decision is file-wide: in a `;` file that has both `78,5` and `1.5`, the column with `1.5` becomes text.
- **Numbers:** plain numbers (with optional exponent) only, or with a decimal comma `1.234,5` (`.` accepted as thousands separator only between groups of 3 digits). Anything else (`1.234,5` without decimal comma, `1,5` with a comma delimiter, currency, percentages, dates) makes the column text. Integers with more than 15 significant digits (e.g. 16-digit NIK) would lose precision as numbers, so such a column is text. Leading zeros are lost (`007` becomes 7). Decimals = the maximum seen, capped at 4 (a value like 3.66666666666667 is shown with 4 decimals but keeps all digits). A column of only empty cells becomes numeric (all system-missing).
- **Names:** invalid characters become `_`, a non-letter start gets a `V` prefix, names are cut to 64 bytes (UTF-8, on a character boundary), empty headers become `VAR00001`, ... (first free number, not the column position) and duplicates (case-insensitive) and SPSS reserved words (ALL, AND, BY, EQ, GE, GT, LE, LT, NE, NOT, OR, TO, WITH) get `_1`, `_2`. The original header is kept as the variable label when the name differs. Text cells are not trimmed, except trailing spaces at the end of an unquoted line. `NA` / `-` are not treated as missing (the column becomes text).
- **XLSX:** date and date-time cells are imported as Excel serial numbers (numeric, no date format, so they show as plain numbers; the 1900/1904 base is taken from the file). Booleans become the text TRUE/FALSE, error cells are empty, a column mixing numbers and text becomes a string variable (numbers are written as the spreadsheet shows them without trailing `.0`). Formulas are read as their cached values. Only the first sheet or the one named is read; the used range starts at the first non-empty cell. `.xls`/`.ods` are opened by `open_any` too (untested).
- **Errors** are English: "Invalid CSV: ...", "Invalid XLSX: ..." (e.g. "sheet 'x' not found"), and an unknown extension is `UnknownType` ("Unsupported file type: .docx"). `xlsx_sheet_names` lists a workbook's sheets.

## Compute Variable (plan 2, Task 4)

- **Logical operands other than 0/1:** in AND, OR, NOT and IF conditions, any non-missing non-zero value counts as true. This has not been checked against SPSS. PSPP treats such values as missing.
- **LAG of a user-missing value** returns system-missing. It is unverified whether SPSS passes the raw value through instead.
- Not implemented: RND/TRUNC fuzzbits argument (default 6 bits is applied), LAG(x, n), string functions, and string targets.
- Expressions are limited to 64 levels of nesting (parentheses, calls, unary operators) and a tree depth of 256; deeper expressions are a syntax error.

## Data model and Recode (plan 2 final review)

- **Names** are limited to 64 UTF-8 bytes and may not be reserved words (`validate_name`), as in SPSS.
- **Type changes:** `Dataset::set_type` returns how many values were lost (non-blank text that is not a number became system-missing; strings cut when narrowing). Text is parsed with a decimal point only (`78,5` becomes system-missing). Numeric -> String uses the requested width, raised to the longest formatted value so nothing is cut.
- **String widths:** `VarType::String { width }` is the storage width; `Variable::width` is only the display width. `Dataset::set_string_width` cuts longer values on a character boundary and reports how many.
- **Recode** rejects NaN rule values and ranges whose lower bound is above the upper bound; output names are compared case-insensitively with full Unicode case folding (as `Dataset::find`).

## Data Editor: Data View & Variable View (Plan 3 Task 3)

- **Align, Role and the Columns of numeric variables are not saved.** The data model has no fields for them, so they are kept in the editor's UI state (keyed by variable name) and are lost on New/Open and not written to `.sav`. Columns of a string variable is its display width and is saved. Changing Align/Role/numeric Columns is not on the undo stack; deleting a variable drops its entries and undoing the delete restores them.
- **Width/Columns of numeric variables:** the model has one width; the Variable View "Width" edits it (the print format width), "Columns" only changes the Data View column width.
- **Copy** puts the values as shown (variable decimals, value labels when View > Value Labels is on) into the clipboard; system-missing becomes an empty cell. Copy with Variable Names/Labels, Paste Variables..., Sort, Find/Replace, Go to Case and Variable Information are not implemented (disabled).
- **Paste** creates new variables as numeric (F8.2) only; text pasted into a numeric column rejects the whole paste. Strings are cut to the storage width. Pasting text with a value label's text while Value Labels is on enters its value.
- **Paste details:** Excel's quoted cells (a cell holding tabs, line breaks or `"`, wrapped in quotes with `""` for a quote) are read as one cell. Trailing columns or rows that are empty everywhere and lie past the data are ignored (a trailing tab does not create a variable). One value pasted into a multi-cell selection fills the whole selection.
- **Number text from Indonesian/European spreadsheets:** `.` is always the decimal point, so `1.234` (Excel showing one thousand two hundred thirty-four with a thousands separator) is read as 1.234; a single comma is a decimal comma (`3,5` = 3.5); text with both (`1.234,5`) is rejected as not a number. Copy unformatted numbers (no thousands separators) from Excel.
- **Copy, Cut and Paste in the Variable View** (copying variable definitions) are not available; Edit > Copy/Cut/Paste act on the Data View only.
- **Invalid numeric input** shows a message box instead of SPSS's beep; the old value is kept. A comma is accepted as the decimal separator when the text has no point and one comma ("3,5").
- **Number display:** values are formatted with the variable's decimals; a number too wide for its column is shown in E notation (`1.00E10`), else as asterisks. Date/time/currency formats kept from `.sav` files are shown as plain numbers (the Type column names the format: Date, Time, Comma, Dollar, ...). The Variable Type dialog only offers Numeric and String.
- **Undo** holds the last 100 edits. New/Open clear it, and a procedure that changes the data (saved variables; later Compute/Recode) clears it as well.
- The Measure, Align and Role icons are Statik's own drawings.

## Output viewer, charts, copy (Plan 3 Task 4)

- **One output document.** The viewer is always "Output1 [Document1]". File ▸ Open Output appends the opened `.stko` entries to the current output (nothing on screen is lost) instead of opening a second viewer. `.stko` is Statik's own JSON (`Vec<OutputEntry>`); `.spv` files are not read or written. Export..., Print, Delete/Hide item, editing pivot tables (pivoting, cell edits, TableLooks) are not available.
- **Bold text** (procedure headings, table and chart titles) is faux bold (drawn twice, one pixel apart): the bundled egui fonts have no bold face. Fonts are egui's default sans-serif, not Arial.
- **Histogram legend:** "Mean = …" uses 2 decimals and "Std. Dev. = …" 3, with a leading zero (`0.983`). SPSS prints a residual mean of about zero in E notation (`Mean = 4.44E-16`); Statik prints `Mean = 0.00`. Bins are Statik's own (about √N bins of a "nice" width), so bar counts can differ from SPSS's binning.
- **Axis captions:** the histogram x caption comes from the chart (`x_label`: "Regression Standardized Residual", or the variable label/name for Frequencies); fixed text elsewhere: Histogram y "Frequency", P-P "Observed Cum Prob" / "Expected Cum Prob" (ticks 0.0-1.0 by 0.2), Q-Q "Observed Value" / "Expected Normal". Histogram x ticks are whole numbers when the data spans 2 or more units. The Q-Q reference line is the fitted normal line z = (x - mean) / sd over the observed range. Charts cannot be edited (no Chart Editor).
- **Copy as Image** fails with a message when no picture of the window arrives within 2 s.
- **Open Output** refuses files over 200 MB.
- **Copy (tables)** puts "HTML Format" (inline-styled table: dark rules above the header and below the last row, light rules elsewhere, title as the first row), "Rich Text Format" (simple RTF table with merged spanning cells) and tab-separated text on the clipboard. Word pastes the HTML. Column widths in the RTF are estimated from character counts.
- **Copy as Image** captures the viewer window (`ViewportCommand::Screenshot`) and crops it to the item: only the part of the item visible in the window is copied (a table taller than the window is cut off), at screen resolution, as CF_DIB (no PNG/EMF). Copy of a chart is always an image.
- The viewer's Window menu has only "Data Editor"; the Data Editor's Window ▸ Output shows/hides the viewer. The embedded fallback (backends without multiple windows, i.e. tests) has no close button.

## Analyze dialogs (Plan 3 Task 5)
- Options Statik's procedures do not compute are shown greyed out (as visible-but-disabled controls): e.g. Regression methods other than Enter, Selection Variable / Case Labels / WLS Weight, Confidence intervals, R squared change; Frequencies Statistics..., Bar/Pie charts and "Show normal curve on histogram"; Explore Factor List, Label Cases by, Statistics..., Options..., boxplots, stem-and-leaf, Display "Plots" (the Descriptives table is always produced); One-Way ANOVA Contrasts..., Post Hoc..., Welch, Brown-Forsythe; K-S distributions other than Normal; Runs cut points other than Median; every Bootstrap..., Style..., Exact....
- Estimates and Model fit (Regression) and Mean/Std. deviation/Minimum/Maximum (Descriptives) are always on (checked, disabled); "Display frequency tables" is always on.
- Descriptives Options...: Kurtosis and Skewness share one switch (the engine reports both together).
- One-Way ANOVA with several dependents runs one analysis per dependent (one "Oneway" heading, separate tables per dependent), whereas SPSS stacks the dependents in one ANOVA table.
- Independent-Samples T Test: the grouping variable must be numeric and Cut point is not available.
- Variable lists: no drag and drop and no keyboard navigation (click, Ctrl/Shift+click, double-click and the arrow buttons only); no right-click "Variable Information"; the "Display variable labels / names" preference is fixed to labels. Pasted syntax opens in a read-only Syntax window (copy only, cannot be run or edited).
