use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tungstenite::Message;
use url::Url;

use crate::{
    buffer::{SwapBufWriter, SwapBuffer},
    Log, Metadata, Record,
};

/// Client側に持たせるもの。
#[derive(Debug)]
struct WebsocketClient {
    url: url::Url,
    buf: SwapBuffer,
    tick_duration: Duration,
    finish_receiver: Receiver<()>,
}

impl WebsocketClient {
    fn new(url: url::Url, buf: SwapBuffer, finish_receiver: Receiver<()>) -> Self {
        Self::builder(url, buf, finish_receiver).build()
    }

    fn builder(
        url: url::Url,
        buf: SwapBuffer,
        finish_receiver: Receiver<()>,
    ) -> WebsocketClientBuilder {
        WebsocketClientBuilder::new(url, buf, finish_receiver)
    }

    fn run(&mut self) -> Result<(), String> {
        use std::io::Read;
        use tungstenite::client::connect;
        let (mut client, _) = connect(&self.url).unwrap();
        let mut read_buf = Vec::<u8>::with_capacity(self.buf.capacity());
        let reader = self.buf.get_reader();
        let mut next_duration = self.tick_duration;
        loop {
            let is_finaly = matches!(self.finish_receiver.recv_timeout(next_duration), Ok(_));
            let start = Instant::now();
            self.buf.swap();
            {
                let mut reader = reader.lock().unwrap();
                reader.read_to_end(&mut read_buf).unwrap();
            }
            client
                .write_message(Message::binary(&read_buf[..]))
                .unwrap();
            read_buf.clear();
            if is_finaly {
                break;
            }
            next_duration = self.tick_duration - start.elapsed();
        }
        client.close(None).unwrap();
        Ok(())
    }
}

struct WebsocketClientBuilder {
    inner: WebsocketClient,
}

impl WebsocketClientBuilder {
    fn new(url: url::Url, buf: SwapBuffer, finish_receiver: Receiver<()>) -> Self {
        Self {
            inner: WebsocketClient {
                url,
                buf,
                finish_receiver,
                tick_duration: Duration::from_millis(500),
            },
        }
    }

    fn tick_duration(mut self, dur: Duration) -> Self {
        self.inner.tick_duration = dur;
        self
    }

    fn build(self) -> WebsocketClient {
        self.inner
    }
}

pub struct LogClient {
    writer: Arc<Mutex<SwapBufWriter>>,
    close_ch: Arc<Mutex<Sender<()>>>,
}

impl LogClient {
    const DEFAULT_BUFFER: usize = 1024 * 1024 * 2;
    const DEFAULT_SWAP_DURATION_MILLIS: u64 = 500;
    pub fn new(url: Url) -> (Self, JoinHandle<()>) {
        let (sender, receiver) = channel();
        let buf = SwapBuffer::new(Self::DEFAULT_BUFFER);
        let writer = buf.get_writer();
        let mut client = WebsocketClient::builder(url, buf, receiver)
            .tick_duration(Duration::from_millis(Self::DEFAULT_SWAP_DURATION_MILLIS))
            .build();

        // run sender
        let handle = thread::spawn(move || {
            client.run().unwrap();
        });

        (
            Self {
                writer,
                close_ch: Arc::new(Mutex::new(sender)),
            },
            handle,
        )
    }
}

impl Log for LogClient {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        use std::io::Write;
        let buf = serde_cbor::to_vec(record).unwrap();
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(&buf).unwrap();
        // serde_cbor::to_writer(&mut writer, record).unwrap();
    }

    fn flush(&self) {
        let close = self.close_ch.lock().unwrap();
        close.send(()).unwrap();
    }
}

impl Drop for LogClient {
    fn drop(&mut self) {
        // self.handle.join();
    }
}

#[cfg(test)]
mod tests {
    use log::warn;
    use std::io::Write;
    use std::net::{TcpListener, ToSocketAddrs};
    use std::sync::mpsc::channel;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;
    use tungstenite::{accept, error, Message};
    use url::Url;

    use crate::buffer::SwapBuffer;
    use crate::client::WebsocketClient;

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
                        warn!("ws message error at {}, {:?}", &addr, _e);
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

    #[test]
    fn test_websocket_client() {
        // 並行してテスト実行できるようにportをずらす
        let addr = "localhost:9002";
        let server_addr = format!("ws://{}/", addr);
        let handle = ws_server(addr);
        let test_data = "Nkmm Drawings\n".as_bytes();

        // build client
        let (sender, receiver) = channel();
        let url = Url::parse(&server_addr).unwrap();
        let buf = SwapBuffer::new(1024);
        let writer = buf.get_writer();
        let mut client = WebsocketClient::builder(url, buf, receiver)
            .tick_duration(Duration::from_millis(50))
            .build();

        // run sender
        let handle_client = thread::spawn(move || {
            client.run().unwrap();
        });

        // write to buffer
        for _ in 0..20 {
            assert_eq!(
                writer.lock().unwrap().write(test_data).unwrap(),
                test_data.len()
            );
            thread::sleep(Duration::from_millis(10));
        }
        sender.send(()).unwrap();
        handle_client.join().unwrap();
        let buf = handle.join().unwrap();
        assert_eq!(buf.len(), test_data.len() * 20);
    }
}
