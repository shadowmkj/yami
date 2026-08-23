//! Benchmark example demonstrating high-throughput zero-copy YAML parsing in `yami`.
//!
//! Run with: `cargo run --release --example benchmark`

use std::time::Instant;
use yami::parse;

fn main() {
    let yaml_payload = r#"
# Application configuration benchmark payload
service:
  name: yami-microservice
  version: "2.4.0"
  active: true
  replicas: 8
  server:
    host: 0.0.0.0
    port: 8080
    ssl:
      enabled: true
      cert_path: /etc/ssl/certs/service.crt
      key_path: /etc/ssl/private/service.key
  database:
    primary:
      host: db-primary.internal
      port: 5432
      pool_size: 20
    replicas:
      - host: db-replica-1.internal
        port: 5432
      - host: db-replica-2.internal
        port: 5432
  features: [tracing, metrics, rate_limiting, auth_v2]
  rate_limits: { requests_per_second: 5000, burst: 10000 }
"#;

    println!("============================================================");
    println!("  yami Zero-Copy YAML Parser Benchmark");
    println!("============================================================");
    println!("Payload size: {} bytes", yaml_payload.len());

    // Warmup
    for _ in 0..1_000 {
        let _ = parse(yaml_payload).expect("payload must be valid");
    }

    let iterations = 100_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let doc = parse(yaml_payload).expect("payload must be valid");
        // Access nested properties to ensure full parsing
        let _name = doc["service"]["name"].as_scalar();
        let _port = doc["service"]["server"]["port"].as_scalar();
    }

    let elapsed = start.elapsed();
    let nanos_per_op = elapsed.as_nanos() as f64 / iterations as f64;
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
    let mb_per_sec = (yaml_payload.len() as f64 * iterations as f64)
        / (1024.0 * 1024.0 * elapsed.as_secs_f64());

    println!("Iterations:       {}", iterations);
    println!("Total time:       {:.2?}", elapsed);
    println!("Time per parse:   {:.2} ns ({:.2} µs)", nanos_per_op, nanos_per_op / 1000.0);
    println!("Throughput:       {:.0} parses/sec", ops_per_sec);
    println!("Bandwidth:        {:.2} MB/sec", mb_per_sec);
    println!("============================================================");
}
