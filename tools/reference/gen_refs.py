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


# Reference cases are added by later tasks: def case_xxx() -> dict, then append to CASES.
CASES = []


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
