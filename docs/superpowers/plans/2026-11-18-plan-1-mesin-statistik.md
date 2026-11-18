# Statik Plan 1 — Mesin Statistik Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Membangun inti statistik Statik tanpa UI — model dataset, rumus, prosedur Tahap 1, dan model output bergaya SPSS — yang bisa dijalankan lewat CLI kecil dan diverifikasi otomatis terhadap angka acuan.

**Architecture:** Workspace Cargo `D:\AI-Test\statik` dengan crate `dataset`, `stat-core`, `output-model`, `procedures`, dan biner `statik-cli`. Prosedur menerima `&Dataset` + struct opsi dan mengembalikan `Vec<OutputItem>`; semua aturan format angka SPSS hanya ada di `output-model`. Angka acuan tes disimpan sebagai fixture JSON yang dibuat sekali oleh script Python (scipy/statsmodels) dan nilai bersertifikat NIST.

**Tech Stack:** Rust stable 1.94 (MSVC), `nalgebra` (QR, eigen), `statrs` (distribusi t/F/normal/chi²), `csv`, `serde`/`serde_json`, `thiserror`. Alat acuan: Python 3.12 venv `tools/reference/.venv` dengan `numpy`, `scipy`, `statsmodels`, `pandas` (dev saja, tidak dikirim).

**Spec:** `docs/superpowers/specs/2026-11-18-statik-design.md`

Rencana lanjutan (ditulis setelah Plan 1 selesai): **Plan 2** — I/O data (`.sav` baca/tulis, CSV/XLSX, Compute Variable, Recode); **Plan 3** — aplikasi egui (Data Editor, menu, dialog, Output Viewer, clipboard ke Word, Autosave).

## Global Constraints

- Acuan angka & perilaku: SPSS Statistics versi terbaru (30+); 1-Sample K-S memakai koreksi Lilliefors.
- Nilai yang tampil harus sama dengan SPSS pada presisi tampilan SPSS.
- System-missing direpresentasikan `f64::NAN`; user-missing ditentukan oleh `Variable::missing`.
- Tidak ada nama "SPSS", logo, atau aset IBM di kode/output (label tabel memakai istilah statistik umum yang sama, mis. "Model Summary", "Coefficients").
- Nama variabel: diawali huruf, tanpa spasi, ≤ 64 karakter, unik case-insensitive.
- Nama variabel simpanan regresi: `PRE_n`, `RES_n`, `ZPR_n`, `ZRE_n`, `SRE_n` dengan `n` bilangan terkecil yang belum dipakai.
- Windows 10/11 x64; tidak ada dependensi C/C++ yang butuh compiler selain MSVC bawaan Rust.
- Toleransi tes: relatif `1e-9` terhadap fixture statsmodels/scipy kecuali disebut lain; `1e-7` relatif terhadap nilai bersertifikat NIST.

## Review Focus

- **Variabel konstan / varians nol** (mis. semua nilai sama di salah satu prediktor atau grup t-test): harus menghasilkan tabel dengan catatan peringatan seperti SPSS ("t cannot be computed because the standard deviation is 0"), bukan panic atau NaN di sel. → tes di Task 5 dan Task 7.
- **Multikolinearitas sempurna** (X2 = 2·X1): SPSS mengeluarkan prediktor dari model dengan tabel "Excluded Variables"; mesin tidak boleh menghasilkan koefisien raksasa. → tes di Task 5.
- **Data missing di tengah** (NaN dan user-missing): semua prosedur memakai listwise deletion default; N di tabel harus mencerminkan baris yang dipakai. → tes di Task 2 dan Task 5.
- **Sampel kecil di batas algoritma** (Shapiro-Wilk n=3 dan n>5000, K-S n<5, regresi dengan n ≤ jumlah prediktor+1): output berupa catatan, bukan crash. → tes di Task 6 dan Task 5.
- **Grup t-test independen yang nilainya tidak cocok dengan "Define Groups"** atau salah satu grup kosong: catatan peringatan, bukan crash. → tes di Task 7.

---

## File Structure

```
statik/
  Cargo.toml                         # workspace
  .gitignore
  KNOWN_GAPS.md                      # perilaku SPSS yang belum bisa dipastikan
  crates/
    dataset/src/{lib.rs, variable.rs, dataset.rs, csv_load.rs}
    stat-core/src/{lib.rs, dist.rs, describe.rs, ols.rs, normality.rs, ttest.rs, anova.rs, runs.rs}
    output-model/src/{lib.rs, table.rs, chart.rs, format.rs, render_text.rs, render_html.rs}
    procedures/src/{lib.rs, descriptives.rs, frequencies.rs, explore.rs, regression.rs,
                    ttest_one.rs, ttest_ind.rs, ttest_paired.rs, oneway.rs, npar_ks.rs, npar_runs.rs}
    statik-cli/src/main.rs
  fixtures/
    data/{longley.csv, sleep.csv, skripsi60.csv, edge_cases.csv}
    ref/*.json                       # angka acuan dari gen_refs.py
  tools/reference/{requirements.txt, gen_refs.py}
```

Tes setiap crate ada di `crates/<crate>/tests/*.rs` dan membaca `fixtures/` lewat `env!("CARGO_MANIFEST_DIR")/../../fixtures`.

---

### Task 1: Workspace, git, dan alat angka acuan

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `KNOWN_GAPS.md`, `crates/*/Cargo.toml`, `crates/*/src/lib.rs` (kosong)
- Create: `tools/reference/requirements.txt`, `tools/reference/gen_refs.py`
- Create: `fixtures/data/longley.csv`, `fixtures/data/sleep.csv`, `fixtures/data/skripsi60.csv`, `fixtures/data/edge_cases.csv`, `fixtures/ref/*.json`

**Interfaces:**
- Produces: workspace yang `cargo test --workspace` hijau; fixture JSON dengan skema `{"case": str, "values": {<nama>: number | [number] }}` per file.

- [ ] **Step 1:** `git init` di `D:\AI-Test\statik`, branch `main`, commit spec + plan yang sudah ada; lalu `git switch -c plan-1-engine`.
- [ ] **Step 2:** Buat workspace dengan lima member di atas, `edition = "2024"`, profil release `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`. `.gitignore`: `target/`, `tools/reference/.venv/`.
- [ ] **Step 3:** Isi `fixtures/data/`:
  - `longley.csv` — dataset NIST StRD Longley (kolom `y,x1..x6`, 16 baris, salin dari itl.nist.gov/div898/strd/lls/data/Longley.shtml).
  - `sleep.csv` — kolom `extra,group,id`: grup 1 = `0.7,-1.6,-0.2,-1.2,-0.1,3.4,3.7,0.8,0.0,2.0`, grup 2 = `1.9,0.8,1.1,0.1,-0.1,4.4,5.5,1.6,4.6,3.4`, id 1–10 berpasangan.
  - `skripsi60.csv` — dibuat `gen_refs.py` dengan `numpy.random.default_rng(20261118)`: 60 baris, `x1~N(50,10)`, `x2~N(30,5)`, `x3~U(1,5)` dibulatkan ke 2 desimal, `y = 5 + 0.6x1 + 0.9x2 + 2x3 + N(0,4)` dibulatkan 2 desimal, `grp` = 1 untuk 30 baris pertama, 2 sisanya.
  - `edge_cases.csv` — kolom `y` (= 3 + 2·x1 + deret tetap `[0.5,-0.3,0.2,-0.6,0.1,0.4,-0.2,0.3,-0.5,0.0,0.6,-0.4]`), `const` (semua 7), `x1` (= 1..12), `x1dbl` (= 2·x1), `withmiss` (NaN di baris 3 dan 8, nilai 99 di baris 5 untuk dijadikan user-missing), 12 baris.
- [ ] **Step 4:** `gen_refs.py` menulis satu JSON per kasus ke `fixtures/ref/`. Isinya dijelaskan di Task yang memakainya (Task 3–8). Setup: `python -m venv tools/reference/.venv` lalu `tools/reference/.venv/Scripts/pip install -r tools/reference/requirements.txt` (`numpy`, `scipy`, `statsmodels`, `pandas` dengan versi dipin).
- [ ] **Step 5:** Run `cargo test --workspace` → PASS (0 tests). Commit `chore: workspace and reference fixtures`.

### Task 2: Crate `dataset`

**Files:** Create `crates/dataset/src/{variable.rs, dataset.rs, csv_load.rs}`; Test `crates/dataset/tests/dataset.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum Measure { Scale, Ordinal, Nominal }
  pub enum VarType { Numeric, String { width: usize } }
  pub enum MissingSpec { None, Discrete(Vec<f64>), Range { lo: f64, hi: f64, discrete: Option<f64> } }
  pub struct Variable { pub name: String, pub label: Option<String>, pub var_type: VarType,
      pub width: usize, pub decimals: usize, pub value_labels: BTreeMap<OrderedValue, String>,
      pub missing: MissingSpec, pub measure: Measure }
  pub enum Column { Numeric(Vec<f64>), Text(Vec<String>) }
  pub struct VarId(pub usize);
  pub struct Dataset { /* vars: Vec<Variable>, cols: Vec<Column> */ }
  impl Dataset {
      pub fn new() -> Self;
      pub fn n_cases(&self) -> usize;
      pub fn add_variable(&mut self, v: Variable, col: Column) -> Result<VarId, DatasetError>;
      pub fn var(&self, id: VarId) -> &Variable;
      pub fn find(&self, name: &str) -> Option<VarId>;   // case-insensitive
      pub fn numeric(&self, id: VarId) -> &[f64];
      pub fn is_missing(&self, id: VarId, row: usize) -> bool; // NaN atau user-missing
      pub fn listwise_rows(&self, ids: &[VarId]) -> Vec<usize>;
      pub fn unique_name(&self, prefix: &str) -> String;  // "RES_" -> "RES_1", "RES_2", ...
  }
  pub fn validate_name(name: &str) -> Result<(), DatasetError>;
  pub fn load_csv(path: &Path) -> Result<Dataset, DatasetError>; // baris 1 = nama; kolom numerik jika semua sel parse f64 atau kosong
  ```
  `OrderedValue` adalah newtype `f64` yang mengimplementasikan `Ord` via `total_cmp`.

- [ ] **Step 1: Tes gagal** di `tests/dataset.rs`:
  - `validate_name("x1")` ok; `"1x"`, `"x y"`, nama 65 karakter → `Err`.
  - `add_variable` dengan nama `"X1"` setelah `"x1"` → `Err(DatasetError::DuplicateName)`.
  - Muat `edge_cases.csv`, set `withmiss.missing = Discrete(vec![99.0])` → `listwise_rows(&[withmiss])` panjang 9 (baris indeks 2, 4, 7 terbuang).
  - `unique_name("RES_")` pada dataset yang sudah punya `RES_1` → `"RES_2"`.
- [ ] **Step 2:** `cargo test -p dataset` → FAIL (simbol belum ada).
- [ ] **Step 3:** Implementasi sesuai Interfaces.
- [ ] **Step 4:** `cargo test -p dataset` → PASS.
- [ ] **Step 5:** Commit `feat(dataset): variables, missing values, csv load`.

### Task 3: `stat-core` — distribusi dan statistik deskriptif

**Files:** Create `crates/stat-core/src/{dist.rs, describe.rs}`; Test `crates/stat-core/tests/describe.rs`

**Interfaces:**
- Produces:
  ```rust
  // dist.rs
  pub fn t_sig_two_tailed(t: f64, df: f64) -> f64;
  pub fn t_quantile(p: f64, df: f64) -> f64;
  pub fn f_sig(f: f64, df1: f64, df2: f64) -> f64;      // P(F > f)
  pub fn normal_cdf(z: f64) -> f64;
  pub fn normal_quantile(p: f64) -> f64;
  // describe.rs
  pub struct Describe { pub n: usize, pub sum: f64, pub mean: f64, pub variance: f64, pub sd: f64,
      pub se_mean: f64, pub min: f64, pub max: f64, pub range: f64,
      pub skewness: Option<f64>, pub se_skewness: Option<f64>,
      pub kurtosis: Option<f64>, pub se_kurtosis: Option<f64> }
  pub fn describe(x: &[f64]) -> Option<Describe>;            // None jika n == 0
  pub fn percentile_haverage(sorted: &[f64], p: f64) -> f64; // p dalam 0..=100
  pub fn trimmed_mean_5(sorted: &[f64]) -> f64;
  ```
- Rumus (SPSS): variance pembagi n−1; skewness `n/((n−1)(n−2)) Σz³` (None jika n<3); kurtosis `n(n+1)/((n−1)(n−2)(n−3)) Σz⁴ − 3(n−1)²/((n−2)(n−3))` (None jika n<4); `SE_skew = sqrt(6n(n−1)/((n−2)(n+1)(n+3)))`; `SE_kurt = 2·SE_skew·sqrt((n²−1)/((n−3)(n+5)))`. Percentile HAVERAGE = posisi `(n+1)p/100` dengan interpolasi linear, dipotong ke min/max. Trimmed mean 5% = pemangkasan pecahan: buang 5% bobot dari tiap ujung, kasus di batas mendapat bobot pecahan.

- [ ] **Step 1: Tes gagal.** `gen_refs.py` menulis `ref/describe_skripsi60.json` untuk `x1`: mean, var (`ddof=1`), skew (`scipy.stats.skew(bias=False)`), kurt (`scipy.stats.kurtosis(bias=False)`), persentil 25/50/75 (`numpy.percentile(method="weibull")`), dan trimmed mean 5% dihitung dengan rumus pecahan di atas langsung di Python. Tes membandingkan setiap field dengan toleransi `1e-9`. Tambah tes distribusi dengan nilai bentuk tertutup: `t_sig_two_tailed(2.0, 2.0)` = `1 − 2/√6` = `0.18350341907227397` (±1e-12), `f_sig(3.0, 2.0, 10.0)` = `1.6⁻⁵` = `0.095367431640625` (±1e-12), `normal_quantile(0.975)` ≈ `1.959963984540054` (±1e-9). `describe(&[5.0])` → `skewness == None`.
- [ ] **Step 2:** `cargo test -p stat-core` → FAIL.
- [ ] **Step 3:** Implementasi; distribusi memakai `statrs::distribution::{StudentsT, FisherSnedecor, Normal}`.
- [ ] **Step 4:** `cargo test -p stat-core` → PASS.
- [ ] **Step 5:** Commit `feat(stat-core): distributions and descriptives`.

### Task 4: `output-model` — tabel, grafik, format angka SPSS

**Files:** Create `crates/output-model/src/{table.rs, chart.rs, format.rs, render_text.rs, render_html.rs}`; Test `crates/output-model/tests/format.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum CellFormat { Fixed(u8), Sig, Count, Text }
  pub enum CellValue { Num(f64), Text(String), Empty }
  pub struct Cell { pub value: CellValue, pub format: CellFormat, pub footnotes: Vec<char> }
  pub struct HeaderNode { pub label: String, pub children: Vec<HeaderNode> }
  pub struct Table { pub title: String, pub col_headers: Vec<HeaderNode>,
      pub row_headers: Vec<HeaderNode>, pub cells: Vec<Vec<Cell>>, pub footnotes: Vec<(char, String)> }
  pub enum Chart {
      Histogram { title: String, values: Vec<f64>, normal_curve: bool },
      PPPlot { title: String, observed_cum: Vec<f64>, expected_cum: Vec<f64> },
      QQPlot { title: String, observed: Vec<f64>, expected: Vec<f64> },
      Scatter { title: String, x_label: String, y_label: String, points: Vec<(f64, f64)> } }
  pub enum OutputItem { Title(String), Notes(String), Table(Table), Chart(Chart), Warning(String) }
  pub fn format_cell(c: &Cell) -> String;
  pub fn render_text(items: &[OutputItem]) -> String;
  pub fn render_html(items: &[OutputItem]) -> String;   // satu <table> per Table, footnote di <tfoot>
  ```
- Aturan `format_cell` (SPSS default, leading zero dibuang untuk |v| < 1):
  - `Fixed(n)`: dibulatkan ke `n` desimal; `0.912` → `.912`, `-0.358` → `-.358`, `12.5` (n=3) → `12.500`.
  - `Sig`: 3 desimal tanpa leading zero; nilai < 0.0005 → `.000`; tidak pernah menampilkan `1.000` dari pembulatan nilai < 0.9995 (pembulatan biasa).
  - `Count`: bilangan bulat tanpa desimal.
  - NaN di `Num` → `.` (system-missing gaya SPSS).
  - Footnote ditempel sebagai huruf: `.912` + `['a']` → `.912a`.

- [ ] **Step 1: Tes gagal** — kasus di atas persis sebagai assertion (`format_cell` untuk `.912`, `-.358`, `12.500`, `.000` dari `0.0001`, `.050` dari `0.0499`, `.` dari NaN, `.912a`); `render_html` dari tabel 2×2 berisi `<table`, judul, dan `<tfoot>` ketika ada footnote.
- [ ] **Step 2:** `cargo test -p output-model` → FAIL.
- [ ] **Step 3:** Implementasi. Desimal per kolom prosedur didefinisikan di `procedures` (Task 5–8), bukan di sini.
- [ ] **Step 4:** `cargo test -p output-model` → PASS.
- [ ] **Step 5:** Commit `feat(output-model): tables, charts, SPSS number format`.

### Task 5: Regresi linear berganda (metode Enter)

**Files:** Create `crates/stat-core/src/ols.rs`, `crates/procedures/src/regression.rs`; Test `crates/stat-core/tests/ols.rs`, `crates/procedures/tests/regression.rs`

**Interfaces:**
- Consumes: `Dataset`, `listwise_rows`, `unique_name` (Task 2); `dist` (Task 3); `Table`, `Chart`, `OutputItem` (Task 4).
- Produces:
  ```rust
  // stat-core::ols
  pub struct OlsFit { pub b: Vec<f64>, pub se: Vec<f64>, pub beta: Vec<f64>, pub t: Vec<f64>, pub sig: Vec<f64>,
      pub r: f64, pub r2: f64, pub adj_r2: f64, pub se_est: f64,
      pub ss_reg: f64, pub ss_res: f64, pub df_reg: usize, pub df_res: usize, pub f: f64, pub f_sig: f64,
      pub durbin_watson: f64, pub tolerance: Vec<f64>, pub vif: Vec<f64>,
      pub eigenvalues: Vec<f64>, pub condition_index: Vec<f64>, pub variance_proportions: Vec<Vec<f64>>,
      pub predicted: Vec<f64>, pub residuals: Vec<f64>, pub z_predicted: Vec<f64>,
      pub z_residuals: Vec<f64>, pub studentized_residuals: Vec<f64>,
      pub excluded: Vec<usize> }   // indeks prediktor yang dikeluarkan karena tolerance < 0.0001
  pub fn ols(y: &[f64], xs: &[&[f64]]) -> Result<OlsFit, OlsError>; // intercept selalu disertakan
  // procedures::regression
  pub struct RegressionOptions { pub dependent: VarId, pub independents: Vec<VarId>,
      pub collinearity: bool, pub durbin_watson: bool,
      pub plot_hist: bool, pub plot_pp: bool, pub plot_zpred_sresid: bool,
      pub save: SaveFlags /* predicted, residual, z_predicted, z_residual, studentized */ }
  pub fn run_regression(ds: &mut Dataset, opt: &RegressionOptions) -> Vec<OutputItem>;
  ```
- Algoritma: QR (`nalgebra`) dengan intercept. Beta = `b_j·sd(x_j)/sd(y)`. Tolerance_j = `1 − R²` dari regresi `x_j` pada prediktor lain; VIF = 1/Tolerance. Prediktor dengan tolerance < 0.0001 dikeluarkan (urutan masuk) dan dilaporkan di tabel "Excluded Variables". Collinearity Diagnostics: kolom matriks desain (termasuk konstanta) diskalakan ke panjang satuan, eigen dari `X'X` diurutkan menurun, `CI_k = sqrt(λ_max/λ_k)`, variance proportions dari vektor eigen (`v_jk²/λ_k` dinormalisasi per koefisien). DW = `Σ(e_t − e_{t−1})² / Σe_t²`. ZPRED = `(ŷ − mean ŷ)/sd ŷ`; ZRESID = `e/se_est`; SRESID = `e/(se_est·sqrt(1 − h_ii))`.
- Tabel output dan desimal: *Variables Entered/Removed*; *Model Summary* (R, R Square, Adjusted R Square: `Fixed(3)`; Std. Error of the Estimate: `Fixed(5)`; Durbin-Watson: `Fixed(3)`; footnote a = "Predictors: (Constant), …", b = "Dependent Variable: …"); *ANOVA* (Sum of Squares, Mean Square, F: `Fixed(3)`; df: `Count`; Sig: `Sig`); *Coefficients* (B, Std. Error, Beta, t: `Fixed(3)`; Sig: `Sig`; Tolerance, VIF: `Fixed(3)`); *Collinearity Diagnostics* (Eigenvalue, Condition Index: `Fixed(3)`; proporsi: `Fixed(2)`); *Residuals Statistics* (Minimum, Maximum, Mean, Std. Deviation untuk Predicted Value, Residual, Std. Predicted Value, Std. Residual: `Fixed(4)`; N: `Count`). Semua desimal ini dicatat di `KNOWN_GAPS.md` sebagai "verifikasi dengan screenshot SPSS pengguna".

- [ ] **Step 1: Tes gagal (stat-core)** di `tests/ols.rs`:
  - Longley (nilai bersertifikat NIST, toleransi relatif 1e-7): `b = [-3482258.63459582, 15.0618722713733, -0.358191792925910e-1, -2.02022980381683, -1.03322686717359, -0.511041056535807e-1, 1829.15146461355]`, `r2 = 0.995479004577296`, `se_est = 304.854073561965`.
  - `skripsi60` y ~ x1+x2+x3 vs `ref/ols_skripsi60.json` dari statsmodels `OLS(...).fit()`: `params`, `bse`, `tvalues`, `pvalues`, `rsquared`, `rsquared_adj`, `fvalue`, `f_pvalue`, `durbin_watson(resid)`, `variance_inflation_factor` per prediktor, `get_influence().resid_studentized_internal` (=SRESID).
  - `edge_cases` y ~ prediktor `[x1, x1dbl]` → `excluded == vec![1]`, `b.len() == 2`.
  - n=3 dengan 3 prediktor → `Err(OlsError::TooFewCases)`.
- [ ] **Step 2:** `cargo test -p stat-core --test ols` → FAIL.
- [ ] **Step 3:** Implementasi `ols`.
- [ ] **Step 4:** `cargo test -p stat-core --test ols` → PASS.
- [ ] **Step 5: Tes gagal (procedures)** di `tests/regression.rs`: `run_regression` pada `skripsi60` dengan semua opsi aktif menghasilkan judul tabel berurutan `["Variables Entered/Removed", "Model Summary", "ANOVA", "Coefficients", "Collinearity Diagnostics", "Residuals Statistics"]` diikuti 3 `Chart`; sel R Square sama dengan `format_cell` dari nilai fixture; setelah dijalankan dua kali, dataset punya `RES_1` dan `RES_2`; variabel `withmiss` (dengan 99 user-missing) sebagai prediktor → N di Residuals Statistics = 9; kasus `excluded` menghasilkan tabel "Excluded Variables"; n terlalu kecil → satu `OutputItem::Warning`, tanpa panic.
- [ ] **Step 6:** `cargo test -p procedures --test regression` → FAIL lalu implementasi `run_regression` → PASS.
- [ ] **Step 7:** Commit `feat: multiple linear regression with diagnostics`.

### Task 6: Uji normalitas — Explore, 1-Sample K-S, Shapiro-Wilk

**Files:** Create `crates/stat-core/src/normality.rs`, `crates/procedures/src/{explore.rs, npar_ks.rs}`; Test `crates/stat-core/tests/normality.rs`, `crates/procedures/tests/normality.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct KsResult { pub d: f64, pub d_pos: f64, pub d_neg: f64, pub sig: f64, pub sig_is_lower_bound: bool }
  pub fn lilliefors(x: &[f64]) -> Option<KsResult>;     // None jika n < 5
  pub struct SwResult { pub w: f64, pub sig: f64 }
  pub fn shapiro_wilk(x: &[f64]) -> Option<SwResult>;   // None jika n < 3 atau n > 5000
  pub struct ExploreOptions { pub dependents: Vec<VarId>, pub normality: bool, pub qq_plot: bool }
  pub fn run_explore(ds: &Dataset, opt: &ExploreOptions) -> Vec<OutputItem>;
  pub struct NparKsOptions { pub vars: Vec<VarId> }
  pub fn run_npar_ks(ds: &Dataset, opt: &NparKsOptions) -> Vec<OutputItem>;
  ```
- Algoritma: D Lilliefors memakai mean dan sd (n−1) sampel. Sig: aproksimasi Dallal-Wilkinson (1986); jika hasil > 0.2 maka `sig = 0.2`, `sig_is_lower_bound = true` dan sel diberi footnote "This is a lower bound of the true significance." Shapiro-Wilk: port algoritma Royston AS R94 (`swilk`). Tabel Explore: *Case Processing Summary*, *Descriptives* (Mean, 95% CI Lower/Upper, 5% Trimmed Mean, Median, Variance, Std. Deviation, Minimum, Maximum, Range, Interquartile Range [HAVERAGE], Skewness, Kurtosis + Std. Error), *Tests of Normality* (Statistic `Fixed(3)`, df `Count`, Sig `Sig`; footnote "Lilliefors Significance Correction"). Tabel 1-Sample K-S (versi terbaru): N, Normal Parameters (Mean, Std. Deviation), Most Extreme Differences (Absolute, Positive, Negative), Test Statistic, Asymp. Sig. (2-tailed) dengan footnote "Lilliefors Corrected".

- [ ] **Step 1: Tes gagal (stat-core).** `gen_refs.py` menulis `ref/normality.json` untuk `x1` dan residual OLS `skripsi60`, serta `sleep.extra`: `scipy.stats.shapiro` (W, p) dan `statsmodels.stats.diagnostic.lilliefors(x, dist="norm", pvalmethod="approx")` (D, p). Assertion: W dan p Shapiro-Wilk toleransi 1e-6; D toleransi 1e-9; untuk p Lilliefors fixture < 0.1 → `sig` toleransi 1e-4; untuk p fixture ≥ 0.2 → `sig == 0.2 && sig_is_lower_bound`. `shapiro_wilk(&[1.0, 2.0])` → None; `lilliefors` pada 4 nilai → None.
- [ ] **Step 2:** `cargo test -p stat-core --test normality` → FAIL → implementasi → PASS.
- [ ] **Step 3: Tes gagal (procedures):** `run_explore` pada `skripsi60.x1` menghasilkan tabel "Tests of Normality" dengan sel Sig. K-S berformat `.200*` bila lower bound; `run_npar_ks` pada kolom `const` menghasilkan `OutputItem::Warning` (sd = 0), tanpa panic.
- [ ] **Step 4:** `cargo test -p procedures --test normality` → FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat: normality tests (Lilliefors K-S, Shapiro-Wilk) and Explore`.
- [ ] **Step 6:** Tambah ke `KNOWN_GAPS.md`: "SPSS terbaru mungkin juga menampilkan baris Monte Carlo Sig. di 1-Sample K-S; angka Monte Carlo tidak bisa identik karena generator acak SPSS tidak publik — konfirmasi dari screenshot pengguna."

### Task 7: Uji t (One-Sample, Independent, Paired)

**Files:** Create `crates/stat-core/src/ttest.rs`, `crates/procedures/src/{ttest_one.rs, ttest_ind.rs, ttest_paired.rs}`; Test `crates/stat-core/tests/ttest.rs`, `crates/procedures/tests/ttest.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct TResult { pub t: f64, pub df: f64, pub sig: f64, pub mean_diff: f64, pub se_diff: f64, pub ci: (f64, f64) }
  pub fn one_sample_t(x: &[f64], test_value: f64, conf: f64) -> Option<TResult>;
  pub struct IndResult { pub levene_f: f64, pub levene_sig: f64, pub equal: TResult, pub welch: TResult }
  pub fn independent_t(a: &[f64], b: &[f64], conf: f64) -> Option<IndResult>;
  pub struct PairedResult { pub corr: f64, pub corr_sig: f64, pub test: TResult, pub sd_diff: f64 }
  pub fn paired_t(a: &[f64], b: &[f64], conf: f64) -> Option<PairedResult>;
  pub fn levene_mean(groups: &[&[f64]]) -> (f64, f64, usize, usize); // F, sig, df1, df2
  // procedures
  pub struct OneSampleOptions { pub vars: Vec<VarId>, pub test_value: f64, pub conf: f64 }
  pub struct IndependentOptions { pub test_vars: Vec<VarId>, pub grouping: VarId, pub group1: f64, pub group2: f64, pub conf: f64 }
  pub struct PairedOptions { pub pairs: Vec<(VarId, VarId)>, pub conf: f64 }
  pub fn run_one_sample(ds: &Dataset, o: &OneSampleOptions) -> Vec<OutputItem>;
  pub fn run_independent(ds: &Dataset, o: &IndependentOptions) -> Vec<OutputItem>;
  pub fn run_paired(ds: &Dataset, o: &PairedOptions) -> Vec<OutputItem>;
  ```
- Levene = berbasis mean (deviasi absolut dari mean grup, lalu one-way ANOVA). Welch df = Satterthwaite. Mengembalikan `None` jika salah satu grup n<2 atau sd gabungan 0; prosedur mengubahnya jadi `Warning`.

- [ ] **Step 1: Tes gagal (stat-core)** dengan `sleep.csv` (nilai R yang dikenal luas, toleransi 1e-4 pada angka tercetak):
  - Paired grup1 vs grup2: `t = -4.0621`, `df = 9`, `sig = 0.002833`, `mean_diff = -1.58`, `ci = (-2.4598858, -0.7001142)`.
  - Independent: equal → `t = -1.8608`, `df = 18`, `sig = 0.07919`; welch → `t = -1.8608`, `df = 17.776`, `sig = 0.07939`.
  - Levene dan one-sample (`skripsi60.x1`, test value 50) vs `ref/ttest.json` dari `scipy.stats.levene(center="mean")` dan `scipy.stats.ttest_1samp`, toleransi 1e-9.
  - `independent_t(&[1.0], &[2.0, 3.0], 0.95)` → None.
- [ ] **Step 2:** `cargo test -p stat-core --test ttest` → FAIL → implementasi → PASS.
- [ ] **Step 3: Tes gagal (procedures):** `run_independent` menghasilkan "Group Statistics" lalu "Independent Samples Test" dengan dua baris ("Equal variances assumed", "Equal variances not assumed"); grouping dengan `group2 = 9` (tidak ada di data) → `Warning`; `run_one_sample` pada `const` → `Warning` "t cannot be computed because the standard deviation is 0."
- [ ] **Step 4:** `cargo test -p procedures --test ttest` → FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat: one-sample, independent and paired t-tests`.

### Task 8: One-Way ANOVA, Runs Test, Descriptives, Frequencies

**Files:** Create `crates/stat-core/src/{anova.rs, runs.rs}`, `crates/procedures/src/{oneway.rs, npar_runs.rs, descriptives.rs, frequencies.rs}`; Test `crates/stat-core/tests/anova_runs.rs`, `crates/procedures/tests/basic.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct OneWay { pub ss_between: f64, pub ss_within: f64, pub df_between: usize, pub df_within: usize, pub f: f64, pub sig: f64 }
  pub fn oneway(groups: &[&[f64]]) -> Option<OneWay>;
  pub struct RunsResult { pub test_value: f64, pub n_below: usize, pub n_at_or_above: usize, pub runs: usize, pub z: f64, pub sig: f64 }
  pub fn runs_test_median(x: &[f64]) -> Option<RunsResult>;
  pub fn run_oneway(ds: &Dataset, o: &OneWayOptions) -> Vec<OutputItem>;      // OneWayOptions { dependent, factor, descriptives: bool, homogeneity: bool }
  pub fn run_runs(ds: &Dataset, o: &RunsOptions) -> Vec<OutputItem>;          // RunsOptions { vars }
  pub fn run_descriptives(ds: &mut Dataset, o: &DescriptivesOptions) -> Vec<OutputItem>; // { vars, skew_kurt: bool, save_z: bool } -> variabel Z<nama>
  pub fn run_frequencies(ds: &Dataset, o: &FrequenciesOptions) -> Vec<OutputItem>;      // { vars, histogram: bool }
  ```
- Runs: test value = median (HAVERAGE); kasus < median vs ≥ median; `μ = 2n₁n₂/n + 1`, `σ² = 2n₁n₂(2n₁n₂ − n)/(n²(n−1))`; jika n < 50 terapkan koreksi kontinuitas 0.5 ke arah μ. Homogeneity table: baris "Based on Mean" wajib; baris Median / Median adjusted df / Trimmed mean dicatat di `KNOWN_GAPS.md` (rumus dari *IBM SPSS Statistics Algorithms*, ONEWAY) untuk diimplementasikan saat dokumennya diverifikasi. Frequencies: kolom Frequency, Percent, Valid Percent, Cumulative Percent (`Fixed(1)`), baris Missing System bila ada; nilai ditampilkan dengan value label bila ada.

- [ ] **Step 1: Tes gagal** — `oneway` pada `skripsi60.y` per `grp` vs `scipy.stats.f_oneway` (`ref/anova.json`); runs pada residual OLS `skripsi60` vs `ref/runs.json` yang dihitung `gen_refs.py` dengan rumus di atas; `run_descriptives` dengan `save_z` membuat variabel `Zx1`; `run_frequencies` pada `withmiss` menampilkan baris "Missing" dengan Frequency 3.
- [ ] **Step 2:** `cargo test -p stat-core -p procedures` → FAIL → implementasi → PASS.
- [ ] **Step 3:** Commit `feat: one-way ANOVA, runs test, descriptives, frequencies`.

### Task 9: `statik-cli` — jalankan prosedur dari CSV

**Files:** Create `crates/statik-cli/src/main.rs`; Test `crates/statik-cli/tests/cli.rs`

**Interfaces:**
- Consumes: semua `run_*` (Task 5–8), `load_csv` (Task 2), `render_text`/`render_html` (Task 4).
- Produces: biner `statik-cli` dengan subcommand:
  `regression <csv> --dep y --indep x1,x2,x3 [--html out.html]`,
  `ttest-ind <csv> --var v --group g --groups 1,2`, `ttest-paired <csv> --pair a,b`,
  `ttest-one <csv> --var v --value 50`, `explore <csv> --var v`, `ks <csv> --var v`,
  `runs <csv> --var v`, `oneway <csv> --dep y --factor g`. Argumen di-parse manual (tanpa `clap`, demi ukuran biner).

- [ ] **Step 1: Tes gagal:** jalankan biner (`env!("CARGO_BIN_EXE_statik-cli")`) `regression fixtures/data/skripsi60.csv --dep y --indep x1,x2,x3` → exit 0, stdout memuat `Model Summary`, `ANOVA`, `Coefficients`; dengan `--html` menulis file yang memuat `<table`. Variabel tak dikenal → exit code 2 dan pesan `unknown variable`.
- [ ] **Step 2:** `cargo test -p statik-cli` → FAIL → implementasi → PASS.
- [ ] **Step 3:** `cargo build --release -p statik-cli` dan catat ukuran `target/release/statik-cli.exe` (harapan < 5 MB).
- [ ] **Step 4:** `cargo test --workspace` → PASS, `cargo clippy --workspace -- -D warnings` → bersih.
- [ ] **Step 5:** Commit `feat(cli): run phase-1 procedures from CSV`.
