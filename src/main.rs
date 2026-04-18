#![feature(read_array)]
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use crossterm::event::{self, KeyCode};
use ratatui::Frame;

mod game;
mod gui;
use game::{Board, Ship};
use gui::Application;

use crate::gui::TurnState;

fn main() {
    let mut term = ratatui::init();
    let mut app = Application::new();

    loop {
        if let Application::Break = app {
            break;
        }

        term.draw(|frame: &mut Frame| app.render(frame)).unwrap();

        // transition from host -> place ships (pregame) when connection is established
        if let Application::Host(ref listener, first) = app
            && let Ok((mut stream, _)) = listener.accept()
        {
            stream
                .write_all(&[u8::from(!first)])
                .expect("failed to send move to server");
            app = Application::place_ships(stream, first);
        }

        let mut keycode = None;

        if event::poll(Duration::from_millis(250)).unwrap()
            && let event::Event::Key(key) = event::read().unwrap()
        {
            if key.code == KeyCode::Esc {
                break;
            }
            keycode = Some(key.code);
        }

        match app {
            Application::Menu(..) => menu(&mut app, keycode),
            Application::ConnectToHost(..) => connect_to_host(&mut app, keycode),
            Application::PlaceShips(..) => place_ships(&mut app, keycode),
            Application::Game(..) => game(&mut app, keycode),
            Application::Help | Application::Host(..) | Application::Break => {}
        }
    }
    ratatui::restore();
}

fn menu(app: &mut Application, code: Option<KeyCode>) {
    let Application::Menu(_, ls, _) = app else {
        unreachable!();
    };
    let Some(code) = code else {
        return;
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
            let first = rand::random_bool(0.5);
            *app = Application::Host(listener, first);
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

fn connect_to_host(app: &mut Application, code: Option<KeyCode>) {
    use std::str::FromStr;
    let Application::ConnectToHost(s, cursor, connection) = app else {
        unreachable!();
    };
    let Some(code) = code else {
        return;
    };
    match code {
        KeyCode::Left => {
            *cursor = cursor.saturating_sub(1);
        }
        KeyCode::Right => *cursor = (*cursor + 1).min(s.chars().count()),
        KeyCode::Backspace if *cursor != 0 => {
            *s = s
                .chars()
                .take(*cursor - 1)
                .chain(s.chars().skip(*cursor))
                .collect();
            *cursor = cursor.saturating_sub(1);
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
            if let Ok(addr) = SocketAddr::from_str(s) {
                *connection = format!("Attempting to connect to: {addr}");
                match TcpStream::connect(addr) {
                    Ok(mut con) => {
                        let first = con
                            .read_array::<1>()
                            .expect("Failed to get first move from host")[0]
                            != 0;
                        *app = Application::place_ships(con, first);
                    }
                    Err(e) => {
                        *connection = format!("Failed to connect to: {addr} - {e}");
                    }
                }
            } else {
                *connection = "Invalid address!".to_string();
            }
        }
        _ => {}
    }
}

fn place_ships(app: &mut Application, code: Option<KeyCode>) {
    let Some(code) = code else {
        return;
    };
    take_mut::take(app, |app| {
        let Application::PlaceShips(con, mut placements, mut ships, mut grid, turn) = app else {
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
            KeyCode::Char('r' | 'R') if ship.valid(ship.pos.0, ship.pos.1, !ship.rotated) => {
                ship.rotated = !ship.rotated;
            }
            KeyCode::Enter => {
                if let Some(ship) = ship.create_ship(&mut grid) {
                    ships.push(ship);
                    placements.pop();
                    assert!(
                        ships.len() == 5 - placements.len(),
                        "{} | {}",
                        ships.len(),
                        placements.len()
                    );
                    if placements.is_empty() {
                        return Application::Game(
                            Board::from_con_ships(con, ships.clone().try_into().unwrap()),
                            if turn {
                                TurnState::OurTurn
                            } else {
                                TurnState::EnemyTurn
                            },
                        );
                    }
                    placements.last_mut().unwrap().occupied = grid;
                }
            }
            _ => {}
        }
        Application::PlaceShips(con, placements, ships, grid, turn)
    });
}

fn game(app: &mut Application, code: Option<KeyCode>) {
    let Application::Game(board, turn) = app else {
        unreachable!();
    };

    board.con.set_nonblocking(true).unwrap();

    match code {
        Some(KeyCode::Down) if board.pending_attack.1 < 9 => {
            board.pending_attack.1 += 1;
        }
        Some(KeyCode::Up) if board.pending_attack.1 > 0 => {
            board.pending_attack.1 -= 1;
        }
        Some(KeyCode::Right) if board.pending_attack.0 < 9 => {
            board.pending_attack.0 += 1;
        }
        Some(KeyCode::Left) if board.pending_attack.0 > 0 => {
            board.pending_attack.0 -= 1;
        }
        _ => {}
    }
    match *turn {
        TurnState::OurTurn => match code {
            Some(KeyCode::Enter)
                if board.your_attacks
                    [(board.pending_attack.0 + board.pending_attack.1 * 10) as usize]
                    == 0 =>
            {
                board
                    .con
                    .write_all(&[board.pending_attack.0, board.pending_attack.1])
                    .unwrap();
                *turn = TurnState::WaitingReply;
            }
            _ => {}
        },
        TurnState::WaitingReply => {
            let status: [u8; 1] = match board.con.read_array().map_err(|e| e.kind()) {
                Ok(v) => v,
                Err(io::ErrorKind::WouldBlock) => return,
                Err(e) => panic!("{e}"),
            };
            let status = status[0];
            if status == 0 {
                board.your_attacks
                    [(board.pending_attack.0 + board.pending_attack.1 * 10) as usize] = 2;
            } else if status != 4 {
                board.your_attacks
                    [(board.pending_attack.0 + board.pending_attack.1 * 10) as usize] = 1;
            } else {
                panic!("WIN");
            }
            *turn = TurnState::EnemyTurn;
        }
        TurnState::EnemyTurn => {
            let attack: [u8; 2] = match board.con.read_array().map_err(|e| e.kind()) {
                Ok(v) => v,
                Err(io::ErrorKind::WouldBlock) => return,
                Err(e) => panic!("{e}"),
            };
            let idx = (attack[0] + 10 * attack[1]) as usize;

            let mut hit = false;
            let mut sunk = false;
            for ship in &mut board.ships {
                if let Some(i) = ship.pos.iter().position(|&i| i == idx) {
                    ship.pos[i] = usize::MAX;
                    hit = true;
                    sunk = ship.sunk();
                    break;
                }
            }

            if sunk && board.ships.iter().all(Ship::sunk) {
                board.con.write_all(&[4]).unwrap();
                panic!("LOSS");
            } else if hit {
                board.con.write_all(&[1]).unwrap();
                board.enemy_attacks[idx] = Board::HIT;
            } else {
                board.con.write_all(&[0]).unwrap();
                board.enemy_attacks[idx] = Board::MISS;
            }
            *turn = TurnState::OurTurn;
        }
    }
}
