"""Read a .sav file with pyreadstat and print its contents as JSON (used by the sav-io tests).

Usage:  tools/reference/.venv/Scripts/python -I tools/reference/check_sav.py <file.sav>

Output: {"encoding", "n_cases", "variables": [{"name", "label", "format", "value_labels": [[v, label]],
"missing": [{"lo", "hi"}], "measure", "values"}]}. NaN (system-missing) is printed as null; user-missing
values are kept as values (user_missing=True).
"""
import json
import math
import sys

import pyreadstat


def _num(x):
    x = float(x)
    return None if math.isnan(x) else x


def main(path):
    df, meta = pyreadstat.read_sav(path, user_missing=True)
    variables = []
    for name in meta.column_names:
        col = df[name]
        fmt = meta.original_variable_types[name]
        is_string = fmt.startswith("A")
        values = [("" if v is None or (isinstance(v, float) and math.isnan(v)) else str(v)) for v in col] \
            if is_string else [_num(v) for v in col]
        variables.append({
            "name": name,
            "label": meta.column_names_to_labels.get(name),
            "format": fmt,
            "value_labels": sorted([[_num(k), v] for k, v in meta.variable_value_labels.get(name, {}).items()],
                                   key=lambda kv: kv[0]),
            "missing": [{"lo": _num(r["lo"]), "hi": _num(r["hi"])} for r in meta.missing_ranges.get(name, [])],
            "measure": meta.variable_measure.get(name),
            "values": values,
        })
    out = {"encoding": meta.file_encoding, "n_cases": int(meta.number_rows), "variables": variables}
    sys.stdout.write(json.dumps(out, ensure_ascii=True))


if __name__ == "__main__":
    main(sys.argv[1])
