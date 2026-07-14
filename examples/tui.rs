use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::Duration;

use denon::client::Client;
use denon::state::State;

use ratatui::crossterm::event::{self, Event as CEvent, KeyCode};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

fn main() -> std::io::Result<()> {
    let (mut writer, reader) = Client::connect("192.168.0.10:23").expect("connection failed");

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    writer.send(&queries)?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || reader.listen(tx));

    let mut state = State::default();
    let mut log: VecDeque<String> = VecDeque::new();
    let mut command_buf: Option<String> = None;

    ratatui::run(|terminal| {
        loop {
            while let Ok(event) = rx.try_recv() {
                log.push_back(format!("{event:?}"));
                state.apply(event);
                if log.len() > 200 {
                    log.pop_front();
                }
            }

            terminal.draw(|frame| {
                let [header_area, main_area, input_area] = Layout::vertical([
                    Constraint::Length(4),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .areas(frame.area());

                let [log_area, left_area] =
                    Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
                        .areas(main_area);

                let [volume_area, display_area] =
                    Layout::vertical([Constraint::Length(3), Constraint::Min(5)])
                        .areas(left_area);

                let power_icon = match state.power {
                    Some(true) => Span::styled("●", Style::default().fg(Color::Green)),
                    Some(false) => Span::styled("●", Style::default().fg(Color::Yellow)),
                    None => Span::styled("●", Style::default().fg(Color::DarkGray)),
                };
                let sleep_text = match state.sleep {
                    Some(m) => format!("(Sleep: {m}m) "),
                    None => "(no sleep) ".to_string(),
                };
                let title = Line::from(vec![
                    Span::styled(
                        " Denon AVR ",
                        Style::default()
                            .fg(Color::Rgb(255, 215, 0))
                            .add_modifier(Modifier::BOLD),
                    ),
                    power_icon,
                    Span::raw(" "),
                    Span::styled(sleep_text, Style::default().fg(Color::Gray)),
                ]);

                let header_text = format!("Mute:  {:?}\nInput: {:?}", state.mute, state.input);
                let header = Paragraph::new(header_text).block(
                    Block::default()
                        .title(title)
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL),
                );
                frame.render_widget(header, header_area);

                let volume = state.volume.unwrap_or(0);
                let ratio = (volume as f64 / 60.0).clamp(0.0, 1.0);
                let gauge = Gauge::default()
                    .block(Block::default().title("Volume").borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Cyan))
                    .ratio(ratio)
                    .label(format!("{volume}/60"));
                frame.render_widget(gauge, volume_area);

                let display_lines = state.display.to_vec();
                let display_widget = Paragraph::new(display_lines.join("\n"))
                    .block(Block::default().title("Display").borders(Borders::ALL));
                frame.render_widget(display_widget, display_area);

                let log_text = log.iter().map(String::as_str).collect::<Vec<_>>().join("\n");
                let inner_height = log_area.height.saturating_sub(2);
                let scroll = (log.len() as u16).saturating_sub(inner_height);
                let log_widget = Paragraph::new(log_text)
                    .block(Block::default().title("Log").borders(Borders::ALL))
                    .scroll((scroll, 0));
                frame.render_widget(log_widget, log_area);

                let input_text = match &command_buf {
                    Some(buf) => format!("/{buf}"),
                    None => "press / to enter a command".to_string(),
                };
                let input_widget = Paragraph::new(input_text)
                    .block(Block::default().title("Command").borders(Borders::ALL));
                frame.render_widget(input_widget, input_area);

                if let Some(buf) = &command_buf {
                    let x = input_area.x + 2 + buf.chars().count() as u16;
                    let y = input_area.y + 1;
                    frame.set_cursor_position((x, y));
                }
            })?;

            if event::poll(Duration::from_millis(16))? {
                if let CEvent::Key(key) = event::read()? {
                    if let Some(buf) = command_buf.as_mut() {
                        match key.code {
                            KeyCode::Enter => {
                                writer.send(&[buf.as_str()])?;
                                log.push_back(format!("> sent: {buf}"));
                                command_buf = None;
                            }
                            KeyCode::Esc => command_buf = None,
                            KeyCode::Backspace => {
                                buf.pop();
                            }
                            KeyCode::Char(c) => buf.push(c),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => break Ok(()),
                            KeyCode::Up => writer.send(&["MVUP"])?,
                            KeyCode::Down => writer.send(&["MVDOWN"])?,
                            KeyCode::Char('/') => command_buf = Some(String::new()),
                            _ => {}
                        }
                    }
                }
            }
        }
    })
}