#[derive(Debug)]
pub enum Event {
    Power(bool),
    Volume(u8),
    Mute(bool),
    Sleep(bool),
    Input(String),
    Display(u8, String),
    Unknown(String),
}
