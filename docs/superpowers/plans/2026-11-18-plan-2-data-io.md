# Statik Plan 2 — I/O Data & Transform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Statik bisa membuka/menyimpan `.sav`, mengimpor CSV (termasuk format Indonesia) dan XLSX, mengedit struktur dataset dengan aman, serta menjalankan Compute Variable dan Recode into Different Variables — semuanya tanpa UI, siap dipakai GUI di Plan 3.

**Architecture:** Crate baru `sav-io` (baca/tulis `.sav`, impor CSV/XLSX) dan `transform` (parser + evaluator ekspresi Compute, Recode). `dataset` mendapat operasi struktur (insert/delete variable & case) dan validasi `VarId`. Prosedur memvalidasi `VarId` dan mengembalikan `Warning` alih-alih panic.

**Tech Stack:** Rust 1.94, `encoding_rs` (Windows-1252 ↔ UTF-8), `calamine` (XLSX), `csv`. Acuan tes `.sav`: `pyreadstat` di venv `tools/reference/.venv`.

**Spec:** `docs/superpowers/specs/2026-11-18-statik-design.md` (§6.1, §6.2, §6.3). Temuan tertunda dari Plan 1: `docs/superpowers/plan-1-rulings.md`.

## Global Constraints

- System-missing = `f64::NAN`; user-missing via `Variable::missing`; `-0.0` dinormalisasi ke `0.0` saat data masuk (impor, Compute, Recode, edit).
- Nama variabel: diawali huruf, tanpa spasi, ≤ 64 karakter, unik case-insensitive (aturan `dataset::validate_name`).
- `.sav` yang ditulis Statik harus bisa dibaca SPSS: little-endian, kompresi bytecode (bias 100), encoding UTF-8 (record 7 subtype 20 = "UTF-8", subtype 3 character code 65001).
- Hasil komputasi non-finite (÷0, LN(0), SQRT(−1), overflow) menjadi system-missing.
- Tidak ada panic dari data/berkas pengguna: berkas rusak → `Err` dengan pesan yang bisa ditampilkan; `VarId` usang → `Warning`.
- Toleransi tes numerik: eksak untuk nilai yang ditulis lalu dibaca ulang (bit-identik, kecuali NaN ↔ NaN).

## Review Focus

- **Berkas `.sav` dari SPSS versi lama / Windows-1252** (label dengan huruf é, ü): label harus terbaca benar, bukan mojibake. → tes di Task 1 (fixture cp1252 dari pyreadstat).
- **CSV Excel Indonesia** (`;` sebagai pemisah, `,` desimal, header "Jenis Kelamin", BOM): harus terbaca sebagai angka dengan nama variabel tersanitasi (`Jenis_Kelamin`). → tes di Task 3.
- **Compute dengan missing**: `x + y` saat y missing → system-missing; `MEAN(x,y)` saat y missing → rata-rata nilai valid (perilaku SPSS). → tes di Task 4.
- **Menghapus variabel lalu menjalankan prosedur dengan opsi lama** (VarId usang): `Warning`, bukan crash. → tes di Task 2.
- **Recode dengan rentang yang tumpang tindih**: aturan pertama yang cocok menang (SPSS). → tes di Task 5.

---

## File Structure

```
crates/dataset/src/{dataset.rs (+ops), variable.rs}
crates/sav-io/src/{lib.rs, read.rs, write.rs, records.rs, compress.rs, csv_import.rs, xlsx_import.rs, names.rs}
crates/transform/src/{lib.rs, lexer.rs, parser.rs, eval.rs, functions.rs, compute.rs, recode.rs}
crates/procedures/src/cells.rs (+validasi VarId)
fixtures/sav/*.sav  (dibuat gen_refs.py via pyreadstat)
fixtures/data/{indo_semicolon.csv, messy_headers.csv, sample.xlsx}
```

`load_csv` lama di `dataset` tetap ada (dipakai CLI) tetapi CLI dipindah ke `sav_io::import_csv` di Task 3.

---

### Task 1: Baca & tulis `.sav`

**Files:** Create `crates/sav-io/src/{lib.rs, read.rs, write.rs, records.rs, compress.rs}`; Test `crates/sav-io/tests/sav.rs`; Modify `tools/reference/{requirements.txt, gen_refs.py}`

**Interfaces:**
- Produces:
  ```rust
  pub enum SavError { Io(std::io::Error), Format(String), Unsupported(String) }
  pub fn read_sav(path: &Path) -> Result<Dataset, SavError>;
  pub fn write_sav(ds: &Dataset, path: &Path) -> Result<(), SavError>;
  ```
- Baca: record 1 (header; `layout_code` 2 = little-endian, 3 = big-endian → dukung keduanya), record 2 (variabel, termasuk continuation untuk string > 8 byte), record 3/4 (value labels), record 6 (dokumen, diabaikan), record 7 subtype 3, 4, 11 (measure/width/alignment), 13 (nama panjang), 14 (very long string), 20 (encoding), 21/22 diabaikan dengan aman, record 999, lalu data tak-terkompresi atau bytecode (kode 0 = abaikan, 1–251 = nilai − 100, 252 = akhir, 253 = 8 byte mentah berikutnya, 254 = spasi 8 byte, 255 = sysmis). `.zsav` (kompresi zlib, `compression = 2`) → `Err(Unsupported("ZSAV"))`.
- Encoding: subtype 20 bila ada; kalau tidak, subtype 3 character code (65001 = UTF-8, 1252 = Windows-1252); kalau tidak ada keduanya, Windows-1252.
- Format SPSS (print/write format) diterjemahkan ke `width`/`decimals`; `measure` dari subtype 11 (1 nominal, 2 ordinal, 3 scale; tidak ada → scale untuk numerik, nominal untuk string).
- Tulis: semua record di atas yang relevan, kompresi bytecode, nama pendek 8 karakter unik dibuat otomatis dengan nama panjang di subtype 13.

- [ ] **Step 1:** Tambah `pyreadstat` ke `requirements.txt`, pasang ke venv. `gen_refs.py` membuat `fixtures/sav/`: `basic.sav` (numerik + string, label variabel, value labels, missing diskret dan rentang, measure), `cp1252.sav` (label berisi "é", "ü", encoding Windows-1252), `long_names.sav` (nama 40 karakter), `long_string.sav` (string 300 karakter), `uncompressed.sav`, plus `fixtures/ref/sav_basic.json` berisi isi yang diharapkan (nilai, label, missing, measure).
- [ ] **Step 2: Tes gagal:** `read_sav` tiap fixture → isi sama dengan JSON (nilai bit-identik, NaN di posisi yang sama, label & value labels identik, "é" utuh); round-trip `read_sav(write_sav(read_sav(basic)))` identik; `.sav` yang ditulis Statik dibaca `pyreadstat` (tes Rust memanggil `tools/reference/.venv/Scripts/python -I tools/reference/check_sav.py <path>` yang mencetak JSON, dilewati dengan pesan jelas bila venv tidak ada); berkas terpotong di tengah → `Err(Format)` tanpa panic; berkas bukan `.sav` → `Err(Format)`.
- [ ] **Step 3:** `cargo test -p sav-io` → FAIL → implementasi → PASS.
- [ ] **Step 4:** Commit `feat(sav-io): read and write .sav files`.

### Task 2: Operasi struktur dataset dan VarId aman

**Files:** Modify `crates/dataset/src/dataset.rs`, `crates/procedures/src/cells.rs` dan setiap `run_*`; Test `crates/dataset/tests/ops.rs`, `crates/procedures/tests/stale_ids.rs`

**Interfaces:**
- Produces:
  ```rust
  impl Dataset {
      pub fn try_var(&self, id: VarId) -> Option<&Variable>;
      pub fn insert_variable(&mut self, at: usize, v: Variable, col: Column) -> Result<VarId, DatasetError>;
      pub fn delete_variable(&mut self, id: VarId) -> Result<(), DatasetError>;
      pub fn rename_variable(&mut self, id: VarId, new_name: &str) -> Result<(), DatasetError>;
      pub fn insert_case(&mut self, at: usize);            // semua kolom: NaN / ""
      pub fn delete_case(&mut self, row: usize) -> Result<(), DatasetError>;
      pub fn set_numeric(&mut self, id: VarId, row: usize, v: f64) -> Result<(), DatasetError>; // −0.0 → 0.0
      pub fn set_text(&mut self, id: VarId, row: usize, s: &str) -> Result<(), DatasetError>;
      pub fn set_type(&mut self, id: VarId, t: VarType) -> Result<(), DatasetError>; // konversi nilai: teks→angka gagal = NaN
  }
  // procedures::cells
  pub(crate) fn check_ids(ds: &Dataset, ids: &[VarId], proc_name: &str) -> Result<(), Vec<OutputItem>>;
  ```
- `VarId` tetap indeks posisi (pilihan sengaja: sederhana untuk grid). GUI bertanggung jawab membangun ulang opsi dialog setelah perubahan struktur; mesin menjaga agar id yang usang/di luar jangkauan atau yang kini menunjuk variabel teks untuk prosedur numerik menghasilkan `Warning` "Variable no longer exists…" / "… is not numeric", bukan panic.

- [ ] **Step 1: Tes gagal:** insert/delete/rename/insert_case/delete_case mengubah bentuk dataset dengan benar dan mempertahankan nilai; rename ke nama duplikat → `Err`; `set_numeric(-0.0)` menyimpan `0.0`; untuk setiap `run_*` di procedures: opsi dengan `VarId(999)` → tepat satu `OutputItem::Warning`, tanpa panic.
- [ ] **Step 2:** `cargo test -p dataset -p procedures` → FAIL → implementasi → PASS.
- [ ] **Step 3:** Commit `feat(dataset): structural edits; procedures reject stale ids`.

### Task 3: Impor CSV & XLSX

**Files:** Create `crates/sav-io/src/{csv_import.rs, xlsx_import.rs, names.rs}`; Modify `crates/statik-cli/src/main.rs` (pakai `import_csv`, terima juga `.sav`/`.xlsx`); Test `crates/sav-io/tests/import.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct CsvOptions { pub delimiter: Option<u8>, pub decimal_comma: Option<bool>, pub header: bool }
  pub fn import_csv(path: &Path, opt: &CsvOptions) -> Result<Dataset, SavError>;   // None = deteksi otomatis
  pub fn import_xlsx(path: &Path, sheet: Option<&str>, header: bool) -> Result<Dataset, SavError>;
  pub fn sanitize_name(raw: &str, taken: &dyn Fn(&str) -> bool) -> String;
  pub fn open_any(path: &Path) -> Result<Dataset, SavError>; // berdasarkan ekstensi: .sav/.csv/.txt/.xlsx
  ```
- Deteksi otomatis: delimiter = karakter terbanyak dan konsisten antar 20 baris pertama di antara `,` `;` `\t`; jika delimiter `;` dan sel numerik memakai `,` sebagai pemisah desimal → `decimal_comma = true`. BOM UTF-8 dibuang. Baris dengan jumlah sel berbeda → `Err(Format("baris N: …"))`. Sel teks tidak di-trim kecuali spasi di ujung baris.
- `sanitize_name`: spasi/karakter ilegal → `_`, diawali non-huruf → prefiks `V`, dipotong 64, kosong → `VAR00001` …, duplikat → akhiran `_1`, `_2` (aturan import SPSS yang setara).
- Desimal impor numerik: `min(desimal terbanyak yang terlihat, 4)`; lebar 8 (atau lebih jika angkanya panjang).
- XLSX: sheet pertama default; sel tanggal → numerik (nilai seri Excel) dengan catatan di `KNOWN_GAPS.md`; kolom campuran angka & teks → string.

- [ ] **Step 1:** Buat `fixtures/data/indo_semicolon.csv` (BOM, `;`, koma desimal, header "Jenis Kelamin;Nilai Ujian;Umur"), `messy_headers.csv` (header kosong, duplikat, diawali angka, 70 karakter), `sample.xlsx` (dibuat `gen_refs.py` dengan pandas/openpyxl: 2 sheet, angka, teks, sel kosong).
- [ ] **Step 2: Tes gagal:** `indo_semicolon` → 3 variabel numerik `Jenis_Kelamin`, `Nilai_Ujian`, `Umur`, nilai "78,5" terbaca 78.5; `messy_headers` → nama valid dan unik sesuai aturan; baris tak sejajar → `Err` dengan nomor baris; `sample.xlsx` sheet 2 terbaca dengan sel kosong = NaN; CSV dengan `3.66666666666667` → desimal 4; `open_any` memilih importir dari ekstensi; CLI `regression fixtures/sav/basic.sav …` berjalan.
- [ ] **Step 3:** `cargo test -p sav-io -p statik-cli` → FAIL → implementasi → PASS.
- [ ] **Step 4:** Commit `feat(sav-io): csv (incl. Indonesian locale) and xlsx import`.

### Task 4: Compute Variable

**Files:** Create `crates/transform/src/{lexer.rs, parser.rs, eval.rs, functions.rs, compute.rs}`; Test `crates/transform/tests/compute.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum Expr { Num(f64), Str(String), Var(String), Unary(Op, Box<Expr>), Binary(Op, Box<Expr>, Box<Expr>), Call(String, Vec<Expr>) }
  pub fn parse(src: &str) -> Result<Expr, ParseError>;   // ParseError { pos: usize, message: String }
  pub struct ComputeSpec { pub target: String, pub expression: String, pub condition: Option<String>, pub label: Option<String> }
  pub fn compute(ds: &mut Dataset, spec: &ComputeSpec) -> Result<ComputeReport, TransformError>;
  pub struct ComputeReport { pub target: VarId, pub created: bool, pub n_sysmis: usize }
  ```
- Sintaks (SPSS): angka (titik desimal), string `'…'`/`"…"`, nama variabel (case-insensitive), `+ - * / **`, unary `-`, `= <> ~= < > <= >=` dan `EQ NE LT GT LE GE`, `AND OR NOT` dan `& | ~`, kurung. Presedensi SPSS: `**` > unary − > `* /` > `+ −` > perbandingan > NOT > AND > OR.
- Fungsi: ABS, SQRT, LN, LG10, EXP, RND (opsional pembulatan ke kelipatan), TRUNC, MOD, MEAN, SUM, SD, VARIANCE, MIN, MAX, LAG(x) (nilai kasus sebelumnya; kasus pertama sysmis), MISSING(x), SYSMIS(x), NVALID. `MEAN(x,y)` dll memakai nilai valid saja (sysmis jika tidak ada); `MEAN.n(...)` dengan minimum n valid opsional.
- Missing: operasi aritmetika/fungsi skalar dengan argumen missing (sysmis atau user-missing) → sysmis; domain error/non-finite → sysmis; logika tiga-nilai SPSS (`missing AND 0 = 0`, `missing OR 1 = 1`, selain itu missing); hasil logika 1/0.
- Target: variabel baru dibuat (numerik, width 8, decimals 2) atau yang sudah ada ditimpa (hanya baris yang memenuhi `condition` bila ada; baris lain tetap untuk variabel lama, sysmis untuk variabel baru). Target string tidak didukung di Tahap 1 → `Err`. −0.0 → 0.0.

- [ ] **Step 1: Tes gagal:** parse/presedensi (`2+3*4**2 = 50`, `-2**2 = -4` karena `**` lebih kuat dari unary minus di SPSS); `ABS_RES = ABS(RES_1)` pada dataset dengan NaN → NaN di baris yang sama; `x + y` dengan y missing → NaN; `MEAN(x,y)` → nilai x; `y/0` → NaN; `LN(0)` → NaN; `x > 3 AND y` dengan y missing dan x ≤ 3 → 0; kondisi `IF grp = 1` hanya mengisi baris grp 1; nama target tidak valid → `Err`; ekspresi salah sintaks → `ParseError` dengan posisi; `LAG(x)` baris 1 → NaN.
- [ ] **Step 2:** `cargo test -p transform --test compute` → FAIL → implementasi → PASS.
- [ ] **Step 3:** Commit `feat(transform): compute variable expressions`.

### Task 5: Recode into Different Variables

**Files:** Create `crates/transform/src/recode.rs`; Test `crates/transform/tests/recode.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum OldValue { Value(f64), Range(f64, f64), Lowest(f64), Highest(f64), SysMis, Missing, Else }
  pub enum NewValue { Value(f64), SysMis, Copy }
  pub struct RecodeSpec { pub pairs: Vec<(VarId, String /* output name */, Option<String> /* label */)>, pub rules: Vec<(OldValue, NewValue)> }
  pub fn recode(ds: &mut Dataset, spec: &RecodeSpec) -> Result<Vec<VarId>, TransformError>;
  ```
- Aturan pertama yang cocok menang; tanpa aturan yang cocok → sysmis (SPSS "Into Different Variables"); `Missing` mencocokkan sysmis dan user-missing; `Range(lo, hi)` inklusif; variabel output numerik baru (width 8, decimals 0 jika semua nilai baru bulat, else 2); nama output yang sudah ada → `Err`. Hanya input numerik di Tahap 1 (string → `Err`).

- [ ] **Step 1: Tes gagal:** rentang tumpang tindih `1 thru 5 → 1`, `3 thru 10 → 2` → nilai 4 jadi 1; `Lowest(2)`, `Highest(8)`; `Else → Copy`; `SysMis → 99`; user-missing dengan `Missing → 9`; tanpa kecocokan → NaN; output dengan nama yang sudah ada → `Err`.
- [ ] **Step 2:** `cargo test -p transform --test recode` → FAIL → implementasi → PASS.
- [ ] **Step 3:** `cargo test --workspace` dan `cargo clippy --workspace -- -D warnings` → bersih.
- [ ] **Step 4:** Commit `feat(transform): recode into different variables`.
