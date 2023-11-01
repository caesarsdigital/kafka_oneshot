use clap::Parser;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json::Value;
use std::io::{self, BufRead};
use std::time::Duration;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, default_value = "localhost:9092")]
    server: String,
    #[arg(long)]
    key: String,
    #[arg(long)]
    topic: String,
}

#[tokio::main]
async fn main() {
    let opts: Cli = Cli::parse();

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &opts.server)
        .create()
        .expect("Producer creation error");

    let stdin = io::stdin();
    let lines: Vec<_> = stdin.lock().lines().collect::<Result<_, _>>().unwrap();

    for line in lines {
        if let Some(key) = extract_key_from_json(&line, &opts.key) {
            let _ = producer
                .send(
                    FutureRecord::to(&opts.topic).payload(&line).key(&key),
                    Duration::from_secs(10),
                )
                .await
                .expect("Failed to send message to Kafka");
        }
    }
}

fn extract_key_from_json(json_str: &str, pointer: &str) -> Option<String> {
    let json: Value = serde_json::from_str(json_str).ok()?;
    json_pointer_to_value(&json, pointer)
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn json_pointer_to_value<'a>(json: &'a Value, pointer: &str) -> Option<&'a Value> {
    pointer.split('.').try_fold(json, |acc, part| match acc {
        Value::Object(map) => map.get(part),
        _ => None,
    })
}
