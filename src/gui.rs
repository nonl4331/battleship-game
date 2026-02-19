use std::net::{TcpListener, TcpStream};

use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Offset, Position, Rect},
    style::{Modifier, Style, Stylize, palette::tailwind},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Widget},
};

use crate::{Board, Ship};
pub enum Application<'a> {
    Menu(Vec<ListItem<'a>>, ListState, Layout),
    Host(TcpListener),
    ConnectToHost(String, usize, String),
    PlaceShips(TcpStream, Vec<ShipPlacement>, Vec<Ship>, [bool; 100]),
    Game(Board),
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
            Self::PlaceShips(_stream, ship_placements, _ships, _) => {
                frame.render_widget(
                    Paragraph::new("").block(Block::bordered().title("Game")),
                    frame.area(),
                );
                //frame.render_widget(ship.table(), frame.area().centered(Constraint::Length(30), Constraint::Length(30)));
                frame.render_widget(ship_placements.last().unwrap(), frame.area());
            }
            Self::Game(_) => {
                todo!();
            }
            Self::Help => {
                frame.render_widget(
                    Paragraph::new("").block(Block::bordered().title("Help")),
                    frame.area(),
                );
                frame.render_widget(
                    Paragraph::new("HELP GOES HERE")
                        .alignment(ratatui::layout::HorizontalAlignment::Center),
                    frame.area().centered_vertically(Constraint::Length(1)),
                );
            }
        }
    }
}

impl<'a> Application<'a> {
    pub fn place_ships(con: TcpStream) -> Self {
        Self::PlaceShips(
            con,
            vec![
                ShipPlacement::new(2, [false; 100]),
                ShipPlacement::new(3, [false; 100]),
                ShipPlacement::new(3, [false; 100]),
                ShipPlacement::new(4, [false; 100]),
                ShipPlacement::new(5, [false; 100]),
            ],
            Vec::new(),
            [false; 100],
        )
    }
}

pub struct ShipPlacement {
    pub pos: (usize, usize),
    pub length: usize,
    pub rotated: bool,
    pub occupied: [bool; 100],
}

impl ShipPlacement {
    pub fn new(length: usize, occupied: [bool; 100]) -> Self {
        Self {
            pos: (0, 0),
            length,
            rotated: false,
            occupied,
        }
    }
    pub fn valid(&self, x: usize, y: usize, rotated: bool) -> bool {
        (!rotated && x + self.length <= 10 && y < 10)
            || (rotated && y + self.length <= 10 && x < 10)
    }
    pub fn inship(&self, x: usize, y: usize) -> bool {
        (!self.rotated && x >= self.pos.0 && x - self.pos.0 < self.length && self.pos.1 == y)
            || (self.rotated && y >= self.pos.1 && y - self.pos.1 < self.length && self.pos.0 == x)
    }
    pub fn create_ship(&self, other: &mut [bool; 100]) -> Option<Ship> {
        let idx = self.pos.0 + 10 * self.pos.1;
        if !self.valid(self.pos.0, self.pos.1, self.rotated) {
            return None;
        }
        if self.rotated {
            for offset in 0..self.length {
                if other[idx + 10 * offset] {
                    return None;
                }
            }
            for offset in 0..self.length {
                other[idx + 10 * offset] = true;
            }
        } else {
            if other[idx..idx + self.length].iter().any(|f| *f) {
                return None;
            }
            for e in &mut other[idx..idx + self.length] {
                *e = true;
            }
        }
        return Some(
            Ship::create_with_pos_and_rotation(
                self.pos.0 as u8,
                self.pos.1 as u8,
                self.length as u8,
                self.rotated,
            )
            .unwrap(),
        );
    }
}

impl Widget for &ShipPlacement {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match (area.width, area.height) {
            (12.., 12..) => {
                let space = area.centered(Constraint::Length(10), Constraint::Length(10));
                Block::new()
                    .borders(Borders::all())
                    .title("Place Ship")
                    .render(
                        area.centered(Constraint::Length(12), Constraint::Length(12)),
                        buf,
                    );
                for line in 0..10 {
                    let mut spans = Vec::new();
                    for col in 0..10 {
                        let idx = col + line * 10;
                        let mut colour = tailwind::WHITE;
                        if self.occupied[idx] {
                            colour = tailwind::RED.c500;
                        }
                        if col == self.pos.0 && line == self.pos.1 {
                            colour = tailwind::GREEN.c500;
                        } else if self.inship(col, line) {
                            colour = tailwind::GREEN.c900;
                        }
                        if self.occupied[idx] {
                            spans.push(Span::raw("X").fg(colour));
                        } else {
                            spans.push(Span::raw("•").fg(colour));
                        }
                    }
                    buf.set_line(space.x, space.y + line as u16, &Line::from(spans), 21);
                }
            }
            _ => buf.set_string(area.x, area.y, "NO SPACE FOR GRID", Style::new().bold()),
        }
    }
}

impl Widget for &Board {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match (area.width, area.height) {
            (23.., 14..) => {
                todo!();
            }
            _ => buf.set_string(area.x, area.y, "NO SPACE FOR GRID", Style::new().bold()),
        }
    }
}
