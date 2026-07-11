use std::io::{self, BufReader, BufRead, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::Duration;

mod events;
use events::Event;

const MAX_MSG: usize = 135;

#[derive(Debug)]
enum Error {
    Parse,
}

struct Client {
    reader: BufReader<TcpStream>,
    buf: Vec<u8>,
}

impl Client {
    fn new(addr: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;

        Ok(Self {
            reader: BufReader::new(stream),
            buf: Vec::with_capacity(MAX_MSG),
        })
    }

    fn send(&mut self, cmds: &[&str]) -> io::Result<()> {
        write!(self.reader.get_mut(), "{}\r", cmds.join("\r"))
    }

    fn read_message(&mut self) -> io::Result<&[u8]> {
        self.buf.clear();
        self.reader.read_until(b'\r', &mut self.buf)?;
        self.buf.pop();
        Ok(&self.buf)
    }

    fn listen(mut self, tx: mpsc::Sender<Event>) -> io::Result<()> {
        self.reader.get_mut().set_read_timeout(None)?;

        loop {
            let data = self.read_message()?;
            match Self::handle(data) {
                Ok(event) => {
                    if tx.send(event).is_err() {
                        return Ok(());
                    }
                }
                Err(e) => eprintln!("parse error: {e:?}"),
            }
        }
    }

    fn handle(data: &[u8]) -> Result<Event, Error> {
        if data.len() < 3 {
            return Err(Error::Parse);
        }

        let msg = str::from_utf8(data).map_err(|_| Error::Parse)?;

        Ok(match msg.split_at(2) {
            ("SL", "POFF")    => Event::Sleep(None),
            ("SL", rest)      => Event::Sleep(Some(rest[1..].parse().map_err(|_| Error::Parse)?)),
            ("PW", "ON")      => Event::Power(true),
            ("PW", "STANDBY") => Event::Power(false),
            ("MU", "ON")      => Event::Mute(true),
            ("MU", "OFF")     => Event::Mute(false),
            ("SI", rest)      => Event::Input(rest.to_owned()),
            ("MV", rest)      => Event::Volume(rest.parse().map_err(|_| Error::Parse)?),
            ("NS", rest) if rest.len() >= 2 => Event::Display(rest.as_bytes()[1] - b'0', rest[2..].to_owned()),
            ("NS", _)         => return Err(Error::Parse),
            _                 => Event::Unknown(msg.to_owned()),
        })
    }
}

fn main() {
    let mut client = Client::new("192.168.0.10:23").expect("connection failed");

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    client.send(&queries).unwrap();

    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        client
            .listen(tx)
            .unwrap_or_else(|e| eprintln!("connection closed: {e}"));
    });

    for event in rx {
        println!("{event:?}");
    }
}