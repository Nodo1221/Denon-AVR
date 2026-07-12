use denon::client::Client;

fn main() -> std::io::Result<()> {
    let mut client = Client::new("192.168.0.10:23")?;

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    client.send(&queries)?;

    let rx = client.spawn_listener();

    for event in rx {
        println!("{event:?}");
    }

    Ok(())
}
