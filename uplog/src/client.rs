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
use tungstenite::Message;
use url::Url;

use crate::{
    buffer::{SwapBufWriter, SwapBuffer},
    logger::{set_boxed_logger, SetLoggerError},
    session_init, Log, MetadataBorrow, RecordBorrow,
};

#[allow(dead_code)]
pub const WS_DEFAULT_PORT: u16 = 8040;
#[allow(dead_code)]
pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024 * 2;

/// initialize the global logger with noop
pub fn init_noop() {
    session_init();
}

/// initialize the global logger
/// # Example
///
/// ```
/// /// initialize log
/// uplog::try_init().unwrap();
///
/// // your program...
///
/// // Force recommend call finally flush()
/// uplog::flush();
/// ```
pub fn try_init() -> Result<(), SetLoggerError> {
    let (logger, handle) = Builder::default().build();
    set_boxed_logger(Box::new(logger), handle)?;
    Ok(())
}

/// initialize the global logger with logging server host
///
/// # Example
///
/// ```
/// uplog::try_init_with_host("localhost").unwrap();
/// ```
pub fn try_init_with_host(host: &str) -> Result<(), SetLoggerError> {
    let (logger, handle) = Builder::default().host(host).build();
    set_boxed_logger(Box::new(logger), handle)?;
    Ok(())
}

/// initialize the global logger with builder
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// let mut builder = uplog::Builder::default();
/// builder.buffer_size(1024)
///     .host("localhost")
///     .port(8080)
///     .duration(Duration::from_millis(1000));
/// uplog::try_init_with_builder(builder).unwrap();
/// ```
pub fn try_init_with_builder(builder: Builder) -> Result<(), SetLoggerError> {
    let (logger, handle) = builder.build();
    set_boxed_logger(Box::new(logger), handle)?;
    Ok(())
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
            log::debug!("send {}", read_buf.len());
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

/// Build the logger instance
#[derive(Debug, Clone, Copy)]
pub struct Builder<'b> {
    secure_connection: bool,
    host: &'b str,
    port: u16,
    swap_buffer_size: usize,
    swap_duration: Duration,
}

impl<'b> Builder<'b> {
    const DEFAULT_SWAP_DURATION_MILLIS: u64 = 500;

    /// Sets the swap buffer size.
    ///
    /// Maximum amount of buffer that can be stored until it is sent to the server
    /// The amount actually reserved is twice this specified value (for sending and writing).
    pub fn buffer_size(&mut self, size: usize) -> &mut Self {
        self.swap_buffer_size = size;
        self
    }

    /// Sets the swap suration.
    ///
    /// Swap the buffer every cycle specified here
    pub fn duration(&mut self, duration: Duration) -> &mut Self {
        self.swap_duration = duration;
        self
    }

    /// Sets the server host name
    pub fn host(&mut self, host: &'b str) -> &mut Self {
        self.host = host;
        self
    }

    /// Sets the server port
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

impl<'b> Default for Builder<'b> {
    fn default() -> Self {
        Self {
            secure_connection: false,
            host: "localhost",
            port: WS_DEFAULT_PORT,
            swap_buffer_size: DEFAULT_BUFFER_SIZE,
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
    fn enabled(&self, _metadata: &MetadataBorrow) -> bool {
        true
    }

    fn log(&self, record: &RecordBorrow) {
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
    use std::io::Write;
    use std::net::{TcpListener, ToSocketAddrs};
    use std::sync::mpsc::channel;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;
    use tungstenite::{accept, Message};
    use url::Url;

    use crate::buffer::SwapBuffer;
    use crate::client::WebsocketClient;

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
}
