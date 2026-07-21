use std::collections::VecDeque;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Flex, Layout, Position, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, LineGauge, Paragraph};

use denon::client::Client;
use denon::state::State;

fn main() -> std::io::Result<()> {
    let (mut writer, reader) = Client::connect("192.168.0.10:23").expect("connection failed");

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    writer.send(&queries)?;

    let (rx, _) = reader.spawn_listener();

    let mut state = State::default();
    let mut log: VecDeque<String> = VecDeque::new();
    let mut command_buf: Option<String> = None;
    let mut input_area = Rect::default();
    let mut show_help = false;

    let main_layout = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(10),
        Constraint::Length(3),
    ]);
    let split_layout = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]);
    let left_layout = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]);

    // help_widget is complex enough to hoist; rendered by reference via Widget for &W
    let help_widget = Paragraph::new(
        [
            ("?", "toggle this help", Color::Yellow),
            ("/", "enter command mode", Color::Yellow),
            ("q", "quit the program", Color::Yellow),
            ("p", "toggle power", Color::Green),
            ("↑ ↓", "volume up / down", Color::Green),
        ]
        .into_iter()
        .map(|l| {
            Line::from_iter([
                Span::styled(format!("{:6}", l.0), Style::new().fg(l.2).bold()),
                Span::raw(l.1),
            ])
        })
        .collect::<Text<'_>>(),
    )
    .block(
        Block::bordered()
            .border_type(BorderType::Double)
            .title("Help")
            .title_bottom(
                Line::from(Span::styled(
                    " Press Esc to close ",
                    Style::new().fg(Color::LightBlue),
                ))
                .right_aligned(),
            ),
    );

    execute!(std::io::stdout(), EnableMouseCapture)?;

    let result = ratatui::run(|terminal| {
        loop {
            while let Ok(event) = rx.try_recv() {
                log.push_back(format!("{event:?}"));
                state.apply(event);
                if log.len() > 200 {
                    log.pop_front();
                }
            }

            terminal.draw(|frame| {
                // Evaluate layouts for current frame area
                let [header_area, main_area, ia] = main_layout.areas(frame.area());
                input_area = ia;

                let [log_area, left_area] = split_layout.areas(main_area);
                let [volume_area, display_area] = left_layout.areas(left_area);

                // Header is dynamic, build inside
                let sleep_text = match state.sleep {
                    Some(m) => format!("(Sleep: {m}m) "),
                    None => "(no sleep) ".to_string(),
                };
                let power_color = match state.power {
                    Some(true) => Color::Green,
                    Some(false) => Color::Yellow,
                    None => Color::DarkGray,
                };

                let title = Line::from_iter([
                    Span::styled(
                        " Denon AVR ",
                        Style::new().fg(Color::Rgb(255, 215, 0)).bold(),
                    ),
                    "● ".fg(power_color),
                    Span::raw(sleep_text),
                ]);

                let header_text = format!("Mute:  {:?}\nInput: {:?}", state.mute, state.input);
                let header =
                    Paragraph::new(header_text).block(Block::bordered().title(title.centered()));
                frame.render_widget(header, header_area);

                let volume = state.volume.unwrap_or(0);
                let ratio = (volume as f64 / 60.0).clamp(0.0, 1.0);
                let gauge = LineGauge::default()
                    .block(Block::bordered().title("Volume"))
                    .filled_style(Style::new().fg(Color::Cyan))
                    .ratio(ratio)
                    .label(format!("{volume}/60"));
                frame.render_widget(gauge, volume_area);

                // Display
                let display_widget = Paragraph::new(state.display.join("\n"))
                    .block(Block::bordered().title("Display"));
                frame.render_widget(display_widget, display_area);

                // Log - avoided .join("\n") massive allocation
                let log_text: Text = log.iter().map(|s| Line::from(s.as_str())).collect();
                let inner_height = log_area.height.saturating_sub(2);
                let scroll = (log.len() as u16).saturating_sub(inner_height);
                let log_widget = Paragraph::new(log_text)
                    .block(Block::bordered().title("Log"))
                    .scroll((scroll, 0));
                frame.render_widget(log_widget, log_area);

                let input_text = match &command_buf {
                    Some(buf) => format!("/{buf}"),
                    None => "press / to enter a command, ? for help".to_string(),
                };
                let input_widget = Paragraph::new(input_text).block(
                    Block::bordered()
                        .border_type(BorderType::LightDoubleDashed)
                        .title("Command"),
                );
                frame.render_widget(input_widget, ia);

                if let Some(buf) = &command_buf {
                    let x = ia.x + 2 + buf.chars().count() as u16;
                    let y = ia.y + 1;
                    frame.set_cursor_position((x, y));
                }

                if show_help {
                    // Rect::centered replaces the centered_rect helper
                    let popup_area = frame
                        .area()
                        .centered(Constraint::Percentage(50), Constraint::Percentage(50));
                    frame.render_widget(Clear, popup_area);
                    frame.render_widget(&help_widget, popup_area); // Rendered by reference
                }
            })?;

            if event::poll(Duration::from_millis(16))? {
                match event::read()? {
                    CEvent::Key(key) => {
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
                                KeyCode::Char('p') => {
                                    let cmd = if state.power == Some(true) {
                                        "PWOFF"
                                    } else {
                                        "PWON"
                                    };
                                    writer.send(&[cmd])?;
                                    log.push_back(format!("> sent: {cmd}"));
                                }
                                KeyCode::Up => {
                                    writer.send(&["MVUP"])?;
                                    log.push_back("> sent: MVUP".to_string());
                                }
                                KeyCode::Down => {
                                    writer.send(&["MVDOWN"])?;
                                    log.push_back("> sent: MVDOWN".to_string());
                                }
                                KeyCode::Char('?') => show_help = !show_help,
                                KeyCode::Esc => show_help = false,
                                KeyCode::Char('/') => command_buf = Some(String::new()),
                                _ => {}
                            }
                        }
                    }
                    CEvent::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::Down(_)
                            if input_area.contains(Position {
                                x: mouse.column,
                                y: mouse.row,
                            }) =>
                        {
                            if command_buf.is_none() {
                                command_buf = Some(String::new());
                            }
                        }
                        MouseEventKind::Down(_) => command_buf = None,
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    });

    execute!(std::io::stdout(), DisableMouseCapture)?;
    result
}
