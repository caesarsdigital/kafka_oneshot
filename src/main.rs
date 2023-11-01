use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::io::{self, BufRead};
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <kafka_topic>", args[0]);
        std::process::exit(1);
    }
    let topic = &args[1];

    // Set up Kafka producer
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .create()
        .expect("Producer creation error");

    // Read JSON messages from STDIN and publish to Kafka
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let message = line.expect("Failed to read line from STDIN");
        let _ = producer
            .send(
                FutureRecord::to(topic)
                    .payload(&message)
                    .key(""),
                Some(Duration::from_secs(10)), // 10 seconds timeout
            )
            .await
            .expect("Failed to send message to Kafka");
    }
}
