use chrono::{DateTime, Local, Utc};
use colorism::{foreground::Fore, util::RESET};
use daemonize::Daemonize;
use influxdb::{Client, Error, InfluxDbWriteable, ReadQuery};
use rand::Rng;
use reqwest;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tokio::time::{sleep, Duration};

fn main() -> Result<(), failure::Error> {
    let file_path_output = format!("{}/.cmc_influx/output.log", env::var("HOME").unwrap());
    let file_path_error = format!("{}/.cmc_influx/error.log", env::var("HOME").unwrap());
    let file_path_pid = format!("{}/.cmc_influx/pid", env::var("HOME").unwrap());
    let cmc_password = env::var("CMC_API_KEY").expect("CMC_API_KEY not set");
    let cwd_path = format!("{}/.cmc_influx", env::var("HOME").unwrap());

    let stdout = File::create(file_path_output).unwrap();
    let stderr = File::create(file_path_error).unwrap();
    let daemonize = Daemonize::new()
        .stderr(stderr)
        .stdout(stdout)
        .working_directory(cwd_path)
        .chown_pid_file(true)
        .pid_file(file_path_pid);

    match daemonize.start() {
        Ok(_) => {
            println!("Success, daemonized");
            fetch_main(cmc_password.as_str());
            Ok(())
        }
        Err(_) => todo!(),
    }
}

#[tokio::main]
pub async fn fetch_main(cmc_api_key: &str) -> Result<(), Error> {
    loop {
        let local: DateTime<Local> = Local::now();
        let timestamp = format!(
            "{} {} {}",
            Fore::color(Fore::BdGreen),
            local.to_string(),
            RESET
        );
        println!(
            "[{}] Waking up to get data and persist to InfluxDB",
            timestamp
        );
        let listing = CryptoListing::fetch_web(&cmc_api_key);
        listing
            .await
            .persist_db()
            .await
            .expect("TODO: panic message");
        sleep(Duration::from_millis(60000)).await;
    }
}

async fn read_data(series: &str) -> Result<(), Error> {
    let client = Client::new("http://localhost:8086", "test");
    let query = format!("SELECT * FROM {}", series);
    let read_query = ReadQuery::new(query);
    let read_result = client.query(read_query).await?;
    Ok(())
}

#[derive(Deserialize, Debug, Clone)]
pub struct CryptoData {
    id: i32,
    name: String,
    symbol: String,
    slug: String,
    num_market_pairs: i32,
    date_added: String,
    tags: Vec<String>,
    max_supply: Option<f64>,
    circulating_supply: Option<f64>,
    total_supply: Option<f64>,
    infinite_supply: Option<bool>,
    platform: Option<CryptoPlatform>,
    cmc_rank: i32,
    self_reported_circulating_supply: Option<f64>,
    self_reported_market_cap: Option<f64>,
    tvl_ratio: Option<f64>,
    last_updated: String,
    quote: CryptoQuote,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CryptoPlatform {}

#[derive(Deserialize, Debug, Clone)]
pub struct CurrencyQuote {
    price: f64,
    volume_24h: f64,
    volume_change_24h: f64,
    percent_change_1h: f64,
    percent_change_24h: f64,
    percent_change_7d: f64,
    percent_change_30d: f64,
    percent_change_60d: f64,
    percent_change_90d: f64,
    market_cap: f64,
    market_cap_dominance: f64,
    fully_diluted_market_cap: f64,
    tvl: Option<f64>,
    last_updated: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CryptoQuote {
    USD: CurrencyQuote,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CryptoListingStatus {
    timestamp: String,
    error_code: i32,
    error_message: Option<String>,
    elapsed: i32,
    credit_count: i32,
    notice: Option<String>,
    total_count: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CryptoListing {
    status: CryptoListingStatus,
    data: Vec<CryptoData>,
}

impl CryptoListing {
    pub async fn fetch(file: &str) -> CryptoListing {
        let mut fi = File::open(file).unwrap();
        let mut data = String::new();
        fi.read_to_string(&mut data).unwrap();
        let cryptolisting: CryptoListing =
            serde_json::from_str(&data).expect("JSON was not well-formatted");
        cryptolisting
    }

    pub async fn fetch_web(api_key: &str) -> CryptoListing {
        let client = reqwest::Client::new();
        let cmc_url = "https://pro-api.coinmarketcap.com/v1/cryptocurrency/listings/latest?start=1&limit=5000&convert=USD";
        let res = client
            .get(cmc_url)
            .header("X-CMC_PRO_API_KEY", api_key)
            .send()
            .await
            .expect("failed to get response")
            .text()
            .await
            .expect("failed to get payload");
        let cryptolisting: CryptoListing =
            serde_json::from_str(&res).expect("JSON was not well-formatted");
        cryptolisting
        //let cryptolisting: CryptoListing = serde_json::from_str(res.unwrap()).expect("JSON was not well-formatted");
        //cryptolisting
    }

    pub async fn persist_db(&self) -> Result<(), Error> {
        for data in self.data.iter() {
            let cmc_price = CmcPrice {
                time: chrono::offset::Utc::now(),
                price: data.quote.USD.price,
                volume: data.quote.USD.volume_24h,
                market_cap: data.quote.USD.market_cap,
                hour_change: data.quote.USD.percent_change_1h,
                day_change: data.quote.USD.percent_change_24h,
                week_change: data.quote.USD.percent_change_7d,
                month_change: data.quote.USD.percent_change_30d,
                quarter_change: data.quote.USD.percent_change_90d,
            };
            cmc_price
                .insert_to_db(&data.symbol.to_string().to_lowercase())
                .await
                .expect("TODO: panic message");
        }

        Ok(())
    }
}

#[derive(InfluxDbWriteable, Debug, Clone, Copy)]
struct CmcPrice {
    time: DateTime<Utc>,
    day_change: f64,
    hour_change: f64,
    week_change: f64,
    month_change: f64,
    market_cap: f64,
    price: f64,
    volume: f64,
    quarter_change: f64,
}

impl CmcPrice {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        CmcPrice {
            time: chrono::offset::Utc::now(),
            day_change: rng.gen(),
            hour_change: rng.gen(),
            week_change: rng.gen(),
            month_change: rng.gen(),
            market_cap: rng.gen(),
            price: rng.gen(),
            volume: rng.gen(),
            quarter_change: rng.gen(),
        }
    }

    pub async fn insert_to_db(&self, ticker: &str) -> Result<(), Error> {
        let cmc_prices = vec![self.into_query(ticker)];
        let client = Client::new("http://localhost:8086", "cmc_prices");
        client.query(cmc_prices).await?;
        Ok(())
    }
}
