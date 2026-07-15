use denon::client::Client;

fn main() -> std::io::Result<()> {
    let (mut writer, reader) = Client::connect("192.168.0.10:23")?;

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    let (rx, handle) = reader.spawn_listener();

    writer.send(&queries)?;

    for event in rx {
        println!("{event:?}");
    }

    handle.join().unwrap().unwrap();

    Ok(())
}
