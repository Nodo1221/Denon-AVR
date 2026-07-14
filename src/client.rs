use std::io::{self, BufReader, BufRead, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::Duration;

use crate::events::Event;

const MAX_MSG: usize = 135;

#[derive(Debug)]
enum Error {
    Parse(String),
}

pub struct Writer {
    stream: TcpStream,
}

impl Writer {
    pub fn send(&mut self, cmds: &[&str]) -> io::Result<()> {
        write!(self.stream, "{}\r", cmds.join("\r"))
    }
}

pub struct Reader {
    reader: BufReader<TcpStream>,
    buf: Vec<u8>,
}

impl Reader {
    fn read_message(&mut self) -> io::Result<&[u8]> {
        self.buf.clear();
        self.reader.read_until(b'\r', &mut self.buf)?;
        self.buf.pop();
        Ok(&self.buf)
    }

    pub fn listen(mut self, tx: mpsc::Sender<Event>) -> io::Result<()> {
        loop {
            let data = self.read_message()?;
            let event = Client::handle(data).unwrap_or_else(|e| Event::Error(format!("{e:?}")));
            if tx.send(event).is_err() {
                return Ok(());
            }
        }
    }

    pub fn spawn_listener(self) -> mpsc::Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || self.listen(tx));
        rx
    }
}

pub struct Client;

impl Client {
    pub fn connect(addr: &str) -> io::Result<(Writer, Reader)> {
        let stream = TcpStream::connect(addr)?;
        let read_half = stream.try_clone()?;

        Ok((
            Writer { stream },
            Reader { reader: BufReader::new(read_half), buf: Vec::with_capacity(MAX_MSG) },
        ))
    }

    fn handle(data: &[u8]) -> Result<Event, Error> {
        if data.len() < 3 {
            return Err(Error::Parse(format!("{data:?}")));
        }

        let msg = str::from_utf8(data)
            .map_err(|_| Error::Parse(format!("{data:?}")))?;

        Ok(match msg.split_at(2) {
            ("SL", "POFF")    => Event::Sleep(None),
            ("SL", rest)      => Event::Sleep(Some(rest[1..].parse().map_err(|_| Error::Parse(msg.to_owned()))?)),
            ("PW", "ON")      => Event::Power(true),
            ("PW", "STANDBY") => Event::Power(false),
            ("MU", "ON")      => Event::Mute(true),
            ("MU", "OFF")     => Event::Mute(false),
            ("SI", rest)      => Event::Input(rest.to_owned()),
            ("MV", rest)      => Event::Volume(
                rest.parse().map_err(|_| Error::Parse(msg.to_owned()))?
            ),
            ("NS", rest) if rest.len() >= 2
                            => Event::Display(rest.as_bytes()[1] - b'0', rest[2..].to_owned()),
            ("NS", _)         => return Err(Error::Parse(msg.to_owned())),
            _                 => Event::Unknown(msg.to_owned()),
        })
    }
}