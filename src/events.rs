#[derive(Debug)]
pub enum Event {
    Power(bool),
    Volume(u8),
    Mute(bool),
    Sleep(Option<u8>),
    Input(String),
    Display(u8, String),
    Unknown(String),
}
