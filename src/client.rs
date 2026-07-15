use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;

use crate::events::Event;

const MAX_MSG: usize = 135;

#[derive(Debug)]
pub enum ConnectionError {
    Tcp(io::Error),
    UnexpectedEof(Vec<u8> /* bytes before the EOF */),
}

impl From<io::Error> for ConnectionError {
    fn from(e: io::Error) -> Self {
        ConnectionError::Tcp(e)
    }
}

pub struct Writer {
    stream: TcpStream,
}

impl Writer {
    /// Sends all cmds.
    pub fn send(&mut self, cmds: &[&str]) -> io::Result<()> {
        write!(self.stream, "{}\r", cmds.join("\r"))
    }
}

pub struct Reader {
    reader: BufReader<TcpStream>,
    buf: Vec<u8>,
}

impl Reader {
    // Reads a single AVR response. Returns unprocessed &[u8].
    // Propagates fatal connection errors as Err.
    fn read_message(&mut self) -> Result<&[u8], ConnectionError> {
        self.buf.clear();
        match self.reader.read_until(b'\r', &mut self.buf)? {
            n if n > 0 && self.buf.pop_if(|&mut b| b == b'\r').is_some() => Ok(&self.buf),
            _ => Err(ConnectionError::UnexpectedEof(self.buf.clone())), // Include corrupted data.
        }
    }

    // Reads continuously, forwarding Events to the channel.
    fn listen(mut self, tx: mpsc::Sender<Event>) -> Result<(), ConnectionError> {
        loop {
            let data = self.read_message()?;
            let event: Event = Client::handle(data);

            // Channel closed; nothing left to be done.
            if tx.send(event).is_err() {
                return Ok(());
            }
        }
    }

    /// Spawns a dedicated listener thread. Tunnels Events over the returned channel
    /// (including parse errors as Event::Error). Connection errors forwarded via the handle
    pub fn spawn_listener(self) -> (mpsc::Receiver<Event>, std::thread::JoinHandle<Result<(), ConnectionError>>) {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || self.listen(tx));
        (rx, handle)
    }
}

pub struct Client;

impl Client {
    /// Establishes a connection. Returns a coupled (Writer, Reader) pair.
    pub fn connect(addr: &str) -> io::Result<(Writer, Reader)> {
        let stream = TcpStream::connect(addr)?;
        let read_half = stream.try_clone()?;
        Ok((
            Writer { stream },
            Reader {
                reader: BufReader::new(read_half),
                buf: Vec::with_capacity(MAX_MSG),
            },
        ))
    }

    // Parse raw &[u8] data to safe Events.
    fn handle(data: &[u8]) -> Event {
        if data.len() < 3 || !data.is_ascii() {
            return Event::Error(format!("{data:?}"));
        }

        let msg = str::from_utf8(data).expect("is_ascii() guarantees valid UTF-8");

        match msg.split_at(2) {
            ("SL", "POFF") => Event::Sleep(None),
            ("SL", rest) => match rest.parse() {
                Ok(v) => Event::Sleep(Some(v)),
                Err(_) => Event::Error(msg.to_owned()),
            },
            ("PW", "ON") => Event::Power(true),
            ("PW", "STANDBY") => Event::Power(false),
            ("MU", "ON") => Event::Mute(true),
            ("MU", "OFF") => Event::Mute(false),
            ("SI", rest) => Event::Input(rest.to_owned()),
            ("MV", rest) => match rest.parse() {
                Ok(v) => Event::Volume(v),
                Err(_) => Event::Error(msg.to_owned()),
            },
            ("NS", rest) if rest.len() >= 2 => {
                Event::Display(rest.as_bytes()[1] - b'0', rest[2..].to_owned())
            }
            ("NS", _) => Event::Error(msg.to_owned()),
            _ => Event::Unknown(msg.to_owned()),
        }
    }
}
