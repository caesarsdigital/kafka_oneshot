# kafka_oneshot


## Get it
Build it
```
cargo build --release
```

Download a binary from the releases page.

## Usage

First, check the updated usage info by running `kafka_oneshot -h`.

Press `Ctrl`+`C` to quit.

### Publish JSON messages to Kafka from stdin

```
$ echo '{"header": {"id": "12345"}, "data": "sample"}' | ./target/release/kafka_oneshot --server localhost:9092 --key header.id --topic kafka_topic_name
```

- Multi-line JSON messages are supported.
- Multiple JSON messages can be sent - just keep pasting or piping the messages to `kafka_oneshot` once it is running.

Currently it is assumed the key is part of the JSON message.
Please raise an issue if you need support for a different keying strategy.

### SSL/TLS

Create an env file with your SSL credentials:
```
KAFKA_SSL_CLIENT_CERT=/path/to/client.pem
KAFKA_SSL_CLIENT_KEY=/path/to/client.key
KAFKA_SSL_CERT_AUTHORITY=/path/to/ca.pem
```

Then pass it with `--ssl-env-file`:
```
kafka_oneshot --server broker:9093 --topic my-topic --ssl-env-file ssl.env
```

`KAFKA_SSL_CERT_AUTHORITY` is optional (uses system CAs if omitted).

### Consuming messages from Kafka and writing to stdout

Use `--mode consumer`, for example:

```
kafka_oneshot --mode consumer --server localhost:9092 --key header.id --topic kafka_topic_name
```

### Producing and Consuming simultaneously

This mode may be useful for seeing if a topic is available and functioning correclty. Use `--mode both`

```
kafka_oneshot --mode both --server localhost:9092 --key header.id --topic kafka_topic_name
```

You'll paste in messages to publish, and see the messages that are consumed along with their key.