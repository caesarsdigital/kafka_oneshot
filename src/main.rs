#![deny(warnings)]

use clap::Parser;
use futures::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json::Value;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(clap::ValueEnum, Clone)]
enum Mode {
    Producer,
    Consumer,
    Both,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Producer
    }
}

#[derive(Clone, Parser)]
#[command(author, version, about = "A simple pub/sub tool for Kafka", long_about = None)]
struct Cli {
    #[arg(long, default_value = "localhost:9092")]
    server: String,
    #[arg(value_enum, long, default_value_t = Mode::Producer)]
    mode: Mode,
    // The key field is only required if the mode includes Producer
    #[arg(long, requires_ifs = [("Producer", "mode"), ("Both", "mode")])]
    key: Option<String>,
    #[arg(long)]
    topic: String,
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

async fn run_producer(opts: Cli) -> KafkaResult<()> {
    let producer: Arc<Mutex<FutureProducer>> = Arc::new(Mutex::new(
        ClientConfig::new()
            .set("bootstrap.servers", &opts.server)
            .create()
            .expect("Producer creation error"),
    ));

    let stdin = io::stdin();
    let reader = stdin.lock();

    futures::stream::iter(reader.lines())
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
                    if let Some(key) =
                        extract_key_from_json(&buffer, &key_path.unwrap_or("id".to_string()))
                    {
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
    Ok(())
}

async fn run_consumer(opts: Cli) -> KafkaResult<()> {
    // Set up the Kafka consumer configuration
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "kafka_oneshot")
        .set("bootstrap.servers", &opts.server)
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()?;

    // Subscribe to the topic(s)
    consumer.subscribe(&[&opts.topic])?;

    // Create a stream to process messages
    let message_stream = consumer.stream();

    // Use `stream::for_each` to iterate over the messages asynchronously
    message_stream
        .for_each(|message| async {
            match message {
                Ok(msg) => {
                    if let Some(payload) = msg.payload() {
                        // Assuming the payload is a string, print it out
                        println!(
                            "Key: '{:?}', Payload: '{}'",
                            msg.key(),
                            String::from_utf8_lossy(payload)
                        );
                    }
                }
                Err(e) => eprintln!("Kafka consumer error: {}", e),
            }
        })
        .await;

    Ok(())
}

#[tokio::main]
async fn main() {
    let opts: Cli = Cli::parse();

    match opts.mode {
        Mode::Producer => {
            let _ = run_producer(opts).await.unwrap();
        }
        Mode::Consumer => {
            let _ = run_consumer(opts).await.unwrap();
        }
        Mode::Both => {
            tokio::try_join!(run_producer(opts.clone()), run_consumer(opts.clone())).unwrap();
        }
    }
}
