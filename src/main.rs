use std::env;
use serde_json;
use serde_bencode;
use serde::Deserialize;

fn bencode_to_serde(value: serde_bencode::value::Value) -> serde_json::Value {
    match value {
        serde_bencode::value::Value::Bytes(bytes) => {
            serde_json::Value::String(String::from_utf8_lossy(bytes.as_slice()).to_string())
        },
        serde_bencode::value::Value::Int(int) => {
            serde_json::Value::Number(serde_json::value::Number::from(int))
        },
        serde_bencode::value::Value::List(list) => {
            serde_json::Value::Array(list.into_iter().map(|el| bencode_to_serde(el)).collect())
        },
        serde_bencode::value::Value::Dict(dict) => {
            serde_json::Value::Object(dict.into_iter().map(|el|
                (String::from_utf8_lossy(el.0.as_slice()).to_string(), bencode_to_serde(el.1))).collect())
        }
    }
}


#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    let value = serde_bencode::from_str(encoded_value).unwrap();
    bencode_to_serde(value)
}

#[derive(Debug, Deserialize)]
struct MetaInfo {
    announce: Vec<u8>,
    info: Info
}

#[derive(Debug, Deserialize)]
struct Info {
    length: i64,
    name: Vec<u8>,
    piece: i64,
    pieces: serde_bytes::ByteBuf
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];
    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value);
    } else if command == "info" {
        let file_path =  &args[2];
        let buf = std::fs::read(file_path).unwrap();
        match serde_bencode::de::from_bytes::<MetaInfo>(&buf) {
            Ok(torrent) => {
                println!("Tracker URL: {}", String::from_utf8_lossy(torrent.announce.as_slice()));
                println!("Length: {}", torrent.info.length);
            },
            Err(e) => {
                println!("Error: {}", e.to_string());
            }
        }
    }
    else {
        println!("unknown command: {}", args[1])
    }
}
