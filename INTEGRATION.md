# Integrasi `digiflazz-service`

Panduan untuk tim yang akan memanggil service ini (backend, worker, atau
aplikasi lain). Semua contoh di bawah sudah dijalankan terhadap service yang
sama, jadi bentuk responsnya sesuai dengan kenyataan, bukan asumsi.

---

## 1. Ringkasan

`digiflazz-service` adalah HTTP wrapper di atas API Digiflazz. Service ini:

- menyimpan kredensial Digiflazz (consumer **tidak pernah** memegang API key),
- menyembunyikan signing MD5 dan penggabungan payload,
- meng-cache price list karena Digiflazz membatasi pengecekan maksimal 1x per 5 menit,
- menerjemahkan kondisi upstream menjadi status HTTP yang bisa diprogram.

| Item | Nilai |
|---|---|
| Base URL | `http://<host>:8080` |
| Format | JSON (request dan response) |
| Auth | `Authorization: Bearer <SERVICE_API_TOKEN>` |
| Content-Type | `application/json` untuk semua `POST` |
| Timeout upstream | tidak di-set eksplisit (lihat §8) |

---

## 2. Menjalankan service

### Lokal

```sh
cp .env.example .env    # isi kredensial

# .env minimal untuk service:
# DIGIFLAZZ_USERNAME=...
# DIGIFLAZZ_API_KEY=...          <- dev atau production key
# SERVICE_API_TOKEN=...          <- min 16 karakter
# DIGIFLAZZ_ENV=development      <- atau production

cargo run -p digiflazz-service
```

Service memuat `.env` otomatis lewat `dotenvy`. Variabel yang sudah ada di
environment **menang** atas isi `.env`.

### Docker

```sh
docker build -t digiflazz-service .
docker run --rm -p 8080:8080 \
  -e SERVICE_API_TOKEN=change-me-at-least-16 \
  -e DIGIFLAZZ_USERNAME=... \
  -e DIGIFLAZZ_API_KEY=... \
  -e DIGIFLAZZ_ENV=production \
  digiflazz-service
```

Image memakai user non-root (`app`, uid 10001), sudah menyertakan
`ca-certificates` (wajib untuk TLS ke `api.digiflazz.com`), dan mendengarkan di
`0.0.0.0:8080`.

### Variabel environment

| Variabel | Wajib | Default | Keterangan |
|---|---|---|---|
| `DIGIFLAZZ_USERNAME` | ya | – | username akun Digiflazz |
| `DIGIFLAZZ_API_KEY` | ya | – | **satu** key; dev atau production |
| `SERVICE_API_TOKEN` | ya | – | bearer token, minimal 16 karakter |
| `DIGIFLAZZ_ENV` | – | `development` | `production` mengubah default `testing` |
| `DIGIFLAZZ_BASE_URL` | – | `https://api.digiflazz.com/v1` | untuk sandbox/proxy |
| `PORT` | – | `8080` | port listen |
| `PRICE_LIST_TTL_SECS` | – | `300` | TTL cache price list |
| `RUST_LOG` | – | `info,digiflazz_service=debug` | filter log |

Service **menolak start** kalau ada variabel wajib yang kosong, atau kalau
`SERVICE_API_TOKEN` kurang dari 16 karakter. Pesan errornya menyebutkan semua
variabel yang kurang sekaligus.

---

## 3. Autentikasi

Kirim header berikut di **semua** endpoint kecuali `/health`:

```
Authorization: Bearer <SERVICE_API_TOKEN>
```

Token dibandingkan secara constant-time. Prefiks `Bearer ` dan `bearer ` dua-duanya
diterima.

| Kondisi | Status | Body |
|---|---|---|
| Header tidak ada | `401` | `{"error":"unauthorized"}` |
| Format bukan Bearer | `401` | `{"error":"unauthorized"}` |
| Token salah | `401` | `{"error":"unauthorized"}` |

Perilaku yang sudah diuji: `/balance` tanpa token → `401`, dengan token benar →
`200`. `/health` tanpa token tetap `200`.

---

## 4. Endpoint

### `GET /health` — tanpa auth

Tidak memanggil Digiflazz, jadi aman dipakai sebagai liveness/readiness probe
tanpa menghabiskan rate limit.

```sh
curl -s localhost:8080/health
```

```json
{"status":"ok","version":"0.1.12"}
```

> Gunakan endpoint ini untuk probe, **bukan** `/balance`. `/balance` memanggil
> upstream setiap kali.

### `GET /balance`

```sh
curl -s -H "Authorization: Bearer $TOKEN" localhost:8080/balance
```

```json
{"deposit":0.0}
```

`deposit` bertipe float (bisa desimal). Nilai `0.0` berarti saldo habis, bukan
error.

### `GET /price-list?kind=prepaid|pasca`

```sh
curl -s -H "Authorization: Bearer $TOKEN" 'localhost:8080/price-list?kind=prepaid'
curl -s -H "Authorization: Bearer $TOKEN" 'localhost:8080/price-list?kind=pasca'
```

- `kind` opsional; default `prepaid`.
- Nilai diterima: `prepaid`, `pasca` (alias `postpaid`). Selain itu → `400`.
- Respons dibungkus: `{"kind":"prepaid","products":[...]}`. Isi `products`
  mengikuti bentuk Digiflazz apa adanya.

```json
{
  "kind": "prepaid",
  "products": [
    {
      "product_name": "Axis 10.000",
      "category": "Pulsa",
      "brand": "AXIS",
      "type": "Umum",
      "seller_name": "AG***",
      "price": 10992,
      "buyer_sku_code": "ax10",
      "buyer_product_status": true,
      "seller_product_status": true,
      "unlimited_stock": true,
      "stock": 0,
      "multi": true,
      "start_cut_off": "00:00",
      "end_cut_off": "00:00",
      "desc": "..."
    }
  ]
}
```

Akun tanpa produk pasca akan menerima `{"kind":"pasca","products":[]}` dengan
status `200` (Digiflazz mengirim `data: null`, service mengubahnya jadi array
kosong).

**Cache.** Hasil di-cache per `kind` selama `PRICE_LIST_TTL_SECS` (default 300s).
Ini bukan optimasi opsional: Digiflazz membatasi pengecekan seluruh price list
menjadi **maksimal 1x per 5 menit** (`rc 83`). Jadi:

- request kedua dalam TTL tidak menyentuh Digiflazz (terukur ~0 ms vs ~20 ms),
- jangan menurunkan TTL di bawah 300 tanpa alasan kuat,
- kalau butuh data lebih segar, simpan sendiri di database seperti anjuran
  Digiflazz, jangan polling endpoint ini.

### `POST /transaction`

Membuat transaksi prepaid atau postpaid pascabayar.

```sh
curl -s -X POST -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"sku":"xld10","customer_no":"087800001230","ref_id":"INV-1","testing":true}' \
  localhost:8080/transaction
```

| Field | Wajib | Tipe | Keterangan |
|---|---|---|---|
| `sku` | ya | string | `buyer_sku_code` produk |
| `customer_no` | ya | string | nomor tujuan / id pelanggan |
| `ref_id` | ya | string | **unik** per transaksi; dipakai di signature |
| `command` | – | string | hanya untuk pascabayar (lihat tabel di bawah) |
| `testing` | – | bool | lihat §6, default berbeda per environment |
| `max_price` | – | int | batas harga yang mau dibayar |
| `callback_url` (`cb_url`) | – | string | override webhook per transaksi |
| `allow_dot` | – | bool | izinkan titik di `customer_no` |

`command` yang diterima:

| Nilai | Fungsi |
|---|---|
| `inq-pasca` | cek tagihan |
| `pay-pasca` | bayar tagihan |
| `status-pasca` | cek status tagihan |
| `pln-subscribe` | cek id PLN |

Nilai lain → `400`. Jangan kirim `command` untuk transaksi prepaid.

Contoh pascabayar:

```sh
curl -s -X POST -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"sku":"pln","customer_no":"530000000001","ref_id":"INV-2","command":"inq-pasca"}' \
  localhost:8080/transaction
```

Response (bentuk asli, sudah diverifikasi):

```json
{
  "ref_id": "INV-2",
  "customer_no": "530000000001",
  "buyer_sku_code": "pln",
  "message": "Transaksi Sukses",
  "rc": "00",
  "sn": "",
  "buyer_last_saldo": 100000.0,
  "price": 10000,
  "status": "Sukses",
  "desc": { "...": "khusus pascabayar, ikut apa adanya" }
}
```

Field khusus pascabayar (`customer_name`, `admin`, `selling_price`, `desc`)
diteruskan apa adanya di level atas objek. Jangan mengasumsikan skema tetap —
pakai field yang Anda butuhkan saja.

### `POST /deposit`

Membuat tiket deposit (instruksi transfer).

```sh
curl -s -X POST -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"amount":200000,"bank":"Flip","owner_name":"John Doe"}' \
  localhost:8080/deposit
```

| Field | Wajib | Tipe | Keterangan |
|---|---|---|---|
| `amount` | ya | int | nominal deposit |
| `bank` | ya | string | lihat catatan bank di bawah |
| `owner_name` (alias `name`) | ya | string | nama pemilik rekening pengirim |

```json
{
  "rc": "00",
  "bank": "Flip (BCA)",
  "payment_method": "Bank Transfer",
  "account_no": "6042888890",
  "amount": 201927,
  "notes": ""
}
```

Dua jebakan yang sudah terverifikasi:

1. **Bank tergantung tipe akun.** Akun perorangan hanya menerima `Flip` dan
   `ShopeePay`. `BCA`/`MANDIRI`/`BRI`/`BNI` hanya untuk akun perusahaan dan akan
   ditolak `502` dengan pesan `Coba pilihan bank lainnya: Flip, ShopeePay`.
2. **Nominal minimum Rp200.000**, dan `amount` pada respons **berbeda** dari
   yang dikirim (`200000` → `201927`) karena ada biaya tambahan. Selalu pakai
   `amount` dari respons, jangan dari request. Untuk Flip, `notes` memang kosong
   (bukan bug).

---

## 5. Format error

Semua error berbentuk JSON dengan field `error` sebagai diskriminator:

```json
{"error":"<kode>", "message":"<penjelasan>", ...field tambahan}
```

| `error` | HTTP | Penyebab | Tindakan yang disarankan |
|---|---|---|---|
| `unauthorized` | 401 | token tidak ada/salah | perbaiki header |
| `bad_request` | 400 | body/query tidak valid | perbaiki request; baca `message` |
| `upstream_rejected` | 502 | Digiflazz menolak dengan `rc` bukan `00` | baca `rc`, lihat tabel §7 |
| `upstream_error` | 502 | HTTP error dari Digiflazz **tanpa** `rc` di body | cek `status` + `message` |
| `upstream_invalid_response` | 502 | respons Digiflazz berubah bentuk | lapor ke tim service |
| `upstream_unavailable` | 504 | gagal konek / TLS / jaringan | retry dengan backoff |
| `misconfigured` | 500 | kredensial/konfigurasi service salah | lapor ke tim ops |

**Penting soal `rc` vs status HTTP.** Digiflazz tidak konsisten: sebagian
penolakan datang sebagai HTTP `200` dengan `rc`, sebagian lain sebagai HTTP `4xx`
yang **tetap** membawa `rc` di body. Service menormalkan keduanya menjadi
`upstream_rejected` selama `rc` ada, sehingga Anda selalu punya kode yang bisa
dipakai memutuskan retry.

Contoh nyata dari production untuk SKU yang tidak aktif (upstream membalas
HTTP 400, service membalas 502):

```json
{"error":"upstream_rejected","message":"SKU tidak di temukan atau Non-Aktif","rc":"43"}
```

Contoh nyata `upstream_rejected` untuk rate limit (upstream HTTP 200):

```json
{"error":"upstream_rejected","rc":"83","message":"Anda telah mencapai limitasi pengecekan pricelist, silahkan coba beberapa saat lagi"}
```

`upstream_error` hanya muncul bila upstream membalas HTTP error **tanpa** `rc`
(misalnya halaman 503 dari load balancer).

Contoh `bad_request` saat field kurang:

```json
{"error":"bad_request","message":"Failed to deserialize the JSON body into the target type: missing field `owner_name` at line 1 column 31"}
```

> Catatan: `404` dan `405` (route/metode salah) **tidak** ber-body JSON karena
> dibuat oleh axum sebelum handler jalan. Jangan bergantung pada body untuk dua
> status itu.

---

## 6. Perilaku `testing` — baca ini sebelum production

Flag `testing` menentukan apakah transaksi menyentuh uang sungguhan.

| `DIGIFLAZZ_ENV` | `testing` dikirim | Efek |
|---|---|---|
| `development` | tidak | **default `true`** (aman) |
| `development` | `true` / `false` | sesuai yang dikirim |
| `production` | tidak | `false` (uang nyata) + log `WARN` berisi `ref_id` |
| `production` | `true` / `false` | sesuai yang dikirim |

Tujuannya mencegah kecelakaan paling umum: kebiasaan memakai dev key terbawa ke
production. Tetap kirim `testing` secara eksplisit di production — jangan
bergantung pada default.

> **Penting (terverifikasi di kedua mode):** perilaku Digiflazz berbeda tajam
> antara development dan production.
>
> | | Development (sandbox) | Production |
> |---|---|---|
> | SKU tidak ada | dibalas `Sukses` + `sn` palsu | ditolak `rc 43`, HTTP 400 |
> | `testing: false` | diabaikan | dihormati |
> | Saldo | saldo mainan | saldo asli |
> | Data price list | contoh | produk aktif sungguhan |
>
> Konsekuensinya: **dev key tidak bisa dipakai untuk menguji jalur error atau
> validasi SKU** (semuanya dibalas sukses). Jalur error hanya bisa diuji lewat
> mock, atau di production.
>
> Selain itu, key terikat ke mode akun. Setelah akun di-switch ke Production di
> dashboard, **development key berhenti berlaku** dan dibalas `rc 41`
> (Signature tidak valid). Jangan menganggap `rc 41` sebagai kesalahan formula
> signature sebelum memastikan mode akun dan key cocok.

---

## 7. Kode respons Digiflazz (`rc`)

Saat `upstream_rejected`, field `rc` diteruskan apa adanya. Kolom "Transaksi
terbentuk" menentukan apakah aman retry — kalau **Ya**, artinya transaksi sudah
tercatat dan retry dengan `ref_id` baru bisa menyebabkan double charge.

| `rc` | Arti | Transaksi terbentuk | Retry aman? |
|---|---|---|---|
| `00` | Sukses | Ya | – |
| `01` | Timeout | Ya | **jangan** retry, cek status dulu |
| `02` | Transaksi gagal | Ya | tidak (sudah tercatat) |
| `03` | Pending | Ya | cek status, jangan buat baru |
| `40` | Payload error | Tidak | ya, setelah perbaiki payload |
| `41` | Signature tidak valid | Tidak | tidak, perbaiki kredensial |
| `42` | Username tidak sesuai | Tidak | tidak |
| `43` | SKU tidak ditemukan / non-aktif | Tidak | tidak, perbaiki SKU |
| `44` | Saldo tidak cukup | Tidak | ya, setelah top up |
| `45` | IP tidak dikenali | Tidak | tidak, whitelist IP |
| `47` | Transaksi sudah terjadi di buyer lain | Tidak | tidak |
| `49` | Ref ID tidak unik | Tidak | ya, dengan `ref_id` baru |
| `83` | Limit pengecekan pricelist | Tidak | ya, setelah beberapa saat |
| `85` | Limit transaksi | Ya | tunggu ~1 menit |
| `99` | DF Router issue | Ya | cek status |

`01`, `02`, `03`, `50`–`75`, `84`–`86`, `99` termasuk yang **sudah membentuk
transaksi**. Untuk semua kondisi "transaksi terbentuk", jangan retry dengan
`ref_id` baru — cek status memakai `ref_id` yang sama.

---

## 8. Panduan implementasi consumer

**Timeout.** Service tidak memasang timeout eksplisit ke Digiflazz, jadi pasang
timeout di sisi klien Anda: 30–35 detik untuk `/transaction` dan `/deposit`,
10 detik untuk `/balance` dan `/price-list`. Timeout di klien **tidak** berarti
transaksi gagal — bisa saja sudah terbentuk. Perlakukan sebagai "unknown" dan
cek status dengan `ref_id` yang sama.

**Retry.** Hanya retry untuk:

- `504 upstream_unavailable` (aman, belum sampai Digiflazz),
- `502` dengan `rc` yang "Transaksi terbentuk = Tidak" (`40`, `43`, `44`, `49`, `83`),
- `429`/`5xx` dari infrastruktur Anda sendiri.

Jangan retry untuk `401` dan `400` tanpa mengubah request.

**Idempotensi.** `ref_id` adalah kunci idempotensi Anda. Simpan sebelum
mengirim request, dan pakai `ref_id` yang sama saat mengecek status. Jangan
generate ulang `ref_id` hanya karena klien timeout.

**Urutan pascabayar.** Untuk tagihan, urutannya wajib:
`inq-pasca` → (hitung tagihan) → `pay-pasca` dengan **`ref_id` yang sama**, di
hari yang sama. Pembayaran harus memakai `ref_id` hasil inquiry.

**Price list.** Jangan panggil per user request. Simpan hasilnya di database
Anda dan segarkan berkala. Service sudah meng-cache 5 menit, tetapi bila ada
beberapa replika, tiap replika punya cache sendiri (N replika = N panggilan per
TTL).

**Logging & korelasi.** Setiap respons memiliki header `x-request-id`. Kalau
Anda mengirim header itu sendiri, nilainya dipakai ulang; kalau tidak, service
membuat UUID. Sertakan `x-request-id` dan `ref_id` di log Anda — itu yang
dibutuhkan saat menelusuri masalah. Contoh terverifikasi:

```
$ curl -D - -o /dev/null localhost:8080/health
x-request-id: fd9cb346-ee4c-4fa3-818f-55269664ab59

$ curl -D - -o /dev/null -H 'x-request-id: my-trace-abc' localhost:8080/health
x-request-id: my-trace-abc
```

Setiap endpoint juga mengembalikan header tersebut, termasuk pada `401`.

**Health probe.** Gunakan `/health` (tidak perlu auth, tidak memanggil
Digiflazz). Jangan pakai `/balance` untuk probe.

---

## 9. Contoh klien

### TypeScript / Node

```ts
const BASE = process.env.DIGIFLAZZ_SERVICE_URL!;
const TOKEN = process.env.DIGIFLAZZ_SERVICE_TOKEN!;

async function transaksi(body: {
  sku: string;
  customer_no: string;
  ref_id: string;
  command?: string;
  testing?: boolean;
}) {
  const response = await fetch(`${BASE}/transaction`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${TOKEN}`,
      "content-type": "application/json",
      "x-request-id": body.ref_id,
    },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(35_000),
  });

  const requestId = response.headers.get("x-request-id");
  const payload = await response.json();

  if (!response.ok) {
    // payload.error: unauthorized | bad_request | upstream_rejected | ...
    throw new Error(
      `${payload.error}${payload.rc ? ` (rc ${payload.rc})` : ""}: ${payload.message} [${requestId}]`,
    );
  }

  return payload as { ref_id: string; status: string; rc: string; sn: string };
}
```

### Go

```go
type TransactionResponse struct {
	RefID     string `json:"ref_id"`
	Status    string `json:"status"`
	RC        string `json:"rc"`
	SN        string `json:"sn"`
	Message   string `json:"message"`
}

type ApiError struct {
	Error   string `json:"error"`
	Message string `json:"message"`
	RC      string `json:"rc"`
}

func Transaksi(ctx context.Context, body map[string]any) (*TransactionResponse, error) {
	payload, _ := json.Marshal(body)

	ctx, cancel := context.WithTimeout(ctx, 35*time.Second)
	defer cancel()

	req, _ := http.NewRequestWithContext(ctx, http.MethodPost,
		baseURL+"/transaction", bytes.NewReader(payload))
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("Content-Type", "application/json")
	if refID, ok := body["ref_id"].(string); ok {
		req.Header.Set("x-request-id", refID)
	}

	res, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("transport: %w", err) // unknown: cek status via ref_id
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusOK {
		var apiErr ApiError
		_ = json.NewDecoder(res.Body).Decode(&apiErr)
		return nil, fmt.Errorf("%s (rc %s): %s", apiErr.Error, apiErr.RC, apiErr.Message)
	}

	var out TransactionResponse
	if err := json.NewDecoder(res.Body).Decode(&out); err != nil {
		return nil, err
	}
	return &out, nil
}
```

---

## 10. Checklist sebelum go-live

- [ ] `SERVICE_API_TOKEN` diganti dari nilai contoh, minimal 16 karakter, disimpan di secret manager.
- [ ] `DIGIFLAZZ_ENV=production` dan `DIGIFLAZZ_API_KEY` diisi **production key**.
- [ ] IP server sudah di-whitelist di menu koneksi API Digiflazz (kalau diaktifkan).
- [ ] Timeout klien di-set (30–35s transaksi, 10s read).
- [ ] `ref_id` disimpan sebelum request dikirim, dan dipakai ulang untuk cek status.
- [ ] Retry hanya untuk kondisi yang aman (§8).
- [ ] Price list disimpan di database sendiri, tidak polling per request.
- [ ] `/health` dipakai untuk probe, bukan `/balance`.
- [ ] `x-request-id` dan `ref_id` tercatat di log.
- [ ] Jalur error (`rc` 44 saldo kurang, `rc` 43 SKU salah) diuji lewat mock, **bukan** dev key (§6).
- [ ] Alur pascabayar memakai `ref_id` yang sama antara `inq-pasca` dan `pay-pasca`.

---

## 11. Batasan yang perlu diketahui

- **Cache per proses.** Beberapa replika = beberapa cache. Belum ada cache bersama (Redis) karena tidak tersedia di lingkungan build saat ini.
- **Tanpa webhook.** Service belum menerima callback dari Digiflazz; status pending harus dicek manual via `status-pasca` atau transaksi ulang dengan `ref_id` yang sama.
- **Tanpa persistensi.** Service tidak menyimpan riwayat transaksi. Penyimpanan `ref_id` dan status adalah tanggung jawab consumer.
- **Satu kredensial.** Satu instance service melayani satu akun Digiflazz.
- **Belum ada rate limit di sisi service.** Pembatasan hanya dari Digiflazz. Kalau service diekspos ke banyak consumer, tambahkan rate limit di depan (ingress/gateway).
- **`404`/`405` tanpa body JSON.** Lihat §5.
- **Tanpa endpoint cek status.** Cek status dilakukan lewat `POST /transaction` dengan `command: "status-pasca"` dan `ref_id` yang sama.
