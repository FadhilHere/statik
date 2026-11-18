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


# Reference cases are added by later tasks: def case_xxx() -> dict, then append to CASES.
CASES = [case_describe_skripsi60]


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
