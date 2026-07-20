use std::collections::VecDeque;
use std::time::Duration;

use denon::client::Client;
use denon::state::State;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CEvent,
    KeyCode, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, LineGauge, Paragraph};

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [_, inner, _] = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);

    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(inner);

    center
}

fn main() -> std::io::Result<()> {
    // why not define the widgets outside then? if paragraph moves text
    let (mut writer, reader) = Client::connect("192.168.0.10:23").expect("connection failed");

    // let help = "/\tenter command mode\n
    // ?\tshow this help\n
    // q\t quit this bitch";

    let help = Text::from(vec![
    Line::from(vec![
        Span::styled("?", Style::default().fg(Color::Yellow)),
        Span::raw("\ttoggle this help"),
    ]),
    Line::from(vec![
        Span::styled("/", Style::default().fg(Color::Yellow)),
        Span::raw("\tenter command mode"),
    ]),
    Line::from(vec![
        Span::styled("↑ ↓", Style::default().fg(Color::Yellow)),
        Span::raw("\tvolume up / down"),
    ]),
    Line::from(vec![
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw("\tquit"),
    ]),
]);


    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    writer.send(&queries)?;

    let (rx, _handle) = reader.spawn_listener();

    let mut state = State::default();
    let mut log: VecDeque<String> = VecDeque::new();
    let mut command_buf: Option<String> = None;
    let mut input_area = Rect::default();
    let mut show_help = false;

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
                let [header_area, main_area, ia] = Layout::vertical([
                    Constraint::Length(4),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .areas(frame.area());
                input_area = ia;

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
                let gauge = LineGauge::default()
                    .block(Block::default().title("Volume").borders(Borders::ALL))
                    .filled_style(Style::default().fg(Color::Cyan))
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
                    None => "press / to enter a command, ? for help".to_string(),
                };
                let input_widget = Paragraph::new(input_text)
                    .block(Block::default().title("Command").borders(Borders::ALL));
                frame.render_widget(input_widget, ia);

                if let Some(buf) = &command_buf {
                    let x = ia.x + 2 + buf.chars().count() as u16;
                    let y = ia.y + 1;
                    frame.set_cursor_position((x, y));
                }

                if show_help {
                    let popup_area = centered_rect(50, 50, frame.area());
                    frame.render_widget(ratatui::widgets::Clear, popup_area);
                    let help = Paragraph::new(help.clone())
                        .block(Block::default().title("Help").borders(Borders::ALL));
                    frame.render_widget(help, popup_area);
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
                                KeyCode::Backspace => { buf.pop(); }
                                KeyCode::Char(c) => buf.push(c),
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') => break Ok(()),
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
                            if input_area.contains(Position { x: mouse.column, y: mouse.row }) =>
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