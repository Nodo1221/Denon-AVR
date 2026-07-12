mod client;
mod events;

use client::Client;
use std::sync::mpsc;

fn main() -> std::io::Result<()> {
    let mut client = Client::new("192.168.0.10:23")?;

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    client.send(&queries)?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || client.listen(tx));

    for event in rx {
        println!("{event:?}");
    }

    Ok(())
}
