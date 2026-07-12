use crate::events::Event;

#[derive(Debug, Default)]
pub struct State {
    pub power: Option<bool>,
    pub mute: Option<bool>,
    pub volume: Option<u8>,
    pub input: Option<String>,
    pub sleep: Option<u8>,
    pub display: [Option<String>; 9],
}

impl State {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Power(v)   => self.power = Some(v),
            Event::Mute(v)    => self.mute = Some(v),
            Event::Volume(v)  => self.volume = Some(v),
            Event::Input(v)   => self.input = Some(v),
            Event::Sleep(v)   => self.sleep = v,
            Event::Display(n, s) => {
                if let Some(slot) = self.display.get_mut(n as usize) {
                    *slot = Some(s);
                }
            }
            Event::Unknown(_) => {}
        }
    }
}