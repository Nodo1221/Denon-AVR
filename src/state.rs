use crate::events::Event;

#[derive(Debug, Default)]
pub struct State {
    pub power: Option<bool>,
    pub mute: Option<bool>,
    pub volume: Option<u8>,
    pub input: Option<String>,
    pub sleep: Option<u8>,
    pub display: [String; 9],
}

impl State {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Power(v) => self.power = Some(v),
            Event::Mute(v) => self.mute = Some(v),
            Event::Volume(v) => self.volume = Some(v),
            Event::Input(v) => self.input = Some(v),
            Event::Sleep(v) => self.sleep = v,
            Event::Display(n, s) => self.display[n as usize] = s,
            Event::Unknown(_) => {}
        }
    }
}