#![feature(read_array)]
use std::{
    io::{Read, Write, stdout},
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use crossterm::event::{self, KeyCode};
use rand::random_bool;
use ratatui::Frame;
#[derive(Debug)]
pub enum Status {
    Won,
    Loss,
}

#[derive(Debug, Clone)]
pub struct Ship {
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
    pub fn create_with_pos_and_rotation(x: u8, y: u8, len: u8, down: bool) -> Option<Self> {
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
pub struct Board {
    pub ships: [Ship; 5],
    pub your_attacks: [u8; 100],
    pub enemy_attacks: [u8; 100],
    pending_attack: (u8, u8),
    con: TcpStream,
}

impl Board {
    pub fn from_con_ships(con: TcpStream, ships: [Ship; 5]) -> Self {
        Self { ships, con, your_attacks: [0; 100], enemy_attacks: [0; 100], pending_attack: (0, 0) }
    }
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
