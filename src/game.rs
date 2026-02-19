use std::{
    net::{TcpStream},
};

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
    pub fn create_with_pos_and_rotation(x: usize, y: usize, len: usize, down: bool) -> Self {
        let mut pos = Vec::new();
        let idx = x + y * 10;

        for offset in 0..len {
            if down {
                pos.push(offset * 10 + idx);
            } else {
                pos.push(offset + idx);
            }
        }

        Self { pos }
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
}
