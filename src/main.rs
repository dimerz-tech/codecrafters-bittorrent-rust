use std::collections::HashMap;
use std::env;
use serde_json;
use serde_bencode;
use serde::{Deserialize, Serialize};
use sha1::{Sha1, Digest, Sha1Core};
use sha1::digest::Output;
use serde_urlencoded;
use hex;

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
    announce: String,
    info: Info
}

#[derive(Debug, Deserialize, Serialize)]
struct Info {
    length: usize,
    name: String,
    #[serde(rename = "piece length")]
    piece_length: usize,
    pieces: serde_bytes::ByteBuf
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub complete: usize,
    pub incomplete: usize,
    pub interval: usize,
    #[serde(rename = "min interval")]
    pub min_interval: usize,
    #[serde(with = "serde_bytes")]
    pub peers: Vec<Peer>,
}

#[derive(Debug, Deserialize)]
struct Peer {
    ip: [u8; 4],
    port: [u8; 2]
}

struct Torrent {
    meta: MetaInfo,
    hash: [u8; 20]
}

impl Torrent {
    pub fn new(file_path: &String) -> Self {
        let buf = std::fs::read(file_path).unwrap();
        let mut hasher = Sha1::new();
        let meta =  serde_bencode::de::from_bytes::<MetaInfo>(&buf).unwrap();
        let bytes = serde_bencode::to_bytes(&meta.info).unwrap();
        hasher.update(bytes);
        let hash: [u8; 20] = hasher.finalize().try_into().unwrap();
        Torrent { meta, hash }
    }
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];
    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value);
    } else if command == "info" {
        let file_path =  &args[2];
        let torrent = Torrent::new(file_path);
        println!("Tracker URL: {}", torrent.meta.announce);
        println!("Length: {}", torrent.meta.info.length);
        println!("Info Hash: {}", hex::encode(torrent.hash));
        println!("Piece Length: {}", torrent.meta.info.piece_length);
        let chunks: Vec<&[u8]> = torrent.meta.info.pieces.as_ref().chunks(20).collect();
        for chunk in chunks {
            println!("{}", hex::encode(chunk));
        }
    } else if command == "peers" {
        let file_path =  &args[2];
        let torrent = Torrent::new(file_path);
        let peer_id = "00112233445566778899";
        let port = 6881;
        let uploaded = 0;
        let downloaded = 0;
        let left = torrent.meta.info.length;
        let compact = 1;
        let info_hash :String = hex::encode(torrent.hash).chars().
            collect::<Vec<char>>().chunks(2).fold(String::new(), |acc, el| acc + "%" + &*el.iter().collect::<String>());
        let url = format!("{}?info_hash={}&peer_id={peer_id}&port={port}&\
        uploaded={uploaded}&downloaded={downloaded}&left={left}&compact={compact}", torrent.meta.announce, info_hash);
        println!("URL: {}", url);
        let res = reqwest::get(url).await.unwrap();
        let resp: Response = serde_bencode::from_bytes(res.bytes().await.unwrap().as_ref()).unwrap();
        println!("Response {:?}", resp);
    }
    else {
        println!("unknown command: {}", args[1])
    }
}