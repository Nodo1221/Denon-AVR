use std::env;
use std::io::{Read, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

const MAX_MSG: usize = 135;

struct Client {
    reader: BufReader<TcpStream>,
    buf: [u8; MAX_MSG],
}

impl Client {
    fn new(addr: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self {
            reader: BufReader::new(stream),
            buf: [0; MAX_MSG]
        })
    }

    fn send(&mut self, cmd: &str) -> std::io::Result<()> {
        write!(self.reader.get_mut(), "{cmd}\r")
    }

    /// Reads a single CR-terminated message into the internal buffer.
    fn read_message(&mut self) -> std::io::Result<&[u8]> {
        let mut len = 0;

        for b in self.reader.by_ref().bytes().take(MAX_MSG) {
            match b {
                Ok(b) if b != b'\r' => { self.buf[len] = b; len += 1; }
                Err(e) => return Err(e),
                _ => break,
            }
        }
        
        Ok(&self.buf[..len])
    }

    /// Reads incoming messages until the connection drops.
    fn listen(&mut self) -> std::io::Result<()> {
        loop {
            let msg = self.read_message()?;
            if !msg.is_empty() {
                println!("{}", String::from_utf8_lossy(msg));
            }
        }
    }
}

fn main() {
    let mut client = Client::new("192.168.0.10:23").expect("connection failed");

    let mut write_stream = client.reader.get_mut().try_clone().expect("clone failed");
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        write!(write_stream, "PW?\r").unwrap();
    });

    client.listen().unwrap_or_else(|e| eprintln!("connection closed: {e}"));
}