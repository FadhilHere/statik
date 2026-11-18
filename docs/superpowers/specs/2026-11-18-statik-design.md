# Statik: Aplikasi Statistik Desktop Ringan — Desain

Tanggal: 2026-11-18
Status: draf, menunggu persetujuan

## 1. Maksud dan alasan

Aplikasi statistik desktop untuk Windows yang dipakai mahasiswa satu angkatan
untuk tugas kuliah. Alur kerja dan tata letak meniru IBM SPSS Statistics 1:1,
supaya pengguna yang belajar dari modul/tutorial SPSS bisa langsung memakainya
tanpa belajar ulang. Alasan dibuat: ringan (satu `.exe` kecil, tanpa lisensi),
bisa dibagikan bebas ke teman-teman.

"Statik" adalah nama kerja (keputusan Claude, bisa diganti).

## 2. Tujuan

- **G1 — Alur kerja SPSS 1:1.** Data Editor (Data View / Variable View), bar
  menu dengan struktur yang sama (File, Edit, View, Data, Transform, Analyze,
  Graphs, Utilities, Window, Help), dialog prosedur dengan daftar variabel kiri,
  tombol panah, kotak target, tombol sub-dialog (Statistics…, Plots…, Save…,
  Options…) dan OK / Paste / Reset / Cancel / Help, serta jendela Output Viewer
  dengan outline di kiri dan tabel di kanan.
- **G2 — Angka sama dengan SPSS.** Setiap nilai yang tampil di tabel output
  sama dengan SPSS pada presisi yang ditampilkan, termasuk format SPSS
  (mis. `.000` untuk Sig. kecil, tanpa nol di depan untuk nilai < 1 pada
  kolom tertentu, dsb.).
- **G3 — Fitur Tahap 1** (lihat §6): uji asumsi klasik, regresi linear
  berganda, uji t, uji F, plus fitur pendukung yang dibutuhkan alur tersebut.
- **G4 — Ringan.** Satu file `.exe` portabel, target ≤ 15 MB, tanpa installer,
  terbuka < 1 detik, RAM idle < 80 MB.
- **G5 — Mudah diperluas.** Menambah prosedur baru tidak perlu mengubah inti
  aplikasi.

## 3. Non-tujuan (untuk sekarang)

- macOS / Linux.
- Bahasa syntax SPSS (tombol **Paste** ada di dialog tapi untuk Tahap 1 hanya
  menampilkan syntax yang setara sebagai teks, tidak mengeksekusi file syntax).
- Chart Builder, pivot table yang bisa diedit interaktif, ekspor ke .spv.
- Prosedur di luar §6 (masuk peta jalan §10).

## 4. Anti-tujuan (gagal walau "jalan")

- Angka berbeda dengan SPSS → dosen menolak hasilnya.
- Kehilangan data pengguna (crash tanpa bisa simpan, file `.sav` rusak).
- Memakai nama "SPSS", logo, ikon, atau aset grafis IBM. Tata letak dan alur
  disamakan; gaya visual (warna, ikon toolbar, font) milik sendiri.
- Aplikasi berat (ratusan MB / lambat dibuka).

## 5. Batasan

- Windows 10/11 x64 saja.
- Rust (stable) + egui/eframe. Distribusi: satu `.exe` statis.
- Tidak ada SPSS yang terinstall di mesin pengembang → angka acuan diambil dari
  R (paket standar) dan contoh output SPSS yang dipublikasikan (buku teks,
  modul kuliah). Lihat §8.
- Dataset tugas kuliah biasanya kecil (≤ 10.000 baris, ≤ 100 variabel), tapi
  grid harus tetap lancar sampai ~100.000 baris.

## 6. Cakupan Tahap 1

### 6.1 Data Editor
- **Data View**: grid sel yang bisa diedit, nomor baris, header nama variabel,
  scroll virtual, insert/delete case & variable, copy-paste dari/ke Excel
  (teks tab-separated), sel kosong = system-missing (`.`), toggle **Value
  Labels** (tampilkan label alih-alih kode).
- **Variable View**: kolom Name, Type (Numeric, String, Date dasar), Width,
  Decimals, Label, Values (dialog value labels), Missing (dialog discrete
  missing values / range), Columns, Align, Measure (Scale/Ordinal/Nominal),
  Role.
- Aturan nama variabel mengikuti SPSS (diawali huruf, tanpa spasi, ≤ 64
  karakter, unik tanpa memperhatikan huruf besar/kecil).

### 6.2 File
- Buka & simpan `.sav` (termasuk kompresi bytecode, label variabel, value
  labels, missing values, measure level, string panjang, encoding UTF-8 dan
  Windows-1252).
- Impor `.csv` dan `.xlsx` (sheet pertama/pilihan, baris pertama sebagai nama
  variabel).
- Simpan/buka berkas output milik sendiri (format sendiri, tidak meniru .spv).

### 6.3 Transform
- **Compute Variable** dengan ekspresi: operator aritmatika/perbandingan/logika
  dan fungsi minimal ABS, SQRT, LN, LG10, EXP, MEAN, SUM, SD, RND, TRUNC, MIN,
  MAX, LAG. (Dibutuhkan untuk uji Glejser: `ABS_RES = ABS(RES_1)`.)
- **Recode into Different Variables** (dasar).

### 6.4 Analyze
| Menu SPSS | Output yang wajib sama |
|---|---|
| Descriptive Statistics → Frequencies | Statistics, tabel frekuensi, histogram |
| Descriptive Statistics → Descriptives | N, Min, Max, Mean, Std. Deviation (+opsi Skewness/Kurtosis), Save standardized |
| Descriptive Statistics → Explore | Descriptives, **Tests of Normality** (Kolmogorov-Smirnov dengan koreksi Lilliefors + Shapiro-Wilk), Normal Q-Q plot |
| Compare Means → One-Sample T Test | One-Sample Statistics, One-Sample Test |
| Compare Means → Independent-Samples T Test | Group Statistics, Levene's Test + t-test (equal / not equal variances) |
| Compare Means → Paired-Samples T Test | Paired Samples Statistics, Correlations, Test |
| Compare Means → One-Way ANOVA | Descriptives, Test of Homogeneity of Variances, ANOVA |
| Regression → Linear (metode Enter) | Variables Entered/Removed, **Model Summary** (R, R², Adjusted R², Std. Error, **Durbin-Watson**), **ANOVA** (uji F), **Coefficients** (B, Std. Error, Beta, t, Sig., Tolerance, **VIF**), Collinearity Diagnostics, Residuals Statistics, plot: histogram residual, **Normal P-P Plot**, scatterplot ZPRED vs SRESID, Save: Unstandardized/Standardized residuals & predicted values (RES_1, ZRE_1, PRE_1, …) |
| Nonparametric Tests → Legacy Dialogs → 1-Sample K-S | One-Sample Kolmogorov-Smirnov Test (untuk normalitas residual) |
| Nonparametric Tests → Legacy Dialogs → Runs | Runs Test (uji autokorelasi alternatif) |

Uji asumsi klasik dikerjakan persis seperti di modul kuliah berbasis SPSS:
normalitas (K-S residual / P-P plot), multikolinearitas (Tolerance & VIF),
heteroskedastisitas (scatterplot + uji Glejser via Compute ABS lalu Regression),
autokorelasi (Durbin-Watson / Runs Test). Tidak ada menu "uji asumsi klasik"
khusus — tetap 1:1 dengan SPSS.

### 6.5 Output Viewer
- Jendela terpisah. Kiri: outline (judul prosedur → tabel/grafik). Kanan:
  dokumen output bergulir dengan judul prosedur, catatan, tabel, grafik.
- Tabel bergaya pivot table SPSS default (header bertingkat, footnote a/b/c,
  catatan "a. Dependent Variable: Y", "b. Predictors: (Constant), X1, X2").
- Klik kanan tabel → **Copy** (ke clipboard sebagai HTML + RTF + teks, supaya
  tempel ke Word tetap jadi tabel) dan **Copy as Image**. Grafik → Copy as Image.
- Ekspor seluruh output ke `.docx` dan `.pdf` → Tahap 2.

### 6.6 Lain-lain
- Undo/redo di Data Editor.
- Autosave/recovery data setiap beberapa menit.
- Menu Graphs Tahap 1: hanya grafik yang dihasilkan prosedur di atas.

## 7. Arsitektur

Workspace Cargo dengan crate terpisah:

```
statik/
  crates/
    stat-core/     # matematika murni: distribusi, OLS, uji statistik. Tanpa UI.
    dataset/       # model data: variabel, kasus, missing, value labels
    sav-io/        # baca/tulis .sav; impor csv/xlsx
    procedures/    # tiap prosedur: opsi → hitung (pakai stat-core) → OutputItem
    output-model/  # OutputItem: tabel/grafik/teks terstruktur + render ke HTML/RTF
    app/           # eframe/egui: Data Editor, menu, dialog, Output Viewer
```

- Prosedur tidak tahu soal UI: menerima `Dataset` + struct opsi, mengembalikan
  daftar `OutputItem`. UI dialog membangun struct opsi. Menambah prosedur =
  crate `procedures` + satu dialog di `app` (G5).
- `output-model` adalah satu-satunya tempat aturan format angka SPSS, supaya
  tampilan layar dan hasil copy ke Word identik.
- Grafik digambar sendiri (egui painter / plotting crate) dan bisa dirender ke
  PNG untuk Copy as Image.

## 8. Ketepatan angka (G2)

- Algoritma mengikuti dokumen publik *IBM SPSS Statistics Algorithms* bila ada,
  plus algoritma standar yang dipakai SPSS: Shapiro-Wilk (Royston AS R94),
  Lilliefors (aproksimasi Dallal-Wilkinson, dipotong ke 0.200 seperti SPSS:
  ".200*  This is a lower bound of the true significance"), Levene berbasis
  mean, Durbin-Watson, distribusi t/F/normal presisi ganda.
- Setiap prosedur punya tes otomatis berisi dataset kecil + nilai acuan dari R
  (`lm`, `shapiro.test`, `nortest::lillie.test`, `car::vif`, `car::leveneTest`
  dengan `center=mean`, `lmtest::dwtest`, `t.test`) dan, bila ada, dari contoh
  output SPSS yang dipublikasikan. Toleransi: sama pada presisi tampilan SPSS.
- Pengguna diminta mengirim file `.sav` + screenshot output SPSS dari modul
  kuliah sebagai acuan tambahan.

## 9. Pendekatan yang dipertimbangkan

| Pendekatan | Kelebihan | Kekurangan | Keputusan |
|---|---|---|---|
| **Rust + egui** | `.exe` 5–10 MB, cepat, aman dari crash, inti statistik mudah dites | Semua kontrol (grid, dialog, tabel output) digambar & disusun sendiri; clipboard HTML/RTF perlu kode Win32 tambahan | **Dipilih** pengguna |
| C++ + Qt | Widget kantoran matang, copy ke Word mudah | 30–50 MB, build Windows rumit, rawan crash | Ditolak |
| Tauri (Rust + WebView) | Output HTML mudah, installer kecil | RAM ≥ 150 MB, grid besar perlu virtualisasi web | Ditolak (kurang ringan) |
| Win32 murni | Paling ringan | Waktu pengembangan berbulan-bulan untuk grid/dialog | Ditolak |

## 10. Peta jalan setelah Tahap 1 (urutan usulan)

1. Correlate → Bivariate (Pearson/Spearman) dan Scale → **Reliability Analysis**
   (Cronbach's Alpha) — uji validitas & reliabilitas kuesioner, sangat sering
   dipakai tugas/skripsi.
2. Crosstabs + Chi-Square, Nonparametric legacy (Mann-Whitney, Wilcoxon,
   Kruskal-Wallis, Friedman).
3. Regression: metode Stepwise/Forward/Backward/Remove, regresi moderasi
   (interaksi via Compute), Curve Estimation, Binary Logistic.
4. GLM Univariate, Repeated Measures, Factor Analysis, Cluster.
5. Ekspor output `.docx`/`.pdf`, Syntax Editor & eksekusi syntax, Chart Builder.

## 11. Keputusan yang diambil Claude (bisa dikoreksi)

- Nama kerja "Statik".
- Bahasa antarmuka **Inggris**, sama dengan SPSS, karena modul/tutorial kuliah
  merujuk menu berbahasa Inggris ("Analyze → Regression → Linear").
- Gaya visual sendiri (warna, ikon) dengan tata letak SPSS.
- Grafik Tahap 1 hanya yang dihasilkan prosedur di §6.4.
- Menu yang belum diimplementasikan tetap tampil tapi nonaktif (abu-abu),
  supaya struktur menu tetap 1:1 dan teman-teman tahu fitur itu akan datang.
- Tombol Paste di Tahap 1 hanya menampilkan syntax setara (bukan eksekusi).
- Output dari 1-Sample K-S mengikuti perilaku SPSS versi yang dipakai dosen
  (lihat §12).

## 12. Versi SPSS acuan (diputuskan pengguna)

- Acuan perilaku dan angka: **SPSS Statistics versi terbaru** (rilis 30+).
  Konsekuensinya, 1-Sample K-S memakai koreksi Lilliefors secara default
  (perilaku sejak v27), dan format tabel output mengikuti tampilan versi terbaru.

## 13. Diserahkan ke pembuat

- Pilihan crate (linear algebra, distribusi, plotting, xlsx, clipboard), selama
  ukuran `.exe` tetap dalam target G4.
- Detail format penyimpanan output milik sendiri.
- Desain visual tabel output (selama struktur kolom/baris/footnote sama SPSS).
