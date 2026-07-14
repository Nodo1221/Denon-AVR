use denon::client::Client;

fn main() -> std::io::Result<()> {
    let (mut writer, reader) = Client::connect("192.168.0.10:23")?;

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    writer.send(&queries)?;

    let rx = reader.spawn_listener();

    for event in rx {
        println!("{event:?}");
    }

    Ok(())
}
