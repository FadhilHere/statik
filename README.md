# Statik

Statik adalah program statistik ringan untuk Windows 10/11, dibuat untuk mahasiswa yang
mengerjakan skripsi atau tugas statistik. Alur kerjanya mirip SPSS: data diisi di **Data
Editor** (Data View dan Variable View), analisis dipilih dari menu **Analyze**, dan hasilnya
muncul di jendela **Output** yang tabelnya bisa langsung disalin ke Word.

Statik bukan produk IBM dan tidak berafiliasi dengan IBM SPSS Statistics. Statik bisa membuka
dan menyimpan berkas data `.sav`, serta mengimpor CSV dan Excel.

## Menjalankan Statik

1. Salin `Statik.exe` ke folder mana saja (misalnya `Documents\Statik`). Tidak perlu instalasi.
2. Klik dua kali `Statik.exe`. Jendela **Statik Data Editor** terbuka dengan data kosong
   (`Untitled1`).
3. Buka data lewat **File ▸ Open ▸ Data...** (`.sav`, `.csv`, `.xlsx`), atau **File ▸ Import
   Data ▸ CSV Data... / Excel...** bila ingin mengatur pemisah kolom dan desimal.
   CSV hasil Excel Indonesia (pemisah `;`, desimal `,`) dikenali otomatis.
4. Simpan dengan **File ▸ Save** / **Save As...** (format `.sav`).

Bila Windows SmartScreen memperingatkan "unrecognized app", pilih **More info ▸ Run anyway**
(berkas belum ditandatangani secara digital).

### Perubahan yang belum disimpan dan autosave

- Saat menutup jendela, **File ▸ Exit**, **New**, **Open**, atau **Import** ketika data belum
  disimpan, Statik bertanya *"Save contents of data editor to ...?"*: **Yes** menyimpan dulu,
  **No** membuang perubahan, **Cancel** membatalkan.
- Setiap 3 menit data yang belum disimpan dicadangkan ke
  `%LOCALAPPDATA%\Statik\recovery\`. Bila Statik tertutup mendadak (listrik padam, crash),
  saat dibuka lagi muncul *"Statik closed unexpectedly. Recover data?"*. Pilih **Recover**
  untuk membuka cadangan terakhir sebagai data baru (`Untitled`), lalu simpan dengan
  **Save As...**.

## Tutorial singkat: regresi linear dan uji asumsi klasik

Contoh: variabel dependen `Y`, variabel independen `X1`, `X2`, `X3`.

### 1. Regresi linear, normalitas grafik, multikolinearitas, autokorelasi

1. **Analyze ▸ Regression ▸ Linear...**
2. Pindahkan `Y` ke **Dependent** dan `X1`, `X2`, `X3` ke **Independent(s)** dengan tombol
   panah.
3. **Statistics...**: centang **Estimates**, **Model fit**, **Collinearity diagnostics**
   (uji multikolinearitas: Tolerance > 0,10 dan VIF < 10) dan **Durbin-Watson** (uji
   autokorelasi). Klik **Continue**.
4. **Plots...**: pindahkan `*SRESID` ke **Y** dan `*ZPRED` ke **X** (scatterplot untuk melihat
   pola heteroskedastisitas), centang **Histogram** dan **Normal probability plot** (normalitas
   residual secara grafik). Klik **Continue**.
5. **Save...**: centang **Residuals ▸ Unstandardized**. Klik **Continue**.
6. Klik **OK**. Tabel *Model Summary*, *ANOVA*, *Coefficients* dan grafik muncul di Output,
   dan variabel baru `RES_1` (residual) ditambahkan ke data.

### 2. Uji normalitas Kolmogorov-Smirnov (residual)

1. **Analyze ▸ Nonparametric Tests ▸ Legacy Dialogs ▸ 1-Sample K-S...**
2. Pindahkan `RES_1` ke **Test Variable List**, pastikan **Normal** tercentang, klik **OK**.
3. Lihat baris **Asymp. Sig. (2-tailed)**: residual berdistribusi normal bila nilainya > 0,05.

### 3. Uji heteroskedastisitas Glejser

1. **Transform ▸ Compute Variable...**
2. **Target Variable**: `ABS_RES`; **Numeric Expression**: `ABS(RES_1)`. Klik **OK**.
3. **Analyze ▸ Regression ▸ Linear...**: **Dependent** `ABS_RES`, **Independent(s)** `X1`,
   `X2`, `X3` (kosongkan centang Save agar tidak menambah residual baru). Klik **OK**.
4. Pada tabel *Coefficients*, tidak terjadi heteroskedastisitas bila **Sig.** setiap variabel
   independen > 0,05.

## Uji t

- **Analyze ▸ Compare Means ▸ One-Sample T Test...**: satu variabel dibandingkan dengan nilai
  uji (**Test Value**).
- **Analyze ▸ Compare Means ▸ Independent-Samples T Test...**: pindahkan variabel ke **Test
  Variable(s)**, variabel kelompok ke **Grouping Variable**, klik **Define Groups...** dan isi
  kode kedua kelompok (misalnya `1` dan `2`). Baca baris *Equal variances assumed* bila
  *Levene's Test* Sig. > 0,05, selain itu *Equal variances not assumed*.
- **Analyze ▸ Compare Means ▸ Paired-Samples T Test...**: pilih pasangan variabel (misalnya
  `pretest` dan `posttest`) sebagai **Paired Variables**.

Analisis lain yang tersedia: **Descriptive Statistics** (Frequencies, Descriptives, Explore
dengan uji normalitas Shapiro-Wilk), **One-Way ANOVA**, **Runs Test**, **Recode into Different
Variables**.

## Menyalin tabel ke Word

1. Di jendela Output, klik tabel (atau grafik) yang ingin disalin.
2. Klik kanan ▸ **Copy** (atau **Edit ▸ Copy**), lalu tempel di Word dengan **Ctrl+V**. Tabel
   masuk sebagai tabel Word yang bisa diedit (format HTML), dan sebagai teks dengan tab bila
   ditempel ke Excel atau Notepad.
3. Untuk menyalin sebagai gambar, pilih **Copy as Image**.

Hasil Output bisa disimpan dengan **File ▸ Save Output As...** di jendela Output (berkas
`.stko`) dan dibuka lagi dengan **File ▸ Open Output...**.

## Keterbatasan

Statik baru mencakup prosedur yang paling sering dipakai untuk skripsi; menu yang belum
tersedia tampil abu-abu. Daftar lengkap keterbatasan dan perbedaan hasil yang diketahui ada di
[KNOWN_GAPS.md](KNOWN_GAPS.md). Selalu periksa kembali hasil penting dengan referensi atau
program lain bila ragu.

## Membangun dari kode sumber

Butuh Rust (toolchain MSVC). Jalankan `bash tools/release.sh`; hasilnya `dist/Statik.exe`.
Tes: `cargo test --workspace`.
