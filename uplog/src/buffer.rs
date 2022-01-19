use std::{
    cmp::min,
    io::{Read, Write},
    ptr,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub(crate) struct SwapBufReader {
    buf: Vec<u8>,
    read_cursor: usize,
}

impl SwapBufReader {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            read_cursor: 0,
        }
    }

    fn swap_reset(&mut self) {
        self.read_cursor = 0;
    }

    #[inline]
    fn read_from_buffer_unchecked(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        debug_assert!(!buf.is_empty());
        let buf_len = min(self.residual_length_read(), buf.len());
        let src = self.buf[self.read_cursor..].as_ptr();
        unsafe {
            ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), buf_len);
        }
        self.read_cursor += buf_len;
        Ok(buf_len)
    }

    #[inline]
    fn residual_length_read(&self) -> usize {
        self.buf.len() - self.read_cursor
    }
}

impl Read for SwapBufReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.residual_length_read() < 1 {
            Ok(0) // meaning of EOF
        } else {
            self.read_from_buffer_unchecked(buf)
        }
    }
}

#[derive(Debug)]
pub(crate) struct SwapBufWriter {
    buf: Vec<u8>,
}

impl SwapBufWriter {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    fn swap_reset(&mut self) {
        unsafe {
            self.buf.set_len(0);
        }
    }

    #[inline]
    fn write_to_buffer_unchecked(&mut self, buf: &[u8]) {
        debug_assert!(buf.len() <= self.spare_capacity_write());
        let old_len = self.buf.len();
        let buf_len = buf.len();
        let src = buf.as_ptr();
        unsafe {
            let dst = self.buf.as_mut_ptr().add(old_len);
            ptr::copy_nonoverlapping(src, dst, buf_len);
            self.buf.set_len(old_len + buf_len);
        }
    }

    #[inline]
    fn spare_capacity_write(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }
}

impl Write for SwapBufWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.len() > self.spare_capacity_write() {
            use std::io::{Error, ErrorKind};
            Err(Error::new(
                ErrorKind::OutOfMemory,
                format!(
                    "buffer is small, writing size {} has capacity {}",
                    buf.len(),
                    self.spare_capacity_write()
                ),
            ))
        } else {
            self.write_to_buffer_unchecked(buf);
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// 書き込みと読み込みスレッドの分離を許容するバッファー
/// 処理スレッドのパフォーマンスを保つためにログ出力処理を最小に保ち
/// 時間のかかる処理を別スレッドが担当する
#[derive(Debug)]
pub(crate) struct SwapBuffer {
    // 同じ大きさのバッファで律速しないように適時入れ替える
    read: Arc<Mutex<SwapBufReader>>,
    write: Arc<Mutex<SwapBufWriter>>,
    capacity: usize,
}

impl SwapBuffer {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            read: Arc::new(Mutex::new(SwapBufReader::new(capacity))),
            write: Arc::new(Mutex::new(SwapBufWriter::new(capacity))),
        }
    }

    pub(crate) fn swap(&mut self) -> usize {
        let mut wb = self
            .write
            .lock()
            .expect(crate::error::ERROR_MESSAGE_MUTEX_LOCK);
        let mut rb = self
            .read
            .lock()
            .expect(crate::error::ERROR_MESSAGE_MUTEX_LOCK);

        // deref mutで中身を取り出してswapする
        unsafe {
            std::ptr::swap(&mut rb.buf, &mut wb.buf);
        }
        rb.swap_reset();
        wb.swap_reset();
        rb.buf.len()
    }

    pub(crate) fn get_reader(&self) -> Arc<Mutex<SwapBufReader>> {
        self.read.clone()
    }

    pub(crate) fn get_writer(&self) -> Arc<Mutex<SwapBufWriter>> {
        self.write.clone()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        ops::{Deref, DerefMut},
        sync::{mpsc::channel, Arc, Mutex, MutexGuard},
        thread,
    };

    use crate::buffer::SwapBuffer;

    // control test sequence
    #[derive(Debug)]
    enum Event {
        Start,
        Finish,
    }

    #[test]
    fn test_swap_buffer() {
        let mut swbuf = SwapBuffer::new(1024);
        let mut read_buf = [0; 512];
        // expects
        let test_data1 = "Nkmm Drawings\n".as_bytes();
        let test_data2 = "Bonnu Cats".as_bytes();
        let mut expect_data = test_data1.to_owned();
        expect_data.extend(test_data2);
        let expect_data = expect_data;

        let reader = swbuf.get_reader();
        let writer = swbuf.get_writer();

        // has not data
        assert_eq!(reader.lock().unwrap().read(&mut read_buf).unwrap(), 0);

        // write testdata
        assert_eq!(
            writer.lock().unwrap().write(test_data1).unwrap(),
            test_data1.len()
        );
        assert_eq!(
            writer.lock().unwrap().write(test_data2).unwrap(),
            test_data2.len()
        );

        // yet buffer swapping
        assert_eq!(reader.lock().unwrap().read(&mut read_buf).unwrap(), 0);

        // read data
        swbuf.swap();
        assert_eq!(
            reader.lock().unwrap().read(&mut read_buf).unwrap(),
            expect_data.len()
        );
        assert_eq!(&read_buf[0..expect_data.len()], &expect_data);

        // has not new
        assert_eq!(reader.lock().unwrap().read(&mut read_buf).unwrap(), 0);
    }

    #[test]
    fn test_swap_buffer_multi_thread() {
        let test_data = "Nkmm Drawings\n".as_bytes();
        let mut read_buf = [0; 4096];

        let mut swbuf = SwapBuffer::new(1024);
        // 同時に所有は出来ないのでReader/WriterをArcする
        let reader = swbuf.get_reader();
        let writer = swbuf.get_writer();
        let (sender, receiver) = channel();

        // write thread
        thread::spawn(move || {
            sender.send(Event::Start).unwrap();
            for _ in 0..100 {
                assert_eq!(
                    writer.lock().unwrap().write(test_data).unwrap(),
                    test_data.len()
                );
                thread::sleep(std::time::Duration::from_millis(10));
            }
            sender.send(Event::Finish).unwrap();
        });

        // wait start
        match receiver.recv().unwrap() {
            Event::Start => {}
            x => panic!("unexpected event message {:?}", x),
        }

        // read and swap thread
        loop {
            swbuf.swap();
            let size = reader.lock().unwrap().read(&mut read_buf).unwrap();
            // 空もしくは中途半端に書かれていないということを確認
            assert!(size == 0 || (size >= test_data.len() && size % test_data.len() == 0));
            thread::sleep(std::time::Duration::from_millis(50));

            // 送信側が閉じて、なおかつ終了イベントがきていたら終了する
            if size > 0 {
                continue;
            }
            if let Ok(x) = receiver.try_recv() {
                match x {
                    Event::Finish => break,
                    x => panic!("unexpected event message {:?}", x),
                }
            }
        }
    }

    #[test]
    fn test_swap() {
        {
            let (ref mut a, ref mut b) = (1, 2);
            println!("a {} {:p}", a, a);
            unsafe {
                std::ptr::swap(&mut *a, &mut *b);
            }
            assert_eq!(a, &2);
            assert_eq!(b, &1);
            println!("a {} {:p}", a, a);
        }
        {
            let (mut a, mut b) = (vec![1], vec![2]);
            println!("a {:?} {:?} {:?}", a, a.as_ptr(), &a as *const Vec<i32>);
            unsafe {
                std::ptr::swap(&mut a, &mut b);
            }
            println!("a {:?} {:?} {:?}", a, a.as_ptr(), &a as *const Vec<i32>);
        }
        {
            let (a, b) = (
                Arc::new(Mutex::new(vec![1, 2])),
                Arc::new(Mutex::new(vec![2, 3])),
            );
            {
                let mut ga = a.lock().unwrap();
                let mut gb = b.lock().unwrap();
                println!(
                    "a {:?} {:?} {:?}",
                    ga,
                    ga.as_ptr(),
                    ga.deref() as *const Vec<i32>
                );
                unsafe {
                    std::ptr::swap(ga.deref_mut(), gb.deref_mut());
                }
                println!(
                    "a {:?} {:?} {:?}",
                    ga,
                    ga.as_ptr(),
                    ga.deref() as *const Vec<i32>
                );
            }
            {
                let ga = a.lock().unwrap();
                println!(
                    "re a {:?} {:?} {:?}",
                    ga,
                    ga.as_ptr(),
                    ga.deref() as *const Vec<i32>
                );
            }
            println!("sizeof Vec {}", std::mem::size_of::<Vec<i32>>());
            println!(
                "sizeof MutexGuard {}",
                std::mem::size_of::<MutexGuard<'_, Vec<i32>>>()
            );
            println!("sizeof Mutex {}", std::mem::size_of::<Mutex<Vec<i32>>>());
        }
    }
}
