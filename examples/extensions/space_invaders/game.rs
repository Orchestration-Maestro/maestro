//! Retained game records and one deterministic simulation tick.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use serde::{Deserialize, Serialize};

/// Number of columns in the playfield.
pub const GAME_WIDTH: i32 = 60;
/// Number of rows in the playfield.
pub const GAME_HEIGHT: i32 = 24;
/// Player's fixed row.
pub const PLAYER_Y: i32 = GAME_HEIGHT - 2;
/// Time between simulation ticks in milliseconds.
pub const TICK_MS: u64 = 50;

/// A moving shot, travelling up for the player or down for an alien.
#[derive(Clone, Serialize, Deserialize)]
pub struct Bullet {
    /// Column.
    pub x: i32,
    /// Row.
    pub y: i32,
    /// Vertical velocity, -1 or 1.
    pub direction: i32,
}
/// One member of the alien formation.
#[derive(Clone, Serialize, Deserialize)]
pub struct Alien {
    /// Center column.
    pub x: i32,
    /// Row.
    pub y: i32,
    /// Sprite and point-value index.
    #[serde(rename = "type")]
    pub kind: usize,
    /// Whether it remains in play.
    pub alive: bool,
}
/// A destructible bunker.
#[derive(Clone, Serialize, Deserialize)]
pub struct Shield {
    /// Left column.
    pub x: i32,
    /// Three rows of four segments.
    pub segments: [[bool; 4]; 3],
}
/// The player's position and remaining lives.
#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    /// Center column.
    pub x: i32,
    /// Remaining hits.
    pub lives: u32,
}
/// A complete saved game, excluding held keys and transient input counters.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    /// Player ship.
    pub player: Player,
    /// Formation in row order.
    pub aliens: Vec<Alien>,
    /// Horizontal velocity.
    pub alien_direction: i32,
    /// Ticks since the last formation movement.
    pub alien_move_counter: u32,
    /// Ticks between formation movements.
    pub alien_move_delay: u32,
    /// Whether the next movement descends.
    pub alien_dropping: bool,
    /// Shots still in the playfield.
    pub bullets: Vec<Bullet>,
    /// Destructible bunkers.
    pub shields: Vec<Shield>,
    /// Points earned in this game.
    pub score: u32,
    /// Highest score retained across restarts.
    pub high_score: u32,
    /// Current difficulty.
    pub level: u32,
    /// Whether the player lost.
    pub game_over: bool,
    /// Whether every alien was destroyed.
    pub victory: bool,
    /// Ticks since the last alien shooting attempt.
    pub alien_shoot_counter: u32,
}
/// Held movement and firing keys.
#[derive(Default)]
pub struct KeyState {
    /// Move left.
    pub left: bool,
    /// Move right.
    pub right: bool,
    /// Fire while held.
    pub fire: bool,
}
impl GameState {
    /// Creates the original formation, bunkers and player for a new level.
    pub fn initial(high_score: u32, level: u32) -> Self {
        Self {
            player: Player {
                x: GAME_WIDTH / 2,
                lives: 3,
            },
            aliens: (0..5)
                .flat_map(|row| {
                    (0..11).map(move |col| Alien {
                        x: 4 + col * 5,
                        y: 2 + row * 2,
                        kind: if row == 0 { 2 } else { usize::from(row < 3) },
                        alive: true,
                    })
                })
                .collect(),
            alien_direction: 1,
            alien_move_counter: 0,
            alien_move_delay: 20_u32.saturating_sub(level.saturating_mul(2)).max(5),
            alien_dropping: false,
            bullets: Vec::new(),
            shields: [8, 22, 36, 50]
                .map(|x| Shield {
                    x,
                    segments: [[true; 4], [true; 4], [true, false, false, true]],
                })
                .into(),
            score: 0,
            high_score,
            level,
            game_over: false,
            victory: false,
            alien_shoot_counter: 0,
        }
    }
    /// Moves the formation, reverses at its edge and descends on the following movement.
    fn move_aliens(&mut self) {
        let alive: Vec<_> = self.aliens.iter_mut().filter(|a| a.alive).collect();
        if alive.is_empty() {
            return;
        }
        let count = alive.len();
        if self.alien_dropping {
            for alien in alive {
                alien.y += 1;
                if alien.y >= PLAYER_Y - 1 {
                    self.game_over = true;
                    return;
                }
            }
            self.alien_dropping = false;
        } else if alive.iter().any(|a| {
            (self.alien_direction == 1 && a.x >= GAME_WIDTH - 3)
                || (self.alien_direction == -1 && a.x <= 2)
        }) {
            self.alien_direction *= -1;
            self.alien_dropping = true;
        } else {
            for alien in alive {
                alien.x += self.alien_direction;
            }
        }
        self.alien_move_delay = match count {
            0..=5 => 1,
            6..=10 => 2,
            11..=20 => 3,
            _ => self.alien_move_delay,
        };
    }
    /// Shoots from the bottom alien in a supplied randomly selected column.
    fn alien_shoot(&mut self, choose: impl FnOnce(usize) -> usize) {
        let mut shooters: Vec<&Alien> = Vec::new();
        for alien in self.aliens.iter().filter(|a| a.alive) {
            if let Some(bottom) = shooters.iter_mut().find(|a| a.x == alien.x) {
                if alien.y > bottom.y {
                    *bottom = alien;
                }
            } else {
                shooters.push(alien);
            }
        }
        if !shooters.is_empty() && self.bullets.iter().filter(|b| b.direction == 1).count() < 3 {
            let shooter = shooters[choose(shooters.len()) % shooters.len()];
            self.bullets.push(Bullet {
                x: shooter.x,
                y: shooter.y + 1,
                direction: 1,
            });
        }
    }
    /// Applies hits and removes the shots that hit any target.
    fn check_collisions(&mut self) {
        self.bullets.retain(|bullet| {
            let mut hit = false;
            if bullet.direction == -1 {
                for alien in &mut self.aliens {
                    if alien.alive && (bullet.x - alien.x).abs() <= 1 && bullet.y == alien.y {
                        alien.alive = false;
                        hit = true;
                        self.score += [10, 20, 30].get(alien.kind).copied().unwrap_or(0);
                        self.high_score = self.high_score.max(self.score);
                        break;
                    }
                }
            } else if bullet.direction == 1
                && (bullet.x - self.player.x).abs() <= 1
                && bullet.y == PLAYER_Y
            {
                hit = true;
                self.player.lives = self.player.lives.saturating_sub(1);
                self.game_over |= self.player.lives == 0;
            }
            for shield in &mut self.shields {
                if let Some(segment) = shield.segment_mut(bullet.x, bullet.y)
                    && *segment
                {
                    *segment = false;
                    hit = true;
                }
            }
            !hit
        });
    }
    /// Advances the world once, using a controlled column selection for alien shots.
    pub fn tick(&mut self, choose: impl FnOnce(usize) -> usize) {
        for bullet in &mut self.bullets {
            bullet.y += bullet.direction;
        }
        self.bullets.retain(|b| (0..GAME_HEIGHT).contains(&b.y));
        self.alien_move_counter += 1;
        if self.alien_move_counter >= self.alien_move_delay {
            self.alien_move_counter = 0;
            self.move_aliens();
        }
        self.alien_shoot_counter += 1;
        if self.alien_shoot_counter >= 30 {
            self.alien_shoot_counter = 0;
            self.alien_shoot(choose);
        }
        self.check_collisions();
        self.victory = self.aliens.iter().all(|a| !a.alive);
    }
}
impl Shield {
    /// The segment at one playfield position, if it is inside this bunker.
    pub fn segment_mut(&mut self, x: i32, y: i32) -> Option<&mut bool> {
        let x = usize::try_from(x - self.x).ok()?;
        let y = usize::try_from(y - (PLAYER_Y - 5)).ok()?;
        self.segments.get_mut(y)?.get_mut(x)
    }
    /// Whether a bunker segment occupies this playfield position.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (Ok(x), Ok(y)) = (
            usize::try_from(x - self.x),
            usize::try_from(y - (PLAYER_Y - 5)),
        ) else {
            return false;
        };
        self.segments
            .get(y)
            .and_then(|row| row.get(x))
            .copied()
            .unwrap_or(false)
    }
}
