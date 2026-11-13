//! Command line interface for the Digiflazz client.

use std::fmt;
use std::process::ExitCode;

use digiflazz::{
    Command, DepositRequest, Digiflazz, Error, PriceList, PriceListKind, Transaction,
    TransactionRequest,
};

const HELP: &str = "\
digiflazz - unofficial CLI for the Digiflazz buyer API

USAGE:
    digiflazz [OPTIONS] <COMMAND> [ARGS]

OPTIONS:
    --env <development|production>  Which API key to use (default: development)
    --json                          Print the raw response as JSON
    -h, --help                      Show this help

COMMANDS:
    cek-saldo
    daftar-harga <prepaid|pasca> [--limit N]
    deposit --amount N --bank <BANK> --name <OWNER> [--dry-run]
    transaksi --sku <SKU> --customer <NO> --ref-id <ID>
              [--cmd inq-pasca|pay-pasca|status-pasca|pln-subscribe]
              [--testing] [--max-price N] [--cb-url URL] [--allow-dot] [--dry-run]

ENVIRONMENT:
    DIGIFLAZZ_USERNAME         Digiflazz username
    DIGIFLAZZ_DEVELOPMENT_KEY  Development API key
    DIGIFLAZZ_PRODUCTION_KEY   Production API key
    DIGIFLAZZ_ENV              Default environment (--env overrides it)

A local .env file in the working directory is loaded automatically; variables
already set in the environment take precedence.

EXAMPLES:
    digiflazz cek-saldo
    digiflazz daftar-harga prepaid --limit 5
    digiflazz deposit --amount 1000000 --bank BCA --name \"John Doe\" --dry-run
    digiflazz transaksi --sku xld10 --customer 087800001230 --ref-id INV-1 --testing
    digiflazz transaksi --sku pln --customer 530000000001 --ref-id INV-2 --cmd inq-pasca
    digiflazz --env production cek-saldo --json
";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(message)) => {
            eprintln!("error: {message}\n");
            eprint!("{HELP}");
            ExitCode::from(2)
        }
        Err(CliError::Failed(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
        Err(CliError::App(error)) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

enum CliError {
    Usage(String),
    Failed(String),
    App(Error),
}

impl From<Error> for CliError {
    fn from(error: Error) -> Self {
        Self::App(error)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::Failed(message) => write!(f, "{message}"),
            Self::App(error) => write!(f, "{error}"),
        }
    }
}

fn usage<T>(message: impl Into<String>) -> Result<T, CliError> {
    Err(CliError::Usage(message.into()))
}

#[derive(Default)]
struct Flags {
    json: bool,
    env: Option<String>,
}

impl Flags {
    fn take(args: &mut Vec<String>) -> Result<Self, CliError> {
        let mut flags = Self::default();
        let mut rest = Vec::with_capacity(args.len());

        {
            let mut iter = args.drain(..);

            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--json" => flags.json = true,
                    "--env" => match iter.next() {
                        Some(value) => flags.env = Some(value),
                        None => return usage("--env requires a value"),
                    },
                    _ if arg.starts_with("--env=") => {
                        flags.env = Some(arg["--env=".len()..].to_owned());
                    }
                    _ => rest.push(arg),
                }
            }
        }

        *args = rest;
        Ok(flags)
    }
}

/// Parsed `--key value` / `--key` pairs for a subcommand.
struct Opts {
    values: Vec<(String, Option<String>)>,
}

impl Opts {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        const BOOLEAN: [&str; 3] = ["--testing", "--allow-dot", "--dry-run"];

        let mut values = Vec::new();
        let mut index = 0;

        while index < args.len() {
            let arg = &args[index];
            if !arg.starts_with("--") {
                return usage(format!("unexpected argument `{arg}`"));
            }

            if BOOLEAN.contains(&arg.as_str()) {
                values.push((arg.clone(), None));
                index += 1;
                continue;
            }

            match args.get(index + 1) {
                Some(value) => {
                    values.push((arg.clone(), Some(value.clone())));
                    index += 2;
                }
                None => return usage(format!("{arg} requires a value")),
            }
        }

        Ok(Self { values })
    }

    fn flag(&self, name: &str) -> bool {
        self.values.iter().any(|(key, _)| key == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(key, _)| key == name)
            .and_then(|(_, value)| value.as_deref())
    }

    fn required(&self, name: &str) -> Result<&str, CliError> {
        match self.value(name) {
            Some(value) => Ok(value),
            None => usage(format!("{name} is required")),
        }
    }

    fn number(&self, name: &str) -> Result<Option<i64>, CliError> {
        match self.value(name) {
            None => Ok(None),
            Some(raw) => raw
                .parse()
                .map(Some)
                .map_err(|_| CliError::Usage(format!("{name} expects a number, got `{raw}`"))),
        }
    }

    fn required_number(&self, name: &str) -> Result<i64, CliError> {
        let raw = self.required(name)?;
        raw.parse()
            .map_err(|_| CliError::Usage(format!("{name} expects a number, got `{raw}`")))
    }

    fn check_known(&self, allowed: &[&str]) -> Result<(), CliError> {
        for (key, _) in &self.values {
            if !allowed.contains(&key.as_str()) {
                return usage(format!("unknown option `{key}` for this command"));
            }
        }
        Ok(())
    }
}

fn load_dotenv() {
    let Ok(contents) = std::fs::read_to_string(".env") else {
        return;
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim().trim_matches(['"', '\'']);

        if key.is_empty() || std::env::var_os(key).is_some() {
            continue;
        }

        std::env::set_var(key, value);
    }
}

fn build_client(flags: &Flags) -> Result<Digiflazz, CliError> {
    let username = std::env::var("DIGIFLAZZ_USERNAME").map_err(|_| {
        CliError::Failed("DIGIFLAZZ_USERNAME is not set (put it in .env or the environment)".into())
    })?;

    let environment = flags
        .env
        .clone()
        .or_else(|| std::env::var("DIGIFLAZZ_ENV").ok())
        .unwrap_or_else(|| "development".to_owned());

    let key_var = match environment.as_str() {
        "development" | "dev" => "DIGIFLAZZ_DEVELOPMENT_KEY",
        "production" | "prod" => "DIGIFLAZZ_PRODUCTION_KEY",
        other => {
            return usage(format!(
                "--env expects development or production, got `{other}`"
            ))
        }
    };

    let key = std::env::var(key_var).map_err(|_| {
        CliError::Failed(format!("{key_var} is not set (needed for {environment})"))
    })?;

    Ok(Digiflazz::new(username, key)?)
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), CliError> {
    let rendered = serde_json::to_string_pretty(value)
        .map_err(|error| CliError::Failed(format!("failed to render JSON: {error}")))?;
    println!("{rendered}");
    Ok(())
}

async fn run() -> Result<(), CliError> {
    load_dotenv();

    let mut args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "-h" || arg == "--help") || args.is_empty() {
        print!("{HELP}");
        return Ok(());
    }

    let flags = Flags::take(&mut args)?;

    let Some(command) = args.first().cloned() else {
        print!("{HELP}");
        return Ok(());
    };

    let rest = &args[1..];

    match command.as_str() {
        "cek-saldo" => {
            let opts = Opts::parse(rest)?;
            opts.check_known(&[])?;

            let balance = build_client(&flags)?.cek_saldo().await?;

            if flags.json {
                print_json(&balance)?;
            } else {
                println!("deposit: {}", balance.deposit);
            }
        }

        "daftar-harga" => {
            let Some(kind) = rest.first() else {
                return usage("daftar-harga requires prepaid or pasca");
            };

            let kind = match kind.as_str() {
                "prepaid" => PriceListKind::Prepaid,
                "pasca" | "postpaid" => PriceListKind::Pasca,
                other => {
                    return usage(format!(
                        "unknown price list `{other}`, use prepaid or pasca"
                    ))
                }
            };

            let opts = Opts::parse(&rest[1..])?;
            opts.check_known(&["--limit"])?;

            let limit = opts.number("--limit")?.unwrap_or(10);
            let limit = usize::try_from(limit).unwrap_or(0);

            let list = build_client(&flags)?.daftar_harga(kind).await?;

            if flags.json {
                match &list {
                    PriceList::Prepaid(products) => print_json(products)?,
                    PriceList::Pasca(products) => print_json(products)?,
                }
                return Ok(());
            }

            match list {
                PriceList::Prepaid(products) => {
                    println!("{} prepaid products", products.len());
                    for product in take(&products, limit) {
                        println!(
                            "  {:<12} {:>10}  {}",
                            product.buyer_sku_code, product.price, product.product_name
                        );
                    }
                }
                PriceList::Pasca(products) => {
                    println!("{} pasca products", products.len());
                    for product in take(&products, limit) {
                        println!(
                            "  {:<12} admin {:>8}  {}",
                            product.buyer_sku_code, product.admin, product.product_name
                        );
                    }
                }
            }
        }

        "deposit" => {
            let opts = Opts::parse(rest)?;
            opts.check_known(&["--amount", "--bank", "--name", "--dry-run"])?;

            let request = DepositRequest {
                amount: opts.required_number("--amount")?,
                bank: opts.required("--bank")?.to_owned(),
                owner_name: opts.required("--name")?.to_owned(),
            };

            if opts.flag("--dry-run") {
                println!(
                    "would create a deposit ticket for {} via {} owned by {}",
                    request.amount, request.bank, request.owner_name
                );
                return Ok(());
            }

            let ticket = build_client(&flags)?.deposit(&request).await?;

            if flags.json {
                print_json(&ticket)?;
            } else {
                println!("rc:             {}", ticket.rc);
                println!("bank:           {}", ticket.bank);
                println!("payment method: {}", ticket.payment_method);
                println!("account no:     {}", ticket.account_no);
                println!("amount:         {}", ticket.amount);
                println!("notes:          {}", ticket.notes);
            }
        }

        "transaksi" => {
            let opts = Opts::parse(rest)?;
            opts.check_known(&[
                "--sku",
                "--customer",
                "--ref-id",
                "--cmd",
                "--testing",
                "--max-price",
                "--cb-url",
                "--allow-dot",
                "--dry-run",
            ])?;

            let command = match opts.value("--cmd") {
                None => None,
                Some("inq-pasca") => Some(Command::Inquiry),
                Some("pay-pasca") => Some(Command::Pay),
                Some("status-pasca") => Some(Command::Status),
                Some("pln-subscribe") => Some(Command::PlnSubscribe),
                Some(other) => {
                    return usage(format!(
                        "unknown --cmd `{other}`, use inq-pasca, pay-pasca, status-pasca or pln-subscribe"
                    ))
                }
            };

            let mut request = TransactionRequest::new(
                opts.required("--sku")?,
                opts.required("--customer")?,
                opts.required("--ref-id")?,
            );
            request.command = command;
            request.testing = opts.flag("--testing");
            request.max_price = opts.number("--max-price")?;
            request.callback_url = opts.value("--cb-url").map(str::to_owned);
            request.allow_dot = opts.flag("--allow-dot").then_some(true);

            if opts.flag("--dry-run") {
                println!(
                    "would create transaction sku={} customer={} ref_id={} cmd={:?} testing={}",
                    request.sku,
                    request.customer_no,
                    request.ref_id,
                    request.command,
                    request.testing
                );
                return Ok(());
            }

            let transaction = build_client(&flags)?.transaksi(&request).await?;

            if flags.json {
                print_json(&transaction.raw)?;
            } else {
                print_transaction(&transaction);
            }
        }

        other => return usage(format!("unknown command `{other}`")),
    }

    Ok(())
}

fn take<T>(items: &[T], limit: usize) -> &[T] {
    if limit == 0 || limit >= items.len() {
        items
    } else {
        &items[..limit]
    }
}

fn print_transaction(transaction: &Transaction) {
    println!("status:  {:?}", transaction.status());
    println!("rc:      {}", transaction.rc);
    println!("message: {}", transaction.message);
    println!("ref_id:  {}", transaction.ref_id);

    if let Some(customer) = transaction
        .raw
        .get("customer_name")
        .and_then(|value| value.as_str())
    {
        println!("customer: {customer}");
    }

    if !transaction.sn.is_empty() {
        println!("sn:      {}", transaction.sn);
    }

    if let Some(price) = transaction.price {
        println!("price:   {price}");
    }

    if let Some(balance) = transaction.buyer_last_saldo {
        println!("balance: {balance}");
    }
}
