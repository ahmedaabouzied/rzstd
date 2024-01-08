use std::io::{self, Read};
use tokio::sync::broadcast::Sender;

pub struct Progress<R> {
    inner: R,
    bytes_read: usize,
    progress_sender: Sender<usize>,
}

impl<R: Read> Progress<R> {
    pub fn new(inner: R, progress_sender: Sender<usize>) -> Self {
        Progress {
            inner,
            bytes_read: 0,
            progress_sender,
        }
    }
}

impl<R: Read> Read for Progress<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.inner.read(buf);
        if let Ok(bytes) = result {
            self.bytes_read += bytes;
            match self.progress_sender.send(self.bytes_read) {
                Ok(_) => (),
                Err(_) => (),
            }
        }
        result
    }
}
