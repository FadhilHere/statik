"""Generate datasets and reference values (from numpy/scipy/statsmodels) for Statik tests.

Run from the repo root:  tools/reference/.venv/Scripts/python tools/reference/gen_refs.py

Each reference case is a function returning {"case": str, "values": {name: number | [number]}}.
Register it in CASES; main() writes fixtures/ref/<case>.json.
"""
import json
import struct
from pathlib import Path

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "fixtures" / "data"
REF = ROOT / "fixtures" / "ref"
SAV = ROOT / "fixtures" / "sav"
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


# ---------------------------------------------------------------------------
# SPSS .sav fixtures (plan 2, task 1).
#
# Each fixture is described by a spec: a list of variable dicts with keys
#   name, type ("numeric" | "string"), width, decimals, label (str | None),
#   value_labels {float: str}, missing (None | {"kind": "discrete", "values": [..]}
#   | {"kind": "range", "lo", "hi", "discrete": float | None}), measure, values.
# The specs are the source of truth for fixtures/ref/sav_basic.json; the .sav files are
# written by pyreadstat (UTF-8) or, for the Windows-1252 / big-endian files that pyreadstat
# cannot produce, by the small hand writer below (layout per the PSPP "System File Format").
# ---------------------------------------------------------------------------
NAN = float("nan")
SAV_DATE = b"18 Nov 26"  # creation date/time are patched so regenerated files are byte-stable
SAV_TIME = b"00:00:00"


def _num(name, width, decimals, label, values, value_labels=None, missing=None, measure="scale"):
    return dict(name=name, type="numeric", width=width, decimals=decimals, label=label,
                value_labels=value_labels or {}, missing=missing, measure=measure, values=values)


def _str(name, label, values, width=None, measure="nominal"):
    if width is None:  # readstat sizes string variables to the longest UTF-8 value
        width = max(1, max(len(v.encode("utf-8")) for v in values))
    return dict(name=name, type="string", width=width, decimals=0, label=label,
                value_labels={}, missing=None, measure=measure, values=values)


def _sav_basic_spec():
    return [
        _num("id", 8, 0, "Nomor responden", [float(i) for i in range(1, 11)]),
        # -0.0 must read back as 0.0; 151/-99 are the bytecode limits, 152/-100 just outside.
        _num("skor", 8, 2, "Skor tes",
             [12.5, -0.0, NAN, 151.0, 152.0, -99.0, -100.0, 3.14159, 1e10, -1.0],
             missing={"kind": "discrete", "values": [-1.0, -2.0, -3.0]}),
        _num("gender", 1, 0, "Jenis kelamin", [1.0, 2.0, 2.0, 1.0, 9.0, 1.0, 2.0, NAN, 1.0, 2.0],
             value_labels={1.0: "Laki-laki", 2.0: "Perempuan", 9.0: "Tidak menjawab"},
             missing={"kind": "discrete", "values": [9.0]}, measure="nominal"),
        _num("puas", 1, 0, "Kepuasan layanan", [1.0, 2.0, 3.0, 4.0, 5.0, 8.0, 0.0, 3.0, NAN, 5.0],
             value_labels={1.0: "Sangat tidak puas", 2.0: "Tidak puas", 3.0: "Netral",
                           4.0: "Puas", 5.0: "Sangat puas", 8.0: "Tidak tahu"},
             missing={"kind": "range", "lo": 7.0, "hi": 9.0, "discrete": 0.0}, measure="ordinal"),
        _num("gaji", 10, 2, "Gaji per bulan (juta) – café",
             [4.75, 12.0, 950.0, 0.5, 2.25, NAN, 7.125, 999.0, 3.0, 1.5],
             missing={"kind": "range", "lo": 900.0, "hi": 999.0, "discrete": None}),
        _num("tanpa", 8, 2, None, [0.25] * 10),
        _str("nama", "Nama responden",
             ["Ani", "Budi Santoso", "", "Café Ünal", " spasi depan", "Dewi", "Eko", "Fajar", "Gita", "Hana"]),
    ]


def _sav_long_names_spec():
    names = ["pendapatan_bulanan_rumah_tangga_ayah_ibu", "pendapatan_bulanan_rumah_tangga_saudara1",
             "Kepuasan_Terhadap_Layanan_Perpustakaan_X"]
    assert all(len(n) == 40 for n in names)
    return [
        _num(names[0], 8, 2, "Pendapatan orang tua", [1.5, 2.5, 3.5]),
        _num(names[1], 8, 2, "Pendapatan saudara", [4.0, NAN, 6.0]),
        _num(names[2], 1, 0, None, [1.0, 2.0, 3.0], value_labels={1.0: "Rendah", 3.0: "Tinggi"},
             measure="ordinal"),
    ]


def _sav_long_string_spec():
    long300 = "".join(chr(ord("a") + i % 26) for i in range(300))
    return [
        _num("nomor", 8, 0, None, [1.0, 2.0, 3.0, 4.0]),
        _str("komentar", "Komentar terbuka (300 karakter)",
             [long300, "Pendek", "", "x" * 250 + "é akhir"]),
        _num("nilai", 8, 2, "Nilai", [3.5, NAN, -2.0, 1e-3]),
        _str("kode", None, ["A1", "B22", "", "D4444"]),
    ]


def _sav_cp1252_spec():
    return [
        _str("kota", "Kota asal (é)", ["Café", "Müller", "Zürich", "", "Señor"], width=10),
        _num("nilai", 5, 1, "Nilai ujian – über", [1.0, 2.0, 99.0, NAN, 1.5],
             value_labels={1.0: "Très bien é", 2.0: "Gut ü"},
             missing={"kind": "discrete", "values": [99.0]}, measure="ordinal"),
    ]


def _write_sav_pyreadstat(path, spec, row_compress):
    import pyreadstat

    df = pd.DataFrame({v["name"]: v["values"] for v in spec})
    missing = {}
    for v in spec:
        m = v["missing"]
        if m and m["kind"] == "discrete":
            missing[v["name"]] = list(m["values"])
        elif m:
            missing[v["name"]] = [{"lo": m["lo"], "hi": m["hi"]}]
            if m["discrete"] is not None:
                missing[v["name"]].append(m["discrete"])
    pyreadstat.write_sav(
        df, str(path), row_compress=row_compress,
        column_labels={v["name"]: v["label"] for v in spec},
        variable_value_labels={v["name"]: v["value_labels"] for v in spec if v["value_labels"]},
        missing_ranges=missing,
        variable_measure={v["name"]: v["measure"] for v in spec},
        variable_format={v["name"]: f"F{v['width']}.{v['decimals']}" for v in spec if v["type"] == "numeric"},
    )
    raw = bytearray(path.read_bytes())
    raw[92:101] = SAV_DATE
    raw[101:109] = SAV_TIME
    path.write_bytes(bytes(raw))
    # Make sure readstat stored what the spec (and so the JSON) claims.
    _, meta = pyreadstat.read_sav(str(path), metadataonly=True)
    for v in spec:
        want = f"F{v['width']}.{v['decimals']}" if v["type"] == "numeric" else f"A{v['width']}"
        got = meta.original_variable_types[v["name"]]
        assert got == want, (path.name, v["name"], got, want)


def _write_sav_by_hand(path, spec, endian, charcode, encoding_name=None):
    """Uncompressed .sav in Windows-1252 with the given byte order ("<" or ">")."""
    enc = "cp1252"
    sysmis = -1.7976931348623157e308

    def i32(x):
        return struct.pack(endian + "i", x)

    def f64(x):
        return struct.pack(endian + "d", x)

    def slots(v):
        return 1 if v["type"] == "numeric" else (v["width"] + 7) // 8

    n = len(spec[0]["values"])
    out = bytearray(b"$FL2" + b"@(#) SPSS DATA FILE - Statik gen_refs hand writer".ljust(60))
    out += i32(2) + i32(sum(slots(v) for v in spec)) + i32(0) + i32(0) + i32(n) + f64(100.0)
    out += SAV_DATE + SAV_TIME + b" " * 64 + b"\0" * 3
    index = {}
    pos = 1
    for v in spec:
        index[v["name"]] = pos
        pos += slots(v)
        numeric = v["type"] == "numeric"
        fmt = ((5 << 16) | (v["width"] << 8) | v["decimals"]) if numeric else ((1 << 16) | (v["width"] << 8))
        m = v["missing"]
        miss = []
        if m and m["kind"] == "discrete":
            miss = list(m["values"])
            nmiss = len(miss)
        elif m:
            miss = [m["lo"], m["hi"]] + ([m["discrete"]] if m["discrete"] is not None else [])
            nmiss = -len(miss)
        else:
            nmiss = 0
        out += i32(2) + i32(0 if numeric else v["width"]) + i32(1 if v["label"] else 0) + i32(nmiss)
        out += i32(fmt) + i32(fmt) + v["name"].encode(enc).ljust(8)
        if v["label"]:
            lb = v["label"].encode(enc)
            out += i32(len(lb)) + lb + b" " * (-len(lb) % 4)
        for x in miss:
            out += f64(x)
        for _ in range(slots(v) - 1):
            out += i32(2) + i32(-1) + i32(0) * 4 + b"\0" * 8
    for v in spec:
        if v["value_labels"]:
            out += i32(3) + i32(len(v["value_labels"]))
            for val, lab in sorted(v["value_labels"].items()):
                lb = lab.encode(enc)
                out += f64(val) + bytes([len(lb)]) + lb + b" " * (-(len(lb) + 1) % 8)
            out += i32(4) + i32(1) + i32(index[v["name"]])
    out += i32(7) + i32(3) + i32(4) + i32(8)
    out += b"".join(i32(x) for x in (20, 0, 0, -1, 1, 0, 1 if endian == ">" else 2, charcode))
    out += i32(7) + i32(4) + i32(8) + i32(3) + f64(sysmis) + f64(1.7976931348623157e308)
    out += struct.pack(endian + "Q", 0xFFEFFFFFFFFFFFFE)
    measure_code = {"nominal": 1, "ordinal": 2, "scale": 3}
    out += i32(7) + i32(11) + i32(4) + i32(3 * len(spec))
    for v in spec:
        out += i32(measure_code[v["measure"]]) + i32(8) + i32(1 if v["type"] == "numeric" else 0)
    if encoding_name:
        out += i32(7) + i32(20) + i32(1) + i32(len(encoding_name)) + encoding_name.encode("ascii")
    out += i32(999) + i32(0)
    for row in range(n):
        for v in spec:
            x = v["values"][row]
            if v["type"] == "numeric":
                out += f64(sysmis if x != x else x)
            else:
                out += x.encode(enc).ljust(slots(v) * 8, b" ")
    path.write_bytes(bytes(out))


def _json_spec(spec):
    def num(x):
        return None if x != x else x + 0.0  # NaN -> null, -0.0 -> 0.0

    out = []
    for v in spec:
        m = v["missing"]
        if m and m["kind"] == "discrete":
            m = {"kind": "discrete", "values": [num(x) for x in m["values"]]}
        out.append({
            "name": v["name"], "type": v["type"], "width": v["width"], "decimals": v["decimals"],
            "label": v["label"],
            "value_labels": [[k, lab] for k, lab in sorted(v["value_labels"].items())],
            "missing": m, "measure": v["measure"],
            "values": [num(x) for x in v["values"]] if v["type"] == "numeric" else list(v["values"]),
        })
    return {"n_cases": len(spec[0]["values"]), "variables": out}


def write_sav_fixtures():
    SAV.mkdir(parents=True, exist_ok=True)
    files = {}
    for fname, spec, compress in [
        ("basic.sav", _sav_basic_spec(), True),
        ("uncompressed.sav", _sav_basic_spec(), False),
        ("long_names.sav", _sav_long_names_spec(), True),
        ("long_string.sav", _sav_long_string_spec(), True),
    ]:
        _write_sav_pyreadstat(SAV / fname, spec, compress)
        files[fname] = _json_spec(spec)
    # cp1252.sav: no subtype 20, character code 1252 in subtype 3 decides the encoding.
    _write_sav_by_hand(SAV / "cp1252.sav", _sav_cp1252_spec(), "<", 1252)
    files["cp1252.sav"] = _json_spec(_sav_cp1252_spec())
    # bigendian.sav: big-endian layout; subtype 20 ("windows-1252") overrides character code 65001.
    _write_sav_by_hand(SAV / "bigendian.sav", _sav_cp1252_spec(), ">", 65001, "windows-1252")
    files["bigendian.sav"] = _json_spec(_sav_cp1252_spec())
    out = {"case": "sav_basic", "files": files}
    (REF / "sav_basic.json").write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    write_sav_metadata_fixture()


def write_sav_metadata_fixture():
    """metadata.sav: dictionary features beyond sav_basic (final review of plan 2).

    Weight variable (bobot), a DATE11 and a DOLLAR10.2 variable, string value labels and string
    missing values on a short (A1) and a long (A15, subtypes 21/22) string, a file label and two
    document lines. Expected values are asserted in crates/sav-io/tests/sav.rs
    (metadata_fixture_*). pyreadstat cannot set the weight, so the header field (offset 76,
    dictionary index of bobot = 2) is patched like the creation date.
    """
    import datetime
    import pyreadstat

    path = SAV / "metadata.sav"
    df = pd.DataFrame({
        "id": [1.0, 2.0, 3.0],
        "bobot": [1.5, 2.0, 0.5],
        "tgl": [datetime.date(2026, 1, 1), datetime.date(2026, 1, 2), None],
        "harga": [1250.5, 99.0, NAN],
        "kode": ["L", "P", "X"],
        "kota": ["Jakarta Selatan", "Bandung", "x"],
    })
    pyreadstat.write_sav(
        df, str(path), row_compress=True, file_label="Survei kepuasan",
        note=["Dokumen baris satu", "baris dua"],
        column_labels={"bobot": "Bobot sampel", "tgl": "Tanggal survei"},
        variable_value_labels={"kode": {"L": "Laki-laki", "P": "Perempuan"}, "kota": {"Bandung": "Kota Bandung"}},
        missing_ranges={"kode": ["X"], "kota": ["x"]},
        variable_measure={"id": "scale", "bobot": "scale", "tgl": "scale", "harga": "scale",
                          "kode": "nominal", "kota": "nominal"},
        variable_format={"id": "F8.0", "bobot": "F8.2", "tgl": "DATE11", "harga": "DOLLAR10.2"},
    )
    raw = bytearray(path.read_bytes())
    raw[76:80] = struct.pack("<i", 2)  # weight = dictionary index of bobot
    raw[92:101] = SAV_DATE
    raw[101:109] = SAV_TIME
    path.write_bytes(bytes(raw))
    _, meta = pyreadstat.read_sav(str(path), metadataonly=True)
    assert meta.original_variable_types == {"id": "F8.0", "bobot": "F8.2", "tgl": "DATE11", "harga": "DOLLAR10.2",
                                            "kode": "A1", "kota": "A15"}, meta.original_variable_types


def write_import_fixtures():
    """CSV/XLSX import fixtures (plan 2, task 3)."""
    # Indonesian-locale Excel: UTF-8 BOM, ';' separator, ',' decimals, CRLF.
    indo = ("Jenis Kelamin;Nilai Ujian;Umur\r\n"
            "1;78,5;20\r\n2;82,25;21\r\n1;90;19\r\n2;65,75;22\r\n1;;20\r\n")
    (DATA / "indo_semicolon.csv").write_bytes(b"\xef\xbb\xbf" + indo.encode("utf-8"))
    # Empty, duplicate, digit-leading and over-long (70 characters) headers.
    messy = ("," + "Nama,Nama,1abc," + "a" * 70 + "\n"
             "1,Ani,x,5,2.5\n2,Budi,y,6,3\n")
    (DATA / "messy_headers.csv").write_bytes(messy.encode("utf-8"))
    # sample.xlsx: sheet "Data" (numbers, text, a date column) and sheet "Kosong" (empty cells).
    data = pd.DataFrame({
        "Jenis Kelamin": [1, 2, 1, 2],
        "Nama": ["Ani", "Budi", "Citra", "Dewi"],
        "Nilai": [78.5, 82.25, 90.0, 65.75],
        "Tanggal": pd.to_datetime(["2026-01-01", "2026-01-02", "2026-01-03", "2026-01-04"]),
    })
    kosong = pd.DataFrame({"a": [1.0, None, 3.0], "b": ["x", "y", None], "c": [10.5, 20.0, 30.25]})
    with pd.ExcelWriter(DATA / "sample.xlsx", engine="openpyxl") as xw:
        data.to_excel(xw, sheet_name="Data", index=False)
        kosong.to_excel(xw, sheet_name="Kosong", index=False)


def main():
    DATA.mkdir(parents=True, exist_ok=True)
    REF.mkdir(parents=True, exist_ok=True)
    write_skripsi60()
    write_edge_cases()
    for fn in CASES:
        out = fn()
        (REF / f"{out['case']}.json").write_text(json.dumps(out, indent=2) + "\n")
    write_sav_fixtures()
    write_import_fixtures()


if __name__ == "__main__":
    main()
