/// logger実体
use std::{
    ops::DerefMut,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use log::debug;
use tungstenite::Message;
use url::Url;

use crate::{Log, Metadata, Record, buffer::{SwapBufWriter, SwapBuffer}, logger::{SetLoggerError, set_boxed_logger}, session_init};

#[allow(dead_code)]
pub const WS_DEFAULT_PORT: u16 = 8040;

// initialize the global logger
pub fn try_init() -> Result<JoinHandle<()>, SetLoggerError> {
    let (logger, handle) = Builder::default().build();
    set_boxed_logger(Box::new(logger))?;
    Ok(handle)
}

/// メインスレッドと別に起動してバッファーを監視し
/// 外部のログサーバーに対してログを送信し続けるクライアント
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
            debug!("send {}", read_buf.len());
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

pub struct Builder {
    secure_connection: bool,
    host: String,
    port: u16,
    swap_buffer_size: usize,
    swap_duration: Duration,
}

impl Builder {
    const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024 * 2;
    const DEFAULT_SWAP_DURATION_MILLIS: u64 = 500;

    pub fn buffer_size(&mut self, size: usize) -> &mut Self {
        self.swap_buffer_size = size;
        self
    }

    pub fn duration(&mut self, duration: Duration) -> &mut Self {
        self.swap_duration = duration;
        self
    }

    pub fn host(&mut self, host: &str) -> &mut Self {
        self.host = host.to_string();
        self
    }

    pub fn port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    fn url(&self) -> Url {
        let protocol = match self.secure_connection {
            true => "wss",
            false => "ws",
        };
        let addr = format!("{}:/{}:{}", protocol, self.host, self.port);
        Url::parse(&addr).expect("failed to parse url")
    }

    pub fn build(self) -> (LogClient, JoinHandle<()>) {
        let url = self.url();
        LogClient::new(url, self.swap_buffer_size, self.swap_duration)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            secure_connection: false,
            host: "localhost".to_string(),
            port: WS_DEFAULT_PORT,
            swap_buffer_size: Self::DEFAULT_BUFFER_SIZE,
            swap_duration: Duration::from_millis(Self::DEFAULT_SWAP_DURATION_MILLIS),
        }
    }
}

/// メインスレッドにログ出力の関数を提供するクライアント
pub struct LogClient {
    writer: Arc<Mutex<SwapBufWriter>>,
    close_ch: Arc<Mutex<Sender<()>>>,
}

impl LogClient {
    pub fn new(url: Url, buffer_size: usize, swap_duration: Duration) -> (Self, JoinHandle<()>) {
        session_init();
        let (sender, receiver) = channel();
        let buf = SwapBuffer::new(buffer_size);
        let writer = buf.get_writer();
        let mut client = WebsocketClient::builder(url, buf, receiver)
            .tick_duration(swap_duration)
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
        let mut writer = self.writer.lock().unwrap();
        serde_cbor::to_writer(writer.deref_mut(), record).unwrap();
    }

    fn flush(&self) {
        let close = self.close_ch.lock().unwrap();
        close.send(()).ok();
    }
}

impl Drop for LogClient {
    fn drop(&mut self) {
        let close = self.close_ch.lock().unwrap();
        close.send(()).ok();
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
    use crate::client::{Builder, LogClient, WebsocketClient};
    use crate::{Log, Record};

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

    /// 送信スレッドのテスト
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

    // loggerとしてふるまいを確認
    #[test]
    fn test_logger() {
        let host = "localhost";
        let port = 9003;
        let server_addr = format!("{}:{}", host, port);
        let handle = ws_server(server_addr);

        let test_category = "uplog.client.test";
        let test_message = "Nkmm Drawings";

        let mut builder = Builder::default();
        builder.port(port).duration(Duration::from_millis(40));
        let (logger, handle_client) = builder.build();

        // write to logger
        for _ in 0..20 {
            let r = devlog!(crate::Level::Info, test_category, test_message);
            logger.log(&r);
            thread::sleep(Duration::from_millis(10));
        }
        logger.flush();
        handle_client.join().unwrap();
        let buf = handle.join().unwrap();
        let iter = serde_cbor::Deserializer::from_slice(&buf).into_iter::<Record>();
        let mut counter = 0;
        for v in iter {
            let v: Record = v.unwrap();
            assert_eq!(v.message, *test_message);
            assert_eq!(v.category, *test_category);
            counter += 1;
        }
        assert_eq!(counter, 20);
    }
}
