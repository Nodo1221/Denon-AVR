use std::env;
use std::io::{BufReader, Read, Write};
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
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;

        Ok(Self {
            reader: BufReader::new(stream),
            buf: [0; MAX_MSG],
        })
    }

    fn send(&mut self, cmds: &[&str]) -> std::io::Result<()> {
        let buf = cmds.join("\r");
        write!(self.reader.get_mut(), "{buf}\r")
    }

    /// Reads a single CR-terminated message into the internal buffer.
    fn read_message(&mut self) -> std::io::Result<&[u8]> {
        let mut len = 0;

        for b in self.reader.by_ref().bytes().take(MAX_MSG) {
            match b {
                Ok(b) if b != b'\r' => {
                    self.buf[len] = b;
                    len += 1;
                }
                Err(e) => return Err(e),
                _ => break,
            }
        }

        Ok(&self.buf[..len])
    }

    /// Reads incoming messages until the connection drops.
    fn listen(&mut self) -> std::io::Result<()> {
        self.reader.get_mut().set_read_timeout(None)?;
        loop {
            let msg = self.read_message()?;
            match str::from_utf8(msg) {
                Ok(s) if s.is_empty() => println!("Empty response"),
                Ok(s) => Self::handle(s),
                Err(_) => eprintln!("Invalid utf-8: {msg:?}"),
            }
        }
    }

    fn handle(msg: &str) {
        let i = msg
            .find(|c: char| !c.is_ascii_uppercase())
            .unwrap_or(msg.len());
        let (cmd, val) = (&msg[..i], &msg[i..]);
        match cmd {
            "PW" => println!("power: {val}"),
            "MV" => println!("volume: {val}"),
            "MU" => println!("mute: {val}"),
            "SI" => println!("input: {val}"),
            "MS" => println!("surround: {val}"),
            "ZM" => println!("zone: {val}"),
            "SLP" => println!("sleep: {val}"),
            "NSE" => println!("display[{}]: {}", &val[..1], &val[1..]),
            _ => println!("unknown: {cmd} {val}"),
        }
    }
}

fn main() {
    let mut client = Client::new("192.168.0.10:23").expect("Connection failed");

    let queries = [
        "PW?", "ZM?", "MV?", "MU?", "SI?", "MS?", "CV?", "SLP?",
        "NSE",  // display lines 0–7, one response per line
        "NSP?", // network play status
        "NST?", // net audio status
        "TF?",  // tuner frequency
        "TP?",  // tuner preset
    ];

    client.send(&queries).unwrap();

    client
        .listen()
        .unwrap_or_else(|e| eprintln!("Connection closed: {e}"));
}
