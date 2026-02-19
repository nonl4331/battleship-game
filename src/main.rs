#![feature(read_array)]
use std::{
    io::{Read, Write, stdout},
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use crossterm::event::{self, KeyCode};
use rand::random_bool;
use ratatui::Frame;

mod gui;
mod game;
use game::*;
use gui::Application;

use crate::gui::ShipPlacement;


fn main() {
    let mut term = ratatui::init();
    let mut app = Application::new();

    loop {
        term.draw(|frame: &mut Frame| app.render(frame)).unwrap();

        if event::poll(Duration::from_millis(250)).unwrap() {
            if let Application::Host(ref listener) = app {
                if let Ok((stream, _)) = listener.accept() {
                    app = Application::place_ships(stream);
                }
            }
            if let event::Event::Key(key) = event::read().unwrap() {
                if key.code == KeyCode::Esc {
                    break;
                }
                take_mut::take(&mut app, |app| {
                    if let Application::PlaceShips(con, mut placements, mut ships, mut grid) = app {
                        let ship = placements.last_mut().unwrap();
                        match key.code {
                            KeyCode::Down
                                if ship.valid(ship.pos.0, ship.pos.1 + 1, ship.rotated) =>
                            {
                                ship.pos.1 += 1;
                            }
                            KeyCode::Up
                                if ship.valid(
                                    ship.pos.0,
                                    ship.pos.1.saturating_sub(1),
                                    ship.rotated,
                                ) =>
                            {
                                ship.pos.1 = ship.pos.1.saturating_sub(1);
                            }
                            KeyCode::Left
                                if ship.valid(
                                    ship.pos.0.saturating_sub(1),
                                    ship.pos.1,
                                    ship.rotated,
                                ) =>
                            {
                                ship.pos.0 = ship.pos.0.saturating_sub(1);
                            }
                            KeyCode::Right
                                if ship.valid(ship.pos.0 + 1, ship.pos.1, ship.rotated) =>
                            {
                                ship.pos.0 += 1;
                            }
                            KeyCode::Char('r') | KeyCode::Char('R')
                                if ship.valid(ship.pos.0, ship.pos.1, !ship.rotated) =>
                            {
                                ship.rotated = !ship.rotated;
                            }
                            KeyCode::Enter => {
                                if let Some(ship) = ship.create_ship(&mut grid) {
                                    ships.push(ship);
                                    placements.pop();
                                    if ships.len() != 5 - placements.len() {
                                        panic!("{} | {}", ships.len(), placements.len());
                                    };
                                    if placements.is_empty() {
                                        return Application::Game(Board::from_con_ships(
                                            con,
                                            ships
                                                .iter()
                                                .cloned()
                                                .collect::<Vec<Ship>>()
                                                .try_into()
                                                .unwrap(),
                                        ));
                                    } else {
                                        placements.last_mut().unwrap().occupied = grid.clone();
                                    }
                                }
                            }
                            _ => {}
                        }
                        Application::PlaceShips(con, placements, ships, grid)
                    } else {
                        app
                    }
                });
                if let Application::Menu(_, ref mut ls, _) = app {
                    match key.code {
                        KeyCode::Down => ls.select_next(),
                        KeyCode::Up => ls.select_previous(),
                        KeyCode::Enter if matches!(ls.selected(), Some(0)) => {
                            let listener = TcpListener::bind("0.0.0.0:0")
                                .expect("TODO: implement error handling here");
                            listener
                                .set_nonblocking(true)
                                .expect("Failed to set nonblocking mode on TcpListener");
                            app = Application::Host(listener);
                        }
                        KeyCode::Enter if matches!(ls.selected(), Some(1)) => {
                            app = Application::ConnectToHost(String::new(), 0, String::new());
                        }
                        KeyCode::Enter if matches!(ls.selected(), Some(2)) => {
                            app = Application::Help;
                        }
                        KeyCode::Enter if matches!(ls.selected(), Some(3)) => {
                            break;
                        }
                        _ => {}
                    }
                }
                use std::str::FromStr;
                if let Application::ConnectToHost(ref mut s, ref mut cursor, ref mut connection) =
                    app
                {
                    match key.code {
                        KeyCode::Left => {
                            *cursor = cursor.saturating_sub(1);
                        }
                        KeyCode::Right => *cursor = (*cursor + 1).min(s.chars().count()),
                        KeyCode::Backspace => {
                            if *cursor != 0 {
                                *s = s
                                    .chars()
                                    .take(*cursor - 1)
                                    .chain(s.chars().skip(*cursor))
                                    .collect();
                                *cursor = cursor.saturating_sub(1);
                            }
                        }
                        KeyCode::Char(v) => {
                            s.insert(
                                s.char_indices()
                                    .map(|(i, _)| i)
                                    .nth(*cursor)
                                    .unwrap_or(s.len()),
                                v,
                            );
                            *cursor = (*cursor + 1).min(s.chars().count());
                        }
                        KeyCode::Enter if !s.is_empty() => {
                            if let Ok(addr) = SocketAddr::from_str(&s) {
                                *connection = format!("Attempting to connect to: {}", addr);
                                match TcpStream::connect(addr) {
                                    Ok(con) => {
                                        app = Application::place_ships(con);
                                    }
                                    Err(e) => {
                                        *connection =
                                            format!("Failed to connect to: {} - {e}", addr);
                                    }
                                }
                            } else {
                                *connection = format!("Invalid address!");
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    ratatui::restore();
}

