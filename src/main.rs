use futures_util::{TryStreamExt};
use tokio_util::io::StreamReader;
use tokio::io::AsyncBufReadExt;
use serde::{Deserialize, Serialize, Deserializer, de};
use chrono::{DateTime, Utc, TimeZone};
use clap::{Arg, Command};
use std::str::FromStr;
use std::fmt::Display;
use std::io::{ErrorKind, Write};
use std::fs::File;
use std::fs::OpenOptions;
use std::borrow::Borrow;
use log::{info, error};
use env_logger::Env;

const DEFAULT_CONFIG: &str = "config.toml";

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    hostname: String,
    token: String,
    account: String,
    instruments: Vec<String>,
    cooldown: u64,
    output_dir: String,
    output_filename: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum StreamLine {
    HEARTBEAT {
        #[serde(deserialize_with = "from_str_time")]
        time: DateTime<Utc>
    },
    PRICE {
        #[serde(deserialize_with = "from_str_time")]
        time: DateTime<Utc>,
        #[serde(deserialize_with = "from_str")]
        #[serde(rename(deserialize = "closeoutBid"))]
        closeout_bid: f64,
        #[serde(deserialize_with = "from_str")]
        #[serde(rename(deserialize = "closeoutAsk"))]
        closeout_ask: f64,
        status: String,
        tradeable: bool,
        instrument: String
    },
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {

    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config_file = get_config_file();

    //std::env::current_dir().unwrap().to_str().unwrap()
    info!("Loading config: {}", std::fs::canonicalize(config_file.clone()).unwrap_or(DEFAULT_CONFIG.parse().unwrap()).to_str().unwrap_or(DEFAULT_CONFIG));

    let config = read_config(config_file);

    match config {
        Ok(c) => {
            info!("Loaded config: {:?}", c);

            info!("Starting recorder loop");

            loop {
                if let Err(e) = record_stream(c.borrow()).await {
                    error!("{}", e);

                    info!("Awaiting cooldown");

                    tokio::time::sleep(std::time::Duration::from_secs(c.cooldown)).await;
                }

                info!("Reconnecting...");
            }
        }
        Err(e) => {
            error!("Error: {}", e.to_string());
            error!("Please create a config file according to this template:\n{}", toml::to_string(Config {
                hostname: "stream-fxtrade.oanda.com".to_string(),
                token: "<your token>".to_string(),
                account: "<your account>".to_string(),
                instruments: vec![String::from("EUR_USD"), String::from("USD_JPY")],
                cooldown: 1,
                output_dir: "data".to_string(),
                output_filename: "oanda_stream_%Y_%m_%d.json".to_string()
            }.borrow()).unwrap_or("crap".to_string()));

            //return Ok(())
            return Err(e);
        }
    }

}

fn get_config_file() ->  String {
    let matches = Command::new("oanda-stream-recorder")
        .version("0.1.0")
        .author("Ivan Ganev <iganev@cytec.bg>")
        .about("oanda-stream-recorder")
        .arg(Arg::new("config")
            .short("c".parse().unwrap())
            .long("config")
            .takes_value(true)
            .help(DEFAULT_CONFIG))
        .get_matches();

    matches.value_of("config").unwrap_or(DEFAULT_CONFIG).to_string()
}

fn read_config(filepath: String) -> std::io::Result<Config> {
    let content = std::fs::read_to_string(filepath)?;
    Ok(toml::from_str(&content)?)
}

async fn record_stream(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::ClientBuilder::new().build()?;

    let url = format!("https://{}//v3/accounts/{}/pricing/stream?instruments={}", config.hostname.as_str(), config.account.as_str(), config.instruments.join("%2C"));

    let stream = client.get(url)
        .header(reqwest::header::AUTHORIZATION, "Bearer ".to_owned() + config.token.as_str())
        .send()
        .await?
        .bytes_stream();

    let reader = StreamReader::new(stream.map_err(convert_err));

    let mut lines = reader.lines();

    let mut current_filename = config.output_dir.as_str().to_owned() + std::path::MAIN_SEPARATOR.to_string().as_str() + Utc::now().format(config.output_filename.as_str()).to_string().as_str();

    info!("Current file name: {}", current_filename);

    let mut file = get_file(current_filename.clone())?;

    while let Some(item) = lines.next_line().await? {

        let test_filename = config.output_dir.as_str().to_owned() + std::path::MAIN_SEPARATOR.to_string().as_str() + Utc::now().format(config.output_filename.as_str()).to_string().as_str();

        if !test_filename.eq(current_filename.clone().as_str()) {
            file = get_file(test_filename.clone())?;

            tokio::spawn( tokio::process::Command::new("gzip").arg(current_filename.clone()).output());

            current_filename = test_filename;
            info!("Current file name: {}", current_filename);
        }

        let sl: StreamLine = serde_json::from_str(item.as_str())?;

        if let StreamLine::PRICE {..} = sl {
            file.write_all(serde_json::to_string(&sl)?.as_bytes())?;
            file.write_all(b"\n")?;
        }
        else if let StreamLine::HEARTBEAT {..} = sl {
            //debug!("Chunk: {:?}", sl);
        }

    }

    Ok(())
}

fn get_file(current_filename: String) -> std::io::Result<File> {
    let file;

    if std::fs::metadata(current_filename.clone()).is_ok() {
        file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(current_filename.as_str())?;
    }
    else {
        file = File::create(current_filename.as_str())?;
    }

    Ok(file)
}

fn convert_err(err: reqwest::Error) -> std::io::Error {
    error!("{}", err);
    std::io::Error::new(ErrorKind::Other, err.to_string().as_str())
}

fn from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where T: FromStr,
          T::Err: Display,
          D: Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(de::Error::custom)
}

fn from_str_time<'de, D>(deserializer: D, ) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Utc.datetime_from_str(&s, "%+").map_err(serde::de::Error::custom)
}