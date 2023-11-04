# kafka_oneshot

Build it
```
cargo build --release
```

Or, download a binary from the releases page.


Publish JSON messages to Kafka from stdin:

```
$ echo '{"header": {"id": "12345"}, "data": "sample"}' | ./target/release/kafka_oneshot --server localhost:9092 --key header.id --topic kafka_topic_name
```

- Multi-line JSON messages are supported.
- Multiple JSON messages can be sent - just keep pasting or piping the messages to `kafka_oneshot` once it is running.
