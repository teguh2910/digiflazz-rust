# Digiflazz Client Library & Service

Unofficial Rust client, CLI, and HTTP microservice for the
[Digiflazz](https://digiflazz.com) buyer API.

This repository is a Cargo workspace:

| Crate | Path | Purpose |
|---|---|---|
| `digiflazz` | `crates/digiflazz` | Async SDK + `digiflazz` CLI |
| `digiflazz-service` | `crates/digiflazz-service` | HTTP microservice (axum) |

> Integrating a client? Read **[INTEGRATION.md](INTEGRATION.md)** — it documents
> the endpoint contracts, error mapping, `rc` retry safety, and the `testing`
> flag behaviour.

## Service

Runs an HTTP API in front of the SDK, with bearer auth and a cached price list.

```sh
SERVICE_API_TOKEN=change-me-at-least-16 \
DIGIFLAZZ_USERNAME=your-username \
DIGIFLAZZ_API_KEY=your-key \
cargo run -p digiflazz-service
```

### Endpoints

| Method | Path | Auth | Notes |
|---|---|---|---|
| GET | `/health` | no | Liveness only; never calls Digiflazz |
| GET | `/balance` | yes | Remaining deposit |
| GET | `/price-list?kind=prepaid\|pasca` | yes | Cached for `PRICE_LIST_TTL_SECS` |
| POST | `/transaction` | yes | Prepaid or postpaid |
| POST | `/deposit` | yes | Creates a deposit ticket |

Every route except `/health` requires `Authorization: Bearer <SERVICE_API_TOKEN>`.

```sh
curl -s localhost:8080/health
curl -s -H "Authorization: Bearer $TOKEN" localhost:8080/balance
curl -s -H "Authorization: Bearer $TOKEN" 'localhost:8080/price-list?kind=prepaid'
curl -s -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"sku":"xld10","customer_no":"087800001230","ref_id":"svc-1","testing":true}' \
  localhost:8080/transaction
```

### Configuration

| Variable | Default | Notes |
|---|---|---|
| `DIGIFLAZZ_USERNAME` | – | required |
| `DIGIFLAZZ_API_KEY` | – | required (dev or production key) |
| `DIGIFLAZZ_BASE_URL` | `https://api.digiflazz.com/v1` | override for sandbox/tests |
| `DIGIFLAZZ_ENV` | `development` | `production` changes the `testing` default |
| `SERVICE_API_TOKEN` | – | required, min 16 characters |
| `PORT` | `8080` | listen port |
| `PRICE_LIST_TTL_SECS` | `300` | price list cache TTL |
| `RUST_LOG` | `info,digiflazz_service=debug` | log filter |

### `testing` behaviour

`POST /transaction` accepts an optional `testing` flag:

- **development** — when omitted, defaults to `true`, so a stray call cannot move
  real balance.
- **production** — when omitted, stays `false` (Digiflazz default) and logs a
  warning naming the `ref_id`.

### Error mapping

Upstream conditions are reported as `502` with the Digiflazz code preserved,
because they are not the caller's mistake:

```json
{ "error": "upstream_rejected", "rc": "83", "message": "Anda telah mencapai limitasi..." }
```

`rc 83` is the price-list rate limit, which is why the price list is cached.
Malformed requests return `400` with a JSON body naming the offending field.

### Docker

```sh
docker build -t digiflazz-service .
docker run --rm -p 8080:8080 \
  -e SERVICE_API_TOKEN=change-me-at-least-16 \
  -e DIGIFLAZZ_USERNAME=... -e DIGIFLAZZ_API_KEY=... \
  digiflazz-service
```

## CLI

The SDK crate also ships a `digiflazz` binary:

```sh
cargo install --path crates/digiflazz   # installs `digiflazz` into ~/.cargo/bin
# or run it without installing:
cargo run -p digiflazz -- --help
```

Credentials are read from the environment, and a `.env` file in the working
directory is loaded automatically (existing environment variables win):

```sh
cp .env.example .env
# DIGIFLAZZ_USERNAME=...
# DIGIFLAZZ_DEVELOPMENT_KEY=...
# DIGIFLAZZ_PRODUCTION_KEY=...
# DIGIFLAZZ_ENV=development
```

```sh
digiflazz cek-saldo
digiflazz daftar-harga prepaid --limit 5
digiflazz deposit --amount 1000000 --bank Flip --name "John Doe"
digiflazz transaksi --sku xld10 --customer 087800001230 --ref-id INV-1 --testing
digiflazz transaksi --sku pln --customer 530000000001 --ref-id INV-2 --cmd inq-pasca
digiflazz --env production cek-saldo --json
```

`--json` prints the structured response, and `--dry-run` (on `deposit` and
`transaksi`) shows the request without calling the API. Exit codes: `0` success,
`2` invalid usage or missing configuration, `1` API failure.

## Installation

```sh
cargo add digiflazz
```

Or add it to `Cargo.toml` directly:

```toml
[dependencies]
digiflazz = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## API

### Create a client

```rust
use digiflazz::Digiflazz;

let digiflazz = Digiflazz::new("your-digiflazz-username", "your-digiflazz-api-key")?;
```

`new` returns an error when the username or key is empty. Use
`Digiflazz::with_base_url(username, key, url)` to point at a sandbox or a proxy,
or `Digiflazz::builder(..).http_client(client)` to supply your own configured
`reqwest::Client` (timeouts, proxies, retries).

The client is cheap to clone and can be shared across tasks.

#### Cek Saldo

```rust
use digiflazz::PriceListKind;

let balance = digiflazz.cek_saldo().await?;
println!("remaining deposit: {}", balance.deposit);
```

#### Daftar Harga

```rust
let products = digiflazz.daftar_harga(PriceListKind::Prepaid).await?;

match products {
    digiflazz::PriceList::Prepaid(list) => {
        for product in list {
            println!("{} {}", product.buyer_sku_code, product.price);
        }
    }
    digiflazz::PriceList::Pasca(list) => { /* ... */ }
}
```

#### Tiket Deposit

```rust
use digiflazz::DepositRequest;

let ticket = digiflazz
    .deposit(&DepositRequest {
        amount: 10_000_000,
        bank: "BCA".into(),
        owner_name: "John Doe".into(),
    })
    .await?;

println!("transfer {} with remark {}", ticket.amount, ticket.notes);
```

#### Transaksi Top Up

```rust
use digiflazz::{Command, TransactionRequest};

// Prepaid
let topup = digiflazz
    .transaksi(&TransactionRequest::new("xld25", "087800001233", "INV-1"))
    .await?;

// Postpaid
let inquiry = digiflazz
    .transaksi(&TransactionRequest::with_command(
        "pln",
        "530000000003",
        "INV-2",
        Command::Inquiry,
    ))
    .await?;

println!("{} ({:?})", topup.message, topup.status());
```

Optional fields on `TransactionRequest`:

```rust
let mut request = TransactionRequest::new("xld25", "087800001233", "INV-1");
request.testing = true;                                  // development mode
request.max_price = Some(25_000);                        // price ceiling
request.callback_url = Some("https://example.com/hook".into());
request.allow_dot = Some(true);                          // allow "." in customer_no
```

NOTE:
* `Command::Inquiry` — cek tagihan
* `Command::Pay` — bayar tagihan
* `Command::Status` — cek status tagihan
* `Command::PlnSubscribe` — cek id pln
* Only set a command for postpaid (pascabayar) transactions.

## Errors

Every call returns `Result<T, digiflazz::Error>`. There are two distinct HTTP-200
failures to handle, because Digiflazz reports logical errors in the response body
rather than with an error status:

```rust
match digiflazz.cek_saldo().await {
    Ok(balance) => println!("{}", balance.deposit),
    // HTTP-level failure (401, 5xx, ...)
    Err(digiflazz::Error::Api { status, message }) => {
        eprintln!("digiflazz responded {status}: {message}");
    }
    // HTTP 200 with a non-"00" rc, e.g. rc 83 (price list rate limit) or rc 02 (failed transaction)
    Err(digiflazz::Error::Rejected { rc, message }) => {
        eprintln!("digiflazz rejected the request ({rc}): {message}");
    }
    Err(err) => eprintln!("request failed: {err}"),
}
```

## Live smoke test

An example exercises every endpoint against the real API. Credentials are read
from the environment, never from source:

```sh
cp .env.example .env    # fill in your username and keys
set -a; source .env; set +a
cargo run --example smoke
```

It uses the development key unless `DIGIFLAZZ_ENV=production`, and the prepaid
transaction always sets `testing: true`. A failing step is printed without
aborting the rest, which matters in practice: the price list is rate limited
(`rc 83`) after repeated calls.

## Webhook (Coming Soon)

## Seller Area (Coming Soon)

## License

MIT
