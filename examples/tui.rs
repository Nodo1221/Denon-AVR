use std::collections::VecDeque;
use std::time::Duration;

use denon::client::Client;
use denon::state::State;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CEvent,
    KeyCode, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, LineGauge, Paragraph};

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [vertical_center] = Layout::vertical([Constraint::Percentage(percent_y)])
        .flex(Flex::Center) // Automatically pads the top and bottom
        .areas(area);

    let [center] = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center) // Automatically pads the left and right
        .areas(vertical_center);

    center
}

fn main() -> std::io::Result<()> {
    let (mut writer, reader) = Client::connect("192.168.0.10:23").expect("connection failed");

    let queries = ["PW?", "MV?", "MU?", "SI?", "SLP?", "NSE"];
    writer.send(&queries)?;

    let (rx, _handle) = reader.spawn_listener();

    let mut state = State::default();
    let mut log: VecDeque<String> = VecDeque::new();
    let mut command_buf: Option<String> = None;
    let mut input_area = Rect::default();
    let mut show_help = false;

    // 1. Build Static Layouts Outside
    let main_layout = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(10),
        Constraint::Length(3),
    ]);
    let split_layout = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]);
    let left_layout = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]);

    // 2. Build Static Blocks Outside
    let gauge_block = Block::new().title("Volume").borders(Borders::ALL);
    let display_block = Block::new().title("Display").borders(Borders::ALL);
    let log_block = Block::new().title("Log").borders(Borders::ALL);
    let input_block = Block::new().title("Command").borders(Borders::ALL);

    // 3. Build Static Help Widget Outside (Startup allocation is fine here)
    let help_widget = Paragraph::new(vec![
        Line::from_iter([
            Span::styled("?", Style::new().fg(Color::Yellow)),
            Span::raw("      toggle this help"),
        ]),
        Line::from_iter([
            Span::styled("/", Style::new().fg(Color::Yellow)),
            Span::raw("      enter command mode"),
        ]),
        Line::from_iter([
            Span::styled("q", Style::new().fg(Color::Yellow)),
            Span::raw("      quit the program"),
            ]),
        Line::from_iter([
            Span::styled("p", Style::new().fg(Color::Green)),
            Span::raw("      toggle power"),
        ]),
        Line::from_iter([
            Span::styled("↑ ↓", Style::new().fg(Color::Green)),
            Span::raw("    volume up / down"),
        ]),
    ])
    .block(Block::new().title("Help").borders(Borders::ALL).title_bottom(Line::from(Span::styled(" Press Esc to close ", Style::new().fg(Color::LightBlue))).right_aligned()));

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
                let power_icon = match state.power {
                    Some(true) => Span::styled("●", Style::new().fg(Color::Green)),
                    Some(false) => Span::styled("●", Style::new().fg(Color::Yellow)),
                    None => Span::styled("●", Style::new().fg(Color::DarkGray)),
                };
                let sleep_text = match state.sleep {
                    Some(m) => format!("(Sleep: {m}m) "),
                    None => "(no sleep) ".to_string(),
                };
                let title = Line::from_iter([
                    Span::styled(
                        " Denon AVR ",
                        Style::new().fg(Color::Rgb(255, 215, 0)).add_modifier(Modifier::BOLD),
                    ),
                    power_icon,
                    Span::raw(" "),
                    Span::raw(sleep_text),
                ]);

                let header_text = format!("Mute:  {:?}\nInput: {:?}", state.mute, state.input);
                let header = Paragraph::new(header_text).block(
                    Block::new()
                        .title(title)
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL),
                );
                frame.render_widget(header, header_area);

                let volume = state.volume.unwrap_or(0);
                let ratio = (volume as f64 / 60.0).clamp(0.0, 1.0);
                let gauge = LineGauge::default()
                    .block(gauge_block.clone())
                    .filled_style(Style::new().fg(Color::Cyan))
                    .ratio(ratio)
                    .label(format!("{volume}/60"));
                frame.render_widget(gauge, volume_area);

                // Display - avoided .join("\n") allocation
                let display_text: Text = state.display.iter().map(|s| Line::from(s.as_str())).collect();
                let display_widget = Paragraph::new(display_text).block(display_block.clone());
                frame.render_widget(display_widget, display_area);

                // Log - avoided .join("\n") massive allocation
                let log_text: Text = log.iter().map(|s| Line::from(s.as_str())).collect();
                let inner_height = log_area.height.saturating_sub(2);
                let scroll = (log.len() as u16).saturating_sub(inner_height);
                let log_widget = Paragraph::new(log_text)
                    .block(log_block.clone())
                    .scroll((scroll, 0));
                frame.render_widget(log_widget, log_area);

                let input_text = match &command_buf {
                    Some(buf) => format!("/{buf}"),
                    None => "press / to enter a command, ? for help".to_string(),
                };
                let input_widget = Paragraph::new(input_text).block(input_block.clone());
                frame.render_widget(input_widget, ia);

                if let Some(buf) = &command_buf {
                    let x = ia.x + 2 + buf.chars().count() as u16;
                    let y = ia.y + 1;
                    frame.set_cursor_position((x, y));
                }

                if show_help {
                    let popup_area = centered_rect(50, 50, frame.area());
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
                                KeyCode::Backspace => { buf.pop(); }
                                KeyCode::Char(c) => buf.push(c),
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') => break Ok(()),
                                KeyCode::Char('p') => {
                                    let cmd = if state.power == Some(true) { "PWOFF" } else { "PWON" };
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