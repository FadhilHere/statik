"""Read a .sav file with pyreadstat and print its contents as JSON (used by the sav-io tests).

Usage:  tools/reference/.venv/Scripts/python -I tools/reference/check_sav.py <file.sav>

Output: {"encoding", "n_cases", "file_label", "notes", "weight_index", "variables": [{"name", "label",
"format", "value_labels": [[v, label]], "missing": [{"lo", "hi"}], "measure", "values"}]}. NaN
(system-missing) is printed as null; user-missing values are kept as values (user_missing=True); dates are
raw SPSS numbers (seconds since 1582-10-14).
String variables have string value-label keys and missing values. "weight_index" is the raw header
field (1-based dictionary index of the weight variable, 0 = none), which pyreadstat does not expose.
"""
import json
import math
import struct
import sys

import pyreadstat


def _num(x):
    x = float(x)
    return None if math.isnan(x) else x


def _weight_index(path):
    with open(path, "rb") as f:
        head = f.read(80)
    endian = "<" if struct.unpack("<i", head[64:68])[0] in (2, 3) else ">"
    return struct.unpack(endian + "i", head[76:80])[0]


def main(path):
    df, meta = pyreadstat.read_sav(path, user_missing=True, disable_datetime_conversion=True)
    variables = []
    for name in meta.column_names:
        col = df[name]
        fmt = meta.original_variable_types[name]
        is_string = fmt.startswith("A")
        key = (lambda k: k) if is_string else _num
        values = [("" if v is None or (isinstance(v, float) and math.isnan(v)) else str(v)) for v in col] \
            if is_string else [_num(v) for v in col]
        variables.append({
            "name": name,
            "label": meta.column_names_to_labels.get(name),
            "format": fmt,
            "value_labels": sorted([[key(k), v] for k, v in meta.variable_value_labels.get(name, {}).items()],
                                   key=lambda kv: kv[0]),
            "missing": [{"lo": key(r["lo"]), "hi": key(r["hi"])} for r in meta.missing_ranges.get(name, [])],
            "measure": meta.variable_measure.get(name),
            "values": values,
        })
    out = {"encoding": meta.file_encoding, "n_cases": int(meta.number_rows), "file_label": meta.file_label,
           "notes": meta.notes, "weight_index": _weight_index(path), "variables": variables}
    sys.stdout.write(json.dumps(out, ensure_ascii=True))


if __name__ == "__main__":
    main(sys.argv[1])
