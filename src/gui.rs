use std::{
    net::{TcpListener, TcpStream},
};

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Offset, Position},
    style::{Modifier, Style, palette::tailwind},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
pub enum Application<'a> {
    Menu(Vec<ListItem<'a>>, ListState, Layout),
    Host(TcpListener),
    ConnectToHost(String, usize, String),
    PlaceShips(TcpStream),
    Help,
}

impl<'a> Application<'a> {
    pub fn new() -> Self {
        use ratatui::prelude::Stylize;
        let list_items = ["Host Game", "Join Game", "Help", "Exit"]
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                ListItem::new(s).bg(if i % 2 == 0 {
                    tailwind::CYAN.c500
                } else {
                    tailwind::GRAY.c500
                })
            })
            .collect();

        Self::Menu(
            list_items,
            ListState::default().with_selected(Some(0)),
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]),
        )
    }
    pub fn render(&mut self, frame: &mut Frame) {
        match self {
            Self::Menu(list_items, ls, lay) => {
                let a = lay.areas::<2>(frame.area())[0];
                let list = List::new(list_items.clone())
                    .block(
                        Block::new()
                            .title(Line::raw("Main Menu").centered())
                            .borders(Borders::all()),
                    )
                    .highlight_style(
                        Style::new()
                            .bg(tailwind::SLATE.c800)
                            .add_modifier(Modifier::BOLD),
                    );

                frame.render_stateful_widget(list, a, ls)
            }
            Self::Host(listener) => {
                frame.render_widget(
                    Paragraph::new("").block(Block::bordered().title("Hosting instance")),
                    frame.area(),
                );

                frame.render_widget(
                    Paragraph::new(format!(
                        "Waiting for connection on {}",
                        listener.local_addr().unwrap()
                    ))
                    .alignment(ratatui::layout::HorizontalAlignment::Center),
                    frame.area().centered_vertically(Constraint::Length(1)),
                );
            }
            Self::ConnectToHost(input, cursor, connection) => {
                let l = Layout::horizontal([
                    Constraint::Percentage(25),
                    Constraint::Min(35),
                    Constraint::Percentage(25),
                ]);
                frame.render_widget(
                    Paragraph::new("").block(Block::bordered().title("Connect to Host")),
                    frame.area(),
                );
                let input_area =
                    frame.area().layout::<3>(&l)[1].centered_vertically(Constraint::Length(3));
                frame.render_widget(
                    Paragraph::new(input.as_str()).block(Block::bordered().title("Peer Address")),
                    input_area,
                );
                frame.render_widget(
                    Paragraph::new(connection.as_str()),
                    input_area.offset(Offset::new(1, 3)),
                );
                frame.set_cursor_position(Position::new(
                    input_area.x + *cursor as u16 + 1,
                    input_area.y + 1,
                ));
            }
            Self::PlaceShips(_stream) => {
                frame.render_widget(
                    Paragraph::new("").block(Block::bordered().title("Game")),
                    frame.area(),
                );
                unimplemented!()
            },
            Self::Help => {
                frame.render_widget(
                    Paragraph::new("").block(Block::bordered().title("Help")),
                    frame.area(),
                );
                frame.render_widget(
                    Paragraph::new(
                        "HELP GOES HERE"
                    )
                    .alignment(ratatui::layout::HorizontalAlignment::Center),
                    frame.area().centered_vertically(Constraint::Length(1)),
                );

            },
        }
    }
}
