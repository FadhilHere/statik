"""Generate datasets and reference values (from numpy/scipy/statsmodels) for Statik tests.

Run from the repo root:  tools/reference/.venv/Scripts/python tools/reference/gen_refs.py

Each reference case is a function returning {"case": str, "values": {name: number | [number]}}.
Register it in CASES; main() writes fixtures/ref/<case>.json.
"""
import json
from pathlib import Path

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "fixtures" / "data"
REF = ROOT / "fixtures" / "ref"
SEED = 20261118


def write_skripsi60():
    rng = np.random.default_rng(SEED)
    n = 60
    x1 = rng.normal(50, 10, n)
    x2 = rng.normal(30, 5, n)
    x3 = np.round(rng.uniform(1, 5, n), 2)
    y = np.round(5 + 0.6 * x1 + 0.9 * x2 + 2 * x3 + rng.normal(0, 4, n), 2)
    grp = np.where(np.arange(n) < 30, 1, 2)
    df = pd.DataFrame({"x1": np.round(x1, 2), "x2": np.round(x2, 2), "x3": x3, "y": y, "grp": grp})
    df.to_csv(DATA / "skripsi60.csv", index=False)


# Static datasets (longley.csv, sleep.csv) are committed as-is.
# edge_cases.csv is written here so its construction is documented; empty cell = NaN.
def write_edge_cases():
    x1 = np.arange(1, 13)
    noise = [0.5, -0.3, 0.2, -0.6, 0.1, 0.4, -0.2, 0.3, -0.5, 0.0, 0.6, -0.4]
    withmiss = [10.5, 11.2, np.nan, 12.8, 99.0, 13.1, 12.0, np.nan, 14.2, 13.5, 15.1, 14.8]
    df = pd.DataFrame({
        "y": 3 + 2 * x1 + np.array(noise),
        "const": 7,
        "x1": x1,
        "x1dbl": 2 * x1,
        "withmiss": withmiss,
    })
    df.to_csv(DATA / "edge_cases.csv", index=False, na_rep="")


def case_describe_skripsi60():
    from scipy import stats

    x = pd.read_csv(DATA / "skripsi60.csv")["x1"].to_numpy(dtype=float)
    n = len(x)
    xs = np.sort(x)
    # SPSS EXAMINE 5% trimmed mean: remove 0.05*n of mass from each end (fractional boundary case).
    t = 0.05 * n
    w = np.ones(n)
    rem = t
    for i in range(n):
        d = min(1.0, rem)
        w[i] -= d
        rem -= d
    rem = t
    for i in range(n - 1, -1, -1):
        d = min(1.0, rem)
        w[i] -= d
        rem -= d
    trimmed = float((w * xs).sum() / w.sum())
    q = [float(np.percentile(x, p, method="weibull")) for p in (25, 50, 75)]
    return {
        "case": "describe_skripsi60",
        "values": {
            "mean": float(x.mean()),
            "var": float(x.var(ddof=1)),
            "skew": float(stats.skew(x, bias=False)),
            "kurt": float(stats.kurtosis(x, bias=False)),
            "p25": q[0],
            "p50": q[1],
            "p75": q[2],
            "trimmed5": trimmed,
        },
    }


def case_ols_skripsi60():
    import statsmodels.api as sm
    from statsmodels.stats.outliers_influence import variance_inflation_factor
    from statsmodels.stats.stattools import durbin_watson

    df = pd.read_csv(DATA / "skripsi60.csv")
    X = sm.add_constant(df[["x1", "x2", "x3"]].to_numpy(dtype=float))
    y = df["y"].to_numpy(dtype=float)
    fit = sm.OLS(y, X).fit()
    infl = fit.get_influence()
    return {
        "case": "ols_skripsi60",
        "values": {
            "params": [float(v) for v in fit.params],
            "bse": [float(v) for v in fit.bse],
            "tvalues": [float(v) for v in fit.tvalues],
            "pvalues": [float(v) for v in fit.pvalues],
            "rsquared": float(fit.rsquared),
            "rsquared_adj": float(fit.rsquared_adj),
            "fvalue": float(fit.fvalue),
            "f_pvalue": float(fit.f_pvalue),
            "durbin_watson": float(durbin_watson(fit.resid)),
            "vif": [float(variance_inflation_factor(X, i)) for i in range(1, X.shape[1])],
            "sresid": [float(v) for v in infl.resid_studentized_internal],
        },
    }


def case_normality():
    import statsmodels.api as sm
    from scipy import stats
    from statsmodels.stats.diagnostic import lilliefors

    df = pd.read_csv(DATA / "skripsi60.csv")
    X = sm.add_constant(df[["x1", "x2", "x3"]].to_numpy(dtype=float))
    resid = sm.OLS(df["y"].to_numpy(dtype=float), X).fit().resid
    samples = {
        "x1": df["x1"].to_numpy(dtype=float),
        "resid": np.asarray(resid, dtype=float),
        "sleep_extra": pd.read_csv(DATA / "sleep.csv")["extra"].to_numpy(dtype=float),
    }
    # Small samples exercise the n = 3 (exact), 4..11 and n <= 5 coefficient branches of AS R94.
    for k in (3, 4, 5, 10):
        samples[f"sleep_{k}"] = samples["sleep_extra"][:k]
    # Skewed samples with small Lilliefors p; n = 150 exercises the n > 100 scaling.
    samples["skew30"] = np.round(np.exp(np.arange(1, 31) / 6.0), 4)
    samples["skew150"] = np.round(10 * (np.arange(1, 151) / 151.0) ** 3, 4)
    values = {}
    for name, x in samples.items():
        sw = stats.shapiro(x)
        values[name] = [float(v) for v in x]
        values[f"{name}_sw_w"] = float(sw.statistic)
        values[f"{name}_sw_p"] = float(sw.pvalue)
        if len(x) >= 5:
            d, p = lilliefors(x, dist="norm", pvalmethod="approx")
            values[f"{name}_lf_d"] = float(d)
            values[f"{name}_lf_p"] = float(p)
    return {"case": "normality", "values": values}


def case_ttest():
    from scipy import stats

    sk = pd.read_csv(DATA / "skripsi60.csv")
    x1 = sk["x1"].to_numpy(dtype=float)
    g1 = sk.loc[sk["grp"] == 1, "x1"].to_numpy(dtype=float)
    g2 = sk.loc[sk["grp"] == 2, "x1"].to_numpy(dtype=float)
    x2 = sk["x2"].to_numpy(dtype=float)
    one = stats.ttest_1samp(x1, 50.0)
    ci1 = one.confidence_interval(0.95)
    lev = stats.levene(g1, g2, center="mean")
    eq = stats.ttest_ind(g1, g2, equal_var=True)
    we = stats.ttest_ind(g1, g2, equal_var=False)
    pr = stats.ttest_rel(x1, x2)
    corr = stats.pearsonr(x1, x2)
    return {
        "case": "ttest",
        "values": {
            "x1": [float(v) for v in x1],
            "g1": [float(v) for v in g1],
            "g2": [float(v) for v in g2],
            "x2": [float(v) for v in x2],
            "one_t": float(one.statistic), "one_df": float(one.df), "one_p": float(one.pvalue),
            "one_ci": [float(ci1.low), float(ci1.high)],
            "levene_f": float(lev.statistic), "levene_p": float(lev.pvalue),
            "eq_t": float(eq.statistic), "eq_df": float(eq.df), "eq_p": float(eq.pvalue),
            "welch_t": float(we.statistic), "welch_df": float(we.df), "welch_p": float(we.pvalue),
            "paired_t": float(pr.statistic), "paired_df": float(pr.df), "paired_p": float(pr.pvalue),
            "corr": float(corr.statistic), "corr_p": float(corr.pvalue),
        },
    }


def case_anova():
    from scipy import stats

    sk = pd.read_csv(DATA / "skripsi60.csv")
    g = [sk.loc[sk["grp"] == k, "y"].to_numpy(dtype=float) for k in (1, 2)]
    r = stats.f_oneway(*g)
    allv = np.concatenate(g)
    gm = allv.mean()
    ssb = float(sum(len(a) * (a.mean() - gm) ** 2 for a in g))
    ssw = float(sum(((a - a.mean()) ** 2).sum() for a in g))
    lev = stats.levene(*g, center="mean")
    return {
        "case": "anova",
        "values": {
            "g1": [float(v) for v in g[0]], "g2": [float(v) for v in g[1]],
            "f": float(r.statistic), "p": float(r.pvalue), "ssb": ssb, "ssw": ssw,
            "levene_f": float(lev.statistic), "levene_p": float(lev.pvalue),
        },
    }


def case_runs():
    import statsmodels.api as sm
    from scipy import stats

    df = pd.read_csv(DATA / "skripsi60.csv")
    X = sm.add_constant(df[["x1", "x2", "x3"]].to_numpy(dtype=float))
    resid = np.asarray(sm.OLS(df["y"].to_numpy(dtype=float), X).fit().resid, dtype=float)
    n = len(resid)
    med = float(np.percentile(resid, 50, method="weibull"))  # HAVERAGE median
    below = resid < med
    n1 = int(below.sum())
    n2 = n - n1
    runs = 1 + int((below[1:] != below[:-1]).sum())
    mu = 2.0 * n1 * n2 / n + 1.0
    var = 2.0 * n1 * n2 * (2.0 * n1 * n2 - n) / (n * n * (n - 1.0))
    d = runs - mu
    if n < 50:
        d = 0.0 if abs(d) < 0.5 else d - 0.5 * np.sign(d)
    z = d / np.sqrt(var)
    return {
        "case": "runs",
        "values": {
            "resid": [float(v) for v in resid], "median": med, "n_below": n1, "n_above": n2,
            "runs": runs, "z": float(z), "p": float(2 * stats.norm.sf(abs(z))),
        },
    }


def case_t_quantile_large_df():
    from scipy import stats

    dfs = [1e5, 1e6]
    ps = [0.95, 0.975, 0.995]
    return {
        "case": "t_quantile_large_df",
        "values": {
            "df": dfs,
            "p": ps,
            "q": [[float(stats.t.ppf(p, df)) for p in ps] for df in dfs],
        },
    }


# Reference cases are added by later tasks: def case_xxx() -> dict, then append to CASES.
CASES = [case_describe_skripsi60, case_ols_skripsi60, case_normality, case_ttest, case_anova, case_runs,
         case_t_quantile_large_df]


def main():
    DATA.mkdir(parents=True, exist_ok=True)
    REF.mkdir(parents=True, exist_ok=True)
    write_skripsi60()
    write_edge_cases()
    for fn in CASES:
        out = fn()
        (REF / f"{out['case']}.json").write_text(json.dumps(out, indent=2) + "\n")


if __name__ == "__main__":
    main()
