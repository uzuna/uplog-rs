use log::{debug, error};
use serde_cbor::to_vec;
use tungstenite::{client::connect, Message};

fn main() {
    env_logger::init();
    uplog::init!();
    let addr = "localhost:9001";
    let url = format!("ws://{}/", addr);
    let (mut client, _) = connect(&url).unwrap();

    for i in 0..5 {
        let record = uplog::devlog!(uplog::Level::Info, "uplog_server.bin.client", "send");
        let buf = to_vec(&record).unwrap();
        client
            .write_message(Message::binary(buf.as_slice()))
            .map_err(|e| error!("failed to send at: {}, {} ", i, e))
            .ok();
        debug!("send {}", i);
    }
}
