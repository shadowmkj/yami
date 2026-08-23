use insta::assert_debug_snapshot;
use yami::parse;

#[test]
fn test_snapshot_docker_compose() {
    let yaml = r#"
version: "3.8"
services:
  web:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    environment:
      NODE_ENV: production
      PORT: 80
    volumes:
      - ./data:/var/www/html
      - ./config/nginx.conf:/etc/nginx/nginx.conf
"#;

    let doc = parse(yaml).expect("docker-compose should parse");
    assert_debug_snapshot!(doc);
}

#[test]
fn test_snapshot_k8s_deployment() {
    let yaml = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nginx-deployment
  labels:
    app: nginx
spec:
  replicas: 3
  selector:
    matchLabels:
      app: nginx
  template:
    metadata:
      labels:
        app: nginx
    spec:
      containers:
        - name: nginx
          image: nginx:1.14.2
          ports:
            - containerPort: 80
"#;

    let doc = parse(yaml).expect("k8s deployment should parse");
    assert_debug_snapshot!(doc);
}

#[test]
fn test_snapshot_mixed_flow_and_block() {
    let yaml = r#"
package:
  name: yami
  keywords: [yaml, parser, zero-copy, fast]
  metadata:
    authors: ['Rust Developer <dev@example.com>']
    links: { repo: "https://github.com/example/yami", docs: "https://docs.rs/yami" }
"#;

    let doc = parse(yaml).expect("mixed yaml should parse");
    assert_debug_snapshot!(doc);
}

#[test]
fn test_snapshot_syntax_error_formatting() {
    let yaml_with_tab = "server:\n\thost: localhost";
    let err = parse(yaml_with_tab).expect_err("tabs should fail");
    assert_debug_snapshot!(err);
}
