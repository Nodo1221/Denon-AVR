use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const REPLY_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_MSG: usize = 135; // spec: "Communication data length: 135bytes (maximum)"

/// Send one command, read back a single CR-terminated reply if any.
/// Returns (buffer, length) -- no heap allocation, just a stack array
/// sized to the protocol's own documented maximum message length.
fn send_cmd(stream: &mut TcpStream, cmd: &str) -> ([u8; MAX_MSG], usize) {
    stream.write_all(format!("{}\r", cmd).as_bytes()).expect("write failed");
    stream.set_read_timeout(Some(REPLY_TIMEOUT)).unwrap();

    let mut buf = [0u8; MAX_MSG];
    let mut len = 0;
    let mut byte = [0u8; 1];
    while len < MAX_MSG {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\r' => break,
            Ok(_) => { buf[len] = byte[0]; len += 1; }
            Err(_) => break,
        }
    }
    (buf, len)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: {} <ip> <cmd> [cmd...]", args[0]);
        std::process::exit(1);
    }
    let addr = format!("{}:23", args[1]);
    let mut stream = TcpStream::connect(&addr).expect("connect failed");

    for cmd in &args[2..] {
        let (buf, len) = send_cmd(&mut stream, cmd);
        if len > 0 {
            println!("{}", String::from_utf8_lossy(&buf[..len]));
        }
    }
}
