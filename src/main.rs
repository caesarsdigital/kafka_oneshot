#![deny(warnings)]
#![allow(dead_code)]

use clap::Parser;
use do_notation::*;
use futures::StreamExt;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::Consumer;
use rdkafka::error::KafkaResult;
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

macro_rules! create_str_consts {
    ($($name:ident), *) => {
        $(
            const $name: &str = stringify!($name);
        )*
    };
}

create_str_consts!(
    KAFKA_SSL_CLIENT_CERT,
    KAFKA_SSL_CLIENT_KEY,
    KAFKA_SSL_CERT_AUTHORITY
);

use openssl::ssl::{SslConnector, SslFiletype, SslMethod, SslVerifyMode};

#[derive(clap::ValueEnum, Clone)]
enum Mode {
    Producer,
    Consumer,
    Both,
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

struct SslConfig {
    client_cert: String,
    client_key: String,
    cert_authority: Option<String>,
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

fn read_env_file(file_path: &str) -> io::Result<HashMap<String, String>> {
    let file = File::open(file_path)?;
    let reader = io::BufReader::new(file);
    let mut env_map = HashMap::new();
    reader.lines().try_for_each(|line_res| {
        let line = line_res?;
        if let Some((key, value)) = line.split_once('=') {
            env_map.insert(key.trim().to_string(), value.trim().to_string());
        }
        io::Result::Ok(())
    })?;
    Ok(env_map)
}

fn extract_ssl_config(env_map: HashMap<String, String>) -> Option<SslConfig> {
    m! {
        client_cert <- env_map.get(&KAFKA_SSL_CLIENT_CERT.to_string()).cloned();
        client_key <- env_map.get(&KAFKA_SSL_CLIENT_KEY.to_string()).cloned();
        let cert_authority = env_map.get(&KAFKA_SSL_CERT_AUTHORITY.to_string()).cloned();
        Some(SslConfig { client_cert, client_key, cert_authority })
    }
}

fn build_ssl_connector(ssl_config: SslConfig) -> SslConnector {
    // ~ OpenSSL offers a variety of complex configurations. Here is an example:
    let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
    builder.set_cipher_list("DEFAULT").unwrap();
    builder.set_verify(SslVerifyMode::PEER);

    // #[cfg(feature = "disable_this")]
    // info!(
    //     "loading cert-file={}, key-file={}",
    //     ssl_config.client_cert, ssl_config.client_key
    // );

    // TODO: detect filetypes?
    builder
        .set_certificate_file(ssl_config.client_cert, SslFiletype::PEM)
        .unwrap();
    builder
        .set_private_key_file(ssl_config.client_key, SslFiletype::PEM)
        .unwrap();
    builder.check_private_key().unwrap();

    if let Some(ca_cert) = ssl_config.cert_authority {
        // info!("loading ca-file={}", ca_cert);

        builder.set_ca_file(ca_cert).unwrap();
    } else {
        // ~ allow client specify the CAs through the default paths:
        // "These locations are read from the SSL_CERT_FILE and
        // SSL_CERT_DIR environment variables if present, or defaults
        // specified at OpenSSL build time otherwise."
        builder.set_default_verify_paths().unwrap();
    }

    //let connector = builder.build();
    builder.build()

    // ~ instantiate KafkaClient with the previous OpenSSL setup
    // let mut client = KafkaClient::new_secure(
    //     cfg.brokers,
    //     SecurityConfig::new(connector).with_hostname_verification(cfg.verify_hostname),
    // );
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
                        let producer_lock = producer_clone.lock().await;
                        let _ = producer_lock
                            .send(
                                FutureRecord::to(&topic).payload(&buffer).key(&key),
                                Duration::from_secs(10),
                            )
                            // This produce is effectively synchronous now
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

    let consumer: rdkafka::consumer::StreamConsumer = ClientConfig::new()
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
                            msg.key().map(String::from_utf8_lossy),
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
        Mode::Producer => run_producer(opts).await.unwrap(),
        Mode::Consumer => run_consumer(opts).await.unwrap(),
        Mode::Both => {
            tokio::try_join!(run_producer(opts.clone()), run_consumer(opts.clone())).unwrap();
        }
    }
}
