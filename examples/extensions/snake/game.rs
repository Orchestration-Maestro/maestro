//! Snake positions, food and the state stored in session entries.
use maestro_extensions_wasm::ExtensionResult;
use serde::{Deserialize, Serialize};

/// Columns in the game, independent of the viewport.
pub const GAME_WIDTH: i32 = 40;
/// Rows in the game.
pub const GAME_HEIGHT: i32 = 15;
/// One game-cell coordinate.
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}
/// Direction applied at a tick boundary.
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Toward the top edge.
    Up,
    /// Toward the bottom edge.
    Down,
    /// Toward the left edge.
    Left,
    /// Toward the right edge.
    Right,
}
/// Complete persisted game, without timers or terminal handles.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    /// Head first, followed by the body.
    pub snake: Vec<Point>,
    /// Current food position.
    pub food: Point,
    /// Last applied direction.
    pub direction: Direction,
    /// Direction queued by input.
    pub next_direction: Direction,
    /// Ten points for each eaten food.
    pub score: u64,
    /// Whether movement has ended.
    pub game_over: bool,
    /// Highest score retained across restarts.
    pub high_score: u64,
}
impl GameState {
    /// Starts at the board's center, keeping food off the whole body.
    pub fn new(high_score: u64) -> Self {
        let snake = vec![
            Point { x: 20, y: 7 },
            Point { x: 19, y: 7 },
            Point { x: 18, y: 7 },
        ];
        let food = spawn_food(&snake);
        Self {
            snake,
            food,
            direction: Direction::Right,
            next_direction: Direction::Right,
            score: 0,
            game_over: false,
            high_score,
        }
    }
    /// Rejects saved coordinates that cannot be rendered or moved safely.
    pub fn validate(&self) -> ExtensionResult<()> {
        if self.snake.is_empty()
            || self.snake.len() > 600
            || self.snake.iter().chain([&self.food]).any(|p| !inside(*p))
        {
            return Err("Invalid Snake save".into());
        }
        Ok(())
    }
    /// Advances once, preserving the board when a wall or body collision ends play.
    pub fn tick(&mut self) {
        if self.game_over {
            return;
        }
        self.direction = self.next_direction;
        let mut head = self.snake[0];
        match self.direction {
            Direction::Up => head.y -= 1,
            Direction::Down => head.y += 1,
            Direction::Left => head.x -= 1,
            Direction::Right => head.x += 1,
        }
        if !inside(head) || self.snake.contains(&head) {
            self.game_over = true;
            return;
        }
        self.snake.insert(0, head);
        if head == self.food {
            self.score = self.score.saturating_add(10);
            self.high_score = self.high_score.max(self.score);
            // A filled board has no food location; stop instead of retrying forever.
            if self.snake.len() == 600 {
                self.game_over = true;
            } else {
                self.food = spawn_food(&self.snake);
            }
        } else {
            self.snake.pop();
        }
    }
    /// Queues a turn unless it reverses the direction of the current tick.
    pub fn turn(&mut self, direction: Direction) {
        let opposite = matches!(
            (self.direction, direction),
            (Direction::Up, Direction::Down)
                | (Direction::Down, Direction::Up)
                | (Direction::Left, Direction::Right)
                | (Direction::Right, Direction::Left)
        );
        if !opposite {
            self.next_direction = direction;
        }
    }
}
/// Whether a point lies within the authored board.
fn inside(point: Point) -> bool {
    (0..GAME_WIDTH).contains(&point.x) && (0..GAME_HEIGHT).contains(&point.y)
}
/// Samples the host's random source until it selects an unoccupied cell.
fn spawn_food(snake: &[Point]) -> Point {
    loop {
        let point = Point {
            x: i32::try_from(maestro_extensions_wasm::wasi::random::random::get_random_u64() % 40)
                .unwrap_or(0),
            y: i32::try_from(maestro_extensions_wasm::wasi::random::random::get_random_u64() % 15)
                .unwrap_or(0),
        };
        if !snake.contains(&point) {
            return point;
        }
    }
}
