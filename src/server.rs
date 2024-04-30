use std::fmt;
use std::str::FromStr;
use influxdb::{Client, Error, InfluxDbWriteable, ReadQuery};
use axum::{
    routing::{get, post},
    http::StatusCode,
    Json,
    Router,
    extract::Query
};
use serde::{de, Deserialize, Deserializer};


#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Params {
    foo: Option<i32>,
    bar: Option<String>,
}

#[tokio::main]
async fn start_server() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app()).await.unwrap();
}

fn app() -> Router {
    Router::new().route("/cmc/:id", get(cmc_price))
}


async fn cmc_price(Query(params): Query<Params>) -> String {
    format!("{params:?}")
}

fn main() {

}
