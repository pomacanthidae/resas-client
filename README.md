# RESAS Client
Client to interact with [RESAS-API](https://opendata.resas-portal.go.jp/docs/api/v1/index.html)

## Usage 
```rust
let token = "<Your token>";
let clinet = client::Client::new(
    String::from(token.to_String()),
    client::RetryPolicy::default(),
);
let prefectures = client
    .get::<schema::Prefecture>("api/v1/prefectures", None, true)
    .unwrap();
```