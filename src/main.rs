mod client;
mod events;

use client::Client;
use std::sync::mpsc;

use ratatui::crossterm::event::{self, Event as CEvent, KeyCode};
use ratatui::widgets::{Block, Borders, Paragraph, Gauge};
use ratatui::style::{Style, Color};
use ratatui::layout::{Constraint, Layout};
use std::time::Duration;
use events::Event;
use std::io::Write;

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

fn main() -> std::io::Result<()> {
    let mut client = Client::new("192.168.0.10:23").expect("connection failed");

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    client.send(&queries).unwrap();

    let mut writer = client.try_clone_stream()?;

    let (tx, rx) = mpsc::channel();
    let mut state = State::default();

    std::thread::spawn(move || {
        client
            .listen(tx)
            .unwrap_or_else(|e| eprintln!("connection closed: {e}"));
    });

    ratatui::run(|mut terminal| {
        loop {
            while let Ok(event) = rx.try_recv() {
                state.apply(event);
            }

            terminal.draw(|frame| {
                let [status_area, volume_area] =
                    Layout::vertical([Constraint::Min(6), Constraint::Length(3)])
                        .areas(frame.area());

                let text = format!(
                    "Power:  {:?}\nMute:   {:?}\nInput:  {:?}\nSleep:  {:?}\n\n(q to quit, ↑/↓ volume)",
                    state.power, state.mute, state.input, state.sleep
                );
                let status = Paragraph::new(text)
                    .block(Block::default().title("Denon AVR").borders(Borders::ALL));
                frame.render_widget(status, status_area);

                let display_lines: Vec<&str> = state.display.iter()
                    .filter_map(|line| line.as_deref())
                    .collect();
                let display_text = display_lines.join("\n");

                let display = Paragraph::new(display_text)
                    .block(Block::default().title("Display").borders(Borders::ALL));

                let volume = state.volume.unwrap_or(0);
                let ratio = (volume as f64 / 60.0).clamp(0.0, 1.0);
                let gauge = Gauge::default()
                    .block(Block::default().title("Volume").borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Cyan))
                    .ratio(ratio)
                    .label(format!("{volume}/60"));
                frame.render_widget(gauge, volume_area);

            })?;

            if event::poll(Duration::from_millis(16))? {
                if let CEvent::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break Ok(()),
                        KeyCode::Up => write!(writer, "MVUP\r")?,
                        KeyCode::Down => write!(writer, "MVDOWN\r")?,
                        _ => {}
                    }
                }
            }
        }
    })
}