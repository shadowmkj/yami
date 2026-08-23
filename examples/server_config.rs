//! A practical example demonstrating zero-copy YAML configuration parsing with `yami`.
//!
//! Run with: `cargo run --example server_config`

use yami::{Yaml, parse};

/// Application server configuration borrowing zero-copy string slices
/// directly from the source YAML input without any string allocations.
#[derive(Debug, PartialEq)]
pub struct ServerConfig<'a> {
    pub name: &'a str,
    pub host: &'a str,
    pub port: i64,
    pub debug: bool,
    pub database_urls: Vec<&'a str>,
    pub features: Vec<&'a str>,
}

impl<'a> ServerConfig<'a> {
    /// Constructs a `ServerConfig` by extracting typed values from a parsed `Yaml<'a>` AST.
    pub fn from_yaml(doc: &Yaml<'a>) -> Result<Self, Box<dyn std::error::Error>> {
        // Direct indexing allows intuitive map navigation
        let name = doc["service"]["name"]
            .as_scalar()
            .ok_or("missing service.name")?;

        let host = doc["service"]["host"]
            .as_scalar()
            .ok_or("missing service.host")?;

        let port = doc["service"]["port"].to_i64()?;
        let debug = doc["service"]["debug"].to_bool()?;

        // Sequences can be iterated and mapped with zero string cloning
        let db_seq = doc["database"]["replicas"]
            .as_sequence()
            .ok_or("missing database.replicas")?;

        let database_urls = db_seq.iter().filter_map(|item| item.as_scalar()).collect();

        // Flow-style sequence parsed seamlessly
        let feature_seq = doc["features"]
            .as_sequence()
            .ok_or("missing features list")?;

        let features = feature_seq
            .iter()
            .filter_map(|item| item.as_scalar())
            .collect();

        Ok(Self {
            name,
            host,
            port,
            debug,
            database_urls,
            features,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml_text = r#"
# Microservice Configuration
service:
  name: auth-service
  host: 0.0.0.0
  port: 8080
  debug: true

database:
  primary: postgres://master.db.internal:5432/auth
  replicas:
    - postgres://replica-1.db.internal:5432/auth
    - postgres://replica-2.db.internal:5432/auth

# Flow-style syntax
features: [jwt_auth, rate_limiting, prometheus_metrics]
"#;

    println!("--- Parsing YAML with yami ---");
    let doc = parse(yaml_text)?;

    println!("Raw parsed AST:\n{}\n", doc);

    println!("--- Extracting into Typed Struct (Zero-Copy) ---");
    let config = ServerConfig::from_yaml(&doc)?;

    println!("Service Name:   {}", config.name);
    println!("Bind Address:   {}:{}", config.host, config.port);
    println!("Debug Mode:     {}", config.debug);
    println!("DB Replicas:    {:?}", config.database_urls);
    println!("Features:       {:?}", config.features);

    println!("\nSuccessfully loaded configuration with zero string heap allocations!");
    Ok(())
}
