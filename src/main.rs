use std::env;
use std::io::{BufReader, BufRead, Write};
use std::net::TcpStream;
use std::time::Duration;

const MAX_MSG: usize = 135;

struct Client {
    reader: BufReader<TcpStream>,
    buf: Vec<u8>,
}

impl Client {
    fn new(addr: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        Ok(Self {
            reader: BufReader::new(stream),
            buf: Vec::with_capacity(MAX_MSG),
        })
    }

    fn send(&mut self, cmds: &[&str]) -> std::io::Result<()> {
        let buf = cmds.join("\r");
        write!(self.reader.get_mut(), "{buf}\r")
    }

    fn read_message(&mut self) -> std::io::Result<&[u8]> {
        self.buf.clear();
        self.reader.read_until(b'\r', &mut self.buf)?;
        self.buf.pop();
        Ok(&self.buf)
    }

    fn listen(&mut self) -> std::io::Result<()> {
        self.reader.get_mut().set_read_timeout(None)?;
        loop {
            let msg = self.read_message()?;
            match str::from_utf8(msg) {
                Ok(s) if s.is_empty() => eprintln!("empty message"),
                Ok(s) => Self::handle(s),
                Err(_) => eprintln!("invalid utf-8: {msg:?}"),
            }
        }
    }

    fn handle(msg: &str) {
        let handlers: &[(&str, fn(&str))] = &[
            ("PW", |val| println!("power: {val}")),
            ("MV", |val| println!("volume: {val}")),
            ("MU", |val| println!("mute: {val}")),
            ("SI", |val| println!("input: {val}")),
            ("MS", |val| println!("surround: {val}")),
            ("ZM", |val| println!("zone: {val}")),
            ("SLP", |val| println!("sleep: {val}")),
            ("NSE", |val| {
                println!("display[{}]: {}", &val[..1], &val[1..])
            }),
        ];

        for (prefix, handler) in handlers {
            if let Some(val) = msg.strip_prefix(prefix) {
                handler(val);
                return;
            }
        }

        eprintln!("unknown: {msg}");
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
