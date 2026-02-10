#![feature(read_array)]
use std::{
    fmt,
    io::{Write, stdout, Read},
    net::{TcpListener, TcpStream},
};

use rand::random_bool;

#[derive(Debug)]
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
        if !((1..=10).contains(&x) && (1..=10).contains(&y) && len > 0) {
            return None;
        }

        let mut pos = Vec::new();
        let idx = ((x - 1) + (y - 1) * 10) as usize;

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
    enemy_attacks: Attacks,
}

impl Board {
    pub const MISS: u8 = 0;
    pub const HIT: u8 = 1;
    pub const SUNK: u8 = 2;
    pub const WIN: u8 = 3;
    fn from_ships(ships: [Ship; 5]) -> Self {
        Self {
            ships,
            enemy_attacks: Attacks::new(),
        }
    }
    fn attack(&mut self, x: u8, y: u8) -> u8 {
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

        if sunk && self.ships.iter().all(Ship::sunk) {
            Self::WIN
        } else if sunk {
            Self::SUNK
        } else if hit {
            Self::HIT
        } else {
            Self::MISS
        }
    }
}

#[derive(Debug)]
struct Attacks {
    board: [u8; 100],
    pending: (u8, u8),
}

impl Attacks {
    fn new() -> Self {
        Self {
            board: [0; 100],
            pending: (u8::MAX, u8::MAX),
        }
    }
    fn update_pending(&mut self, worked: u8) {
        let x = self.pending.0;
        let y = self.pending.1;
        assert!((1..=10).contains(&x) && (1..=10).contains(&y));
        let idx = ((x - 1) + (y - 1) * 10) as usize;
        self.board[idx] = worked;
    }
    fn set_pending(&mut self, x: u8, y: u8) {
        self.pending = (x, y);
    }
    fn move_exists(&self, x: u8, y: u8) -> bool {
        assert!((1..=10).contains(&x) && (1..=10).contains(&y));
        let idx = ((x - 1) + (y - 1) * 10) as usize;
        (x == self.pending.0 && y == self.pending.1) || self.board[idx] != 0
    }
}

impl fmt::Display for Attacks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in 0..10 {
            for c in 0..10 {
                let c = match self.board[line * 10 + c] {
                    0 => '.',
                    1 => 'X',
                    2 => '#',
                    _ => unreachable!(),
                };
                write!(f, "{c}")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

fn main() {
    let stdin = std::io::stdin();

    let mut host = false;
    // decide on hosting or not
    loop {
        println!("Do you want to host or join a battleships game? [h/j]");
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
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
    let mut con;
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
            stdin.read_line(&mut input).unwrap();
            let Ok(c) = TcpStream::connect(input.trim()) else {
                println!("Failed to join to server: {}", input.trim());
                continue;
            };
            con = c;
            break;
        }
    }

    // create board
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
            stdin.read_line(&mut input).unwrap();
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

    let mut board = Board::from_ships(ships.try_into().unwrap());

    // decide on which player goes first
    let first;
    if host {
        first = random_bool(0.5);
        con.write_all(&[u8::from(!first)])
            .expect("Failed to send move to server");
    } else {
        println!("Waiting for server to say who goes first.");
        let arr: [u8; 1] = con
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

    let mut moveset = Attacks::new();

    // play first move if applicable:
    if first {
        println!("Our turn");
        con.write_all(&[4]).unwrap();
        make_move(&mut con, &mut moveset);
    } else {
        println!("Enemy turn");
    }

    loop {
        let status: [u8; 1] = con.read_array().expect("Failed to get message from server");

        match status[0] {
            0 => {
                println!("Miss!");
                moveset.update_pending(2);
                println!("{moveset}");
                receive_move(&mut con, &mut board);
                make_move(&mut con, &mut moveset);
            } // miss, em, your move
            1 => {
                println!("Hit!");
                moveset.update_pending(1);
                println!("{moveset}");
                receive_move(&mut con, &mut board);
                make_move(&mut con, &mut moveset);
            } // hit, em, your move
            2 => {
                println!("Sunk!");
                moveset.update_pending(1);
                println!("{moveset}");
                receive_move(&mut con, &mut board);
                make_move(&mut con, &mut moveset);
            } // sunk, em, your move
            3 => {
                println!("Sunk, you win!");
                moveset.update_pending(1);
                println!("{moveset}");
                break;
            } // sunk, you win
            4 => {
                receive_move(&mut con, &mut board);
                make_move(&mut con, &mut moveset);
            } // em, first move
            m => panic!("Invalid message from server: {m}"),
        }
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

fn receive_move(con: &mut TcpStream, board: &mut Board) {
    let [x, y]: [u8; 2] = con.read_array().expect("Failed to get message from server");
    let board_attack = board.attack(x, y);
    if board_attack != Board::MISS {
        board.enemy_attacks.set_pending(x, y);
        board.enemy_attacks.update_pending(1);
    }
    println!("Your ships:\n{}", board.enemy_attacks);
    con.write_all(&[board_attack]).unwrap();
}

fn make_move(con: &mut TcpStream, moveset: &mut Attacks) {
    loop {
        let x = get_pos("Please choose the x coordinate of your attack [1-10]");
        let y = get_pos("Please choose the y coordinate of your attack [1-10]");

        if moveset.move_exists(x, y) {
            println!("Invalid coordinate, already attacked @ ({x}, {y})");
            continue;
        }
        moveset.set_pending(x, y);

        con.write_all(&[x, y])
            .expect("Failed to send move to server");
        println!("sent: {:?}", [x, y]);

        break;
    }
}
