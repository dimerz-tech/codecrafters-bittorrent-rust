use serde_json;
use std::env;
use serde_json::{Number, Value};
use serde_bencode;


#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    serde_bencode::de::from_bytes(encoded_value.as_ref()).unwrap()
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
