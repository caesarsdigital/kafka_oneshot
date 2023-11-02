#![deny(warnings)]

use clap::Parser;
use futures::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json::Value;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
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

    let producer: Arc<Mutex<FutureProducer>> = Arc::new(Mutex::new(
        ClientConfig::new()
            .set("bootstrap.servers", &opts.server)
            .create()
            .expect("Producer creation error"),
    ));

    let stdin = io::stdin();
    let reader = stdin.lock();

    let _ = futures::stream::iter(reader.lines())
        .map(|line| line.unwrap())
        .flat_map(|line| futures::stream::iter(line.chars().collect::<Vec<char>>()))
        .fold((String::new(), 0), move |(mut buffer, brace_count), ch| {
            let key_path = opts.key.clone();
            let producer_clone = producer.clone();
            let topic = opts.topic.clone();
            async move {
                buffer.push(ch);
                let brace_count = match ch {
                    '{' => brace_count + 1,
                    '}' => brace_count - 1,
                    _ => brace_count,
                };

                if brace_count == 0 && !buffer.trim().is_empty() {
                    if let Some(key) = extract_key_from_json(&buffer, &key_path) {
                        let producer_lock = producer_clone.lock().unwrap();
                        let _ = producer_lock
                            .send(
                                FutureRecord::to(&topic).payload(&buffer).key(&key),
                                Duration::from_secs(10),
                            )
                            .await
                            .expect("Failed to send message to Kafka");
                    }
                    buffer.clear();
                }

                (buffer, brace_count)
            }
        })
        .await;
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
