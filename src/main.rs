use serde_json;
use std::env;
use serde::Serialize;
use serde_bencode;


#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_bencode::value::Value {
    serde_bencode::from_str(encoded_value).unwrap()
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];
    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}",  decoded_value.try_into().unwrap());
    } else {
        println!("unknown command: {}", args[1])
    }
}
