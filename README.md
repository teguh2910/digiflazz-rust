# Digiflazz Client Library

Unofficial async Rust client and CLI for the [Digiflazz](https://digiflazz.com) buyer API.

## CLI

The crate also ships a `digiflazz` binary:

```sh
cargo install --path .          # installs `digiflazz` into ~/.cargo/bin
# or run it without installing:
cargo run -- --help
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
