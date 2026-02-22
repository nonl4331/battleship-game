#![feature(read_array)]
use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use crossterm::event::{self, KeyCode};
use ratatui::Frame;

mod game;
mod gui;
use game::*;
use gui::Application;

fn main() {
    let mut term = ratatui::init();
    let mut app = Application::new();

    loop {
        if let Application::Break = app {
            break;
        }

        term.draw(|frame: &mut Frame| app.render(frame)).unwrap();

        if event::poll(Duration::from_millis(250)).unwrap() {
            // transition from host -> place ships (pregame) when connection is established
            if let Application::Host(ref listener) = app {
                if let Ok((stream, _)) = listener.accept() {
                    app = Application::place_ships(stream);
                }
            }

            if let event::Event::Key(key) = event::read().unwrap() {
                if key.code == KeyCode::Esc {
                    break;
                }
                match app {
                    Application::Menu(..) => menu(&mut app, key.code),
                    Application::ConnectToHost(..) => connect_to_host(&mut app, key.code),
                    Application::PlaceShips(..) => place_ships(&mut app, key.code),
                    Application::Game(..) => {}
                    Application::Help | Application::Host(..) | Application::Break => {}
                }
            }
        }
    }
    ratatui::restore();
}

fn menu(app: &mut Application, code: KeyCode) {
    let Application::Menu(_, ls, _) = app else {
        unreachable!();
    };
    match code {
        KeyCode::Down => ls.select_next(),
        KeyCode::Up => ls.select_previous(),
        KeyCode::Enter if matches!(ls.selected(), Some(0)) => {
            let listener =
                TcpListener::bind("0.0.0.0:0").expect("TODO: implement error handling here");
            listener
                .set_nonblocking(true)
                .expect("Failed to set nonblocking mode on TcpListener");
            *app = Application::Host(listener);
        }
        KeyCode::Enter if matches!(ls.selected(), Some(1)) => {
            *app = Application::ConnectToHost(String::new(), 0, String::new());
        }
        KeyCode::Enter if matches!(ls.selected(), Some(2)) => {
            *app = Application::Help;
        }
        KeyCode::Enter if matches!(ls.selected(), Some(3)) => {
            *app = Application::Break;
        }
        _ => {}
    }
}

fn connect_to_host(app: &mut Application, code: KeyCode) {
    use std::str::FromStr;
    let Application::ConnectToHost(s, cursor, connection) = app else {
        unreachable!();
    };
    match code {
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
                        *app = Application::place_ships(con);
                    }
                    Err(e) => {
                        *connection = format!("Failed to connect to: {} - {e}", addr);
                    }
                }
            } else {
                *connection = format!("Invalid address!");
            }
        }
        _ => {}
    }
}

fn place_ships(app: &mut Application, code: KeyCode) {
    take_mut::take(app, |app| {
        let Application::PlaceShips(con, mut placements, mut ships, mut grid) = app else {
            unreachable!();
        };
        let ship = placements.last_mut().unwrap();
        match code {
            KeyCode::Down if ship.valid(ship.pos.0, ship.pos.1 + 1, ship.rotated) => {
                ship.pos.1 += 1;
            }
            KeyCode::Up if ship.valid(ship.pos.0, ship.pos.1.saturating_sub(1), ship.rotated) => {
                ship.pos.1 = ship.pos.1.saturating_sub(1);
            }
            KeyCode::Left if ship.valid(ship.pos.0.saturating_sub(1), ship.pos.1, ship.rotated) => {
                ship.pos.0 = ship.pos.0.saturating_sub(1);
            }
            KeyCode::Right if ship.valid(ship.pos.0 + 1, ship.pos.1, ship.rotated) => {
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
    });
}
