use std::env;
use std::io::{self, Read, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

const REPLY_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_MSG: usize = 135;

struct Client {
    reader: BufReader<TcpStream>,
    buf: [u8; MAX_MSG],
}

impl Client {
    fn new(addr: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(REPLY_TIMEOUT))?;

        Ok(Self {
            reader: BufReader::new(stream),
            buf: [0; MAX_MSG]
        })
    }

    fn send(&mut self, cmd: &str) -> io::Result<&[u8]> {
        write!(self.reader.get_mut(), "{cmd}\r")?;
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
}

fn main() {
    let mut args = env::args().skip(1);
    let ip = args.next().expect("usage: <ip> <cmd> [cmd...]");
    let mut client = Client::new(&format!("{}:23", ip)).expect("connection failed");
    
    for cmd in args {
        match client.send(&cmd) {
            Ok(reply) => println!("{}", String::from_utf8_lossy(reply)),
            Err(e) => eprintln!("error: {e}"),
        }
    }
}

// ./target/release/denon 192.168.0.10 'PW?'
// ./target/release/denon 192.168.0.10 'NSE' 
// Minimal, dependency-free wrapper around Denon AVR protocol. Easily extensible. Usage:
//
// Example script in main