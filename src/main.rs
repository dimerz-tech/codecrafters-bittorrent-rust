use serde_json;
use std::env;
use serde::Serialize;
use serde_bencode;


#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    let ben_val: serde_bencode::value::Value = serde_bencode::from_str(encoded_value).unwrap();
    println!("Ben Val {:?}", ben_val);
    let x = ben_val.serialize(serde_json::value::Serializer).unwrap();
    println!("X: {}", x);
    x
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
