use std::{
    net::{TcpListener, ToSocketAddrs},
    sync::mpsc::channel,
    thread::{self, JoinHandle},
};

use tungstenite::{accept, Message};
use uplog::{debug, error, info, trace, warn, Record};

#[cfg_attr(lib_build, test)]
fn main() {
    base();
    client();
}

fn base() {
    uplog::session_init();
    info!("test.base", "hello");
    let _ = warn!("test.base", "hello");
    debug!("test.base", "hello", "cats", "meow");
    trace!("test.base", "hello", "cats", "meow", "nekomimi", true);
}

fn client() {
    let addr = format!("localhost:{}", 9004);
    let handle = ws_server(addr);

    let builder = uplog::Builder::default().port(9004);
    uplog::try_init_with_builder(builder).unwrap();
    trace!("test.base", "hello", "cats", "meow", "nekomimi", true);
    debug!("test.base", "hello", "cats", "meow");
    info!("test.base", "hello", "cat", "mii");
    let _ = warn!("test.base", "hello", "cat", "aooo");
    error!("test.base", "hello", "cat", "grrr");
    uplog::flush();

    let result = handle.join().unwrap();
    let iter = serde_cbor::Deserializer::from_slice(&result).into_iter::<Record>();

    let mut counter = 0;
    for v in iter {
        let v = v.unwrap();
        assert_eq!(v.category.as_str(), "test.base");
        assert_eq!(v.message.as_str(), "hello");
        if let Some(ref kv) = v.kv {
            assert!(!kv.is_empty());
        }
        counter += 1;
    }
    assert_eq!(counter, 5);
}

/// テスト用の受信サーバー
fn ws_server<A: ToSocketAddrs>(addr: A) -> JoinHandle<Vec<u8>> {
    use bytes::BufMut;
    let server = TcpListener::bind(addr).unwrap();
    let (sender, receiver) = channel();
    // dummy server
    let handle = thread::spawn(move || {
        {
            sender.send(()).unwrap();
        }
        let mut buf = Vec::new();
        let (stream, addr) = server.accept().unwrap();
        let mut ws = accept(stream).unwrap();
        loop {
            let msg = match ws.read_message() {
                Ok(x) => x,
                // close stream
                Err(_e) => {
                    log::warn!("ws message error at {}, {:?}", &addr, _e);
                    break;
                }
            };
            match msg {
                Message::Text(ref x) => {
                    buf.put(x.as_bytes());
                }
                Message::Binary(x) => {
                    buf.put(&x[..]);
                }
                Message::Close(_) => {
                    break;
                }
                _ => unimplemented!(),
            }
        }
        buf
    });

    // wait server ready
    receiver.recv().unwrap();
    handle
}
