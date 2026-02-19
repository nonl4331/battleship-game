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
use gui::Application;

use crate::gui::ShipPlacement;

#[derive(Debug)]
enum Status {
    Won,
    Loss,
}

#[derive(Debug, Clone)]
struct Ship {
    pub pos: Vec<usize>,
}

impl Ship {
    const HIT: usize = usize::MAX;
    fn sunk(&self) -> bool {
        self.pos.iter().all(|v| *v == Self::HIT)
    }
    fn intersects_ship(&self, ship: &Self) -> bool {
        for pos in &ship.pos {
            if self.pos.contains(pos) {
                return true;
            }
        }
        false
    }
    fn create_with_pos_and_rotation(x: u8, y: u8, len: u8, down: bool) -> Option<Self> {
        let mut pos = Vec::new();
        let idx = (x + y * 10) as usize;

        for offset in 0..(len as usize) {
            if down {
                pos.push(offset * 10 + idx);
            } else {
                pos.push(offset + idx);
            }
        }

        Some(Self { pos })
    }
}

#[derive(Debug)]
struct Board {
    ships: [Ship; 5],
    your_attacks: [u8; 100],
    enemy_attacks: [u8; 100],
    pending_attack: (u8, u8),
    con: TcpStream,
}

impl Board {
    pub const MISS: u8 = 0;
    pub const HIT: u8 = 1;
    fn play_game(con: TcpStream, host: bool) -> Status {
        let mut ships: Vec<Ship> = Vec::new();
        for (name, len) in [
            ("Carrier", 5),
            ("Battleship", 4),
            ("Destroyer", 3),
            ("Submarine", 3),
            ("Patrol Boat", 2),
        ] {
            loop {
                let x = get_pos(&format!(
                    "please choose the x coordinate to put your {name} (length: {len}) [1-10]"
                ));
                let y = get_pos(&format!(
                    "please choose the y coordinate to put your {name} (length: {len}) [1-10]"
                ));

                let mut input = String::new();
                println!("Is your {name} (length: {len}) rotated? [y/n]");
                std::io::stdin().read_line(&mut input).unwrap();
                let down = match &input.trim().to_lowercase()[..] {
                    "y" => true,

                    "n" => false,
                    i => {
                        println!("Invalid input \"{i}\"! Please try again");
                        continue;
                    }
                };

                let Some(ship) = Ship::create_with_pos_and_rotation(x, y, len, down) else {
                    println!("Invalid ship position, please try again.");
                    continue;
                };

                if ships.iter().any(|s| s.intersects_ship(&ship)) {
                    println!("{name} intersects existing ship. Please try again.");
                    continue;
                }

                ships.push(ship);
                break;
            }
        }
        let mut b = Self {
            ships: ships.try_into().unwrap(),
            your_attacks: [0; 100],
            enemy_attacks: [0; 100],
            pending_attack: (0, 0),
            con,
        };

        // decide on which player goes first
        let first;
        if host {
            first = random_bool(0.5);
            b.con
                .write_all(&[u8::from(!first)])
                .expect("Failed to send move to server");
        } else {
            println!("Waiting for server to say who goes first.");
            let arr: [u8; 1] = b
                .con
                .read_array()
                .expect("Failed to get who goes first from server");
            if arr[0] == 1 {
                first = true;
            } else if arr[0] == 0 {
                first = false;
            } else {
                panic!("Server sent malformed data.");
            }
        }

        // play first move if applicable:
        if first {
            println!("Our turn");
            b.con.write_all(&[4]).unwrap();
            b.make_move();
        } else {
            println!("Enemy turn");
        }

        loop {
            let status: [u8; 1] = b
                .con
                .read_array()
                .expect("Failed to get message from server");

            match status[0] {
                0 => {
                    println!("Miss!");
                    b.update_pending(2);
                    Board::print_board(&b.your_attacks);
                    if let Err(loss) = b.receive_move() {
                        return loss;
                    }
                    b.make_move();
                } // miss, em, your move
                1 => {
                    println!("Hit!");
                    b.update_pending(1);
                    Board::print_board(&b.your_attacks);
                    if let Err(loss) = b.receive_move() {
                        return loss;
                    }
                    b.make_move();
                } // hit, em, your move
                2 => {
                    println!("Sunk!");
                    b.update_pending(1);
                    Board::print_board(&b.your_attacks);
                    if let Err(loss) = b.receive_move() {
                        return loss;
                    }
                    b.make_move();
                } // sunk, em, your move
                3 => {
                    println!("Sunk, you win!");
                    b.update_pending(1);
                    Board::print_board(&b.your_attacks);
                    return Status::Won;
                } // sunk, you win
                4 => {
                    if let Err(loss) = b.receive_move() {
                        return loss;
                    }
                    b.make_move();
                } // em, first move
                m => panic!("Invalid message from server: {m}"),
            }
        }
    }
    fn make_move(&mut self) {
        loop {
            let x = get_pos("Please choose the x coordinate of your attack [1-10]");
            let y = get_pos("Please choose the y coordinate of your attack [1-10]");

            if (x == self.pending_attack.0 && y == self.pending_attack.1)
                || self.your_attacks[((x - 1) + (y - 1) * 10) as usize] != 0
            {
                println!("Invalid coordinate, already attacked @ ({x}, {y})");
                continue;
            }
            self.pending_attack = (x, y);

            self.con
                .write_all(&[x, y])
                .expect("Failed to send move to server");

            break;
        }
    }
    fn receive_move(&mut self) -> Result<(), Status> {
        let [x, y]: [u8; 2] = self
            .con
            .read_array()
            .expect("Failed to get message from server");

        let mut hit = false;
        let mut sunk = false;
        assert!((1..=10).contains(&x) && (1..=10).contains(&y));

        let idx = ((x - 1) + (y - 1) * 10) as usize;
        for ship in &mut self.ships {
            if let Some(i) = ship.pos.iter().position(|&i| i == idx) {
                ship.pos[i] = usize::MAX;
                hit = true;
                sunk = ship.sunk();
                break;
            }
        }

        let board_attack = if sunk && self.ships.iter().all(Ship::sunk) {
            self.con.write_all(&[3]).unwrap();
            return Err(Status::Loss);
        } else if hit {
            Self::HIT
        } else {
            Self::MISS
        };

        if board_attack != Board::MISS {
            self.enemy_attacks[((x - 1) + (y - 1) * 10) as usize] = 1;
        }
        println!("Your ships:");
        Self::print_board(&self.enemy_attacks);
        self.con.write_all(&[board_attack]).unwrap();
        Ok(())
    }
    fn print_board(board: &[u8; 100]) {
        for line in 0..10 {
            for c in 0..10 {
                let c = match board[line * 10 + c] {
                    0 => '.',
                    1 => 'X',
                    2 => '#',
                    _ => unreachable!(),
                };
                print!("{c}");
            }
            println!();
        }
    }
    fn update_pending(&mut self, attack_state: u8) {
        self.your_attacks
            [((self.pending_attack.0 - 1) + (self.pending_attack.1 - 1) * 10) as usize] =
            attack_state;
    }
}

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
                                        return Application::Game(Board {
                                            con,
                                            ships: ships
                                                .iter()
                                                .cloned()
                                                .collect::<Vec<Ship>>()
                                                .try_into()
                                                .unwrap(),
                                            your_attacks: [0; 100],
                                            enemy_attacks: [0; 100],
                                            pending_attack: (0, 0),
                                        });
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
    return;

    let mut host = false;
    // decide on hosting or not
    loop {
        println!("Do you want to host or join a battleships game? [h/j]");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        match &input.trim().to_lowercase()[..] {
            "h" => {
                host = true;
                break;
            }
            "j" => break,
            i => println!("Invalid input \"{i}\"! Please try again"),
        }
    }

    // establish connection
    let con;
    if host {
        // wait for connection to host
        let listener = TcpListener::bind("0.0.0.0:0").expect("Failed to bind to tcp port");
        println!(
            "Server bound on {}, waiting for connection.",
            listener.local_addr().unwrap()
        );
        (con, _) = listener.accept().expect("Failed to accept connection");
    } else {
        // connection to remote server
        loop {
            print!("Enter address of server you'd like to connect to: ");
            stdout().flush().unwrap();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            let Ok(c) = TcpStream::connect(input.trim()) else {
                println!("Failed to join to server: {}", input.trim());
                continue;
            };
            con = c;
            break;
        }
    }

    let result = Board::play_game(con, host);
    println!("{result:?}");
}

fn get_pos(msg: &str) -> u8 {
    loop {
        let mut input = String::new();
        println!("{msg}");
        std::io::stdin().read_line(&mut input).unwrap();
        match input.trim().parse::<u8>() {
            Ok(p) if p > 0 && p <= 10 => return p,
            _ => {
                println!("Invalid number: {input}. Input must be between 1 and 10");
            }
        }
    }
}
