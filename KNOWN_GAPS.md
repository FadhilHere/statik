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
