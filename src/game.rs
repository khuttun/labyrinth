use instant::Instant;
use nalgebra_glm as glm;
use std::f32::consts::PI;

pub const BALL_R: f32 = 20.0;
pub const HOLE_R: f32 = 1.2 * BALL_R;
const MAX_ANGLE: f32 = PI / 32.0;
const ACCEL_COEFF: f32 = 300.0 * BALL_R / PI;
const BOUNCE_COEFF: f32 = 0.2;

#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl From<&json::JsonValue> for Point {
    fn from(json_val: &json::JsonValue) -> Point {
        Point {
            x: json_val["x"].as_f32().unwrap(),
            y: json_val["y"].as_f32().unwrap(),
        }
    }
}

impl From<&glm::Vec2> for Point {
    fn from(v: &glm::Vec2) -> Point {
        Point { x: v.x, y: v.y }
    }
}

impl From<&Point> for glm::Vec2 {
    fn from(p: &Point) -> glm::Vec2 {
        glm::vec2(p.x, p.y)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl From<&glm::Vec2> for Velocity {
    fn from(v: &glm::Vec2) -> Velocity {
        Velocity { x: v.x, y: v.y }
    }
}

impl From<&Velocity> for glm::Vec2 {
    fn from(v: &Velocity) -> glm::Vec2 {
        glm::vec2(v.x, v.y)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

impl From<&json::JsonValue> for Size {
    fn from(json_val: &json::JsonValue) -> Size {
        Size {
            w: json_val["w"].as_f32().unwrap(),
            h: json_val["h"].as_f32().unwrap(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Rect {
    pub pos: Point, // top-left corner
    pub size: Size,
}

impl From<&json::JsonValue> for Rect {
    fn from(json_val: &json::JsonValue) -> Rect {
        Rect {
            pos: Point::from(&json_val["pos"]),
            size: Size::from(&json_val["size"]),
        }
    }
}

impl Rect {
    // Check if self contains Point p
    fn contains(&self, p: glm::Vec2) -> bool {
        p.x >= self.pos.x
            && p.x < self.pos.x + self.size.w
            && p.y >= self.pos.y
            && p.y < self.pos.y + self.size.h
    }
}

/// Holds the information about single Labyrinth level.
/// Level coordinate system origin is in top-left corner of the board.
/// Positive x-axis direction is right and positive y-axis direction down.
pub struct Level {
    pub name: String,
    pub size: Size,
    pub start: Point,
    pub end: Rect,
    pub walls: Vec<Rect>,
    pub holes: Vec<Point>,
}

impl Level {
    pub fn from_json(json: &str) -> Level {
        let data = json::parse(json).unwrap();
        Level {
            name: String::from(data["name"].as_str().unwrap()),
            size: Size::from(&data["size"]),
            start: Point::from(&data["start"]),
            end: Rect::from(&data["end"]),
            walls: data["walls"].members().map(|j| Rect::from(j)).collect(),
            holes: data["holes"].members().map(|j| Point::from(j)).collect(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum State {
    InProgress,
    Won,
    Lost { hole: Point, t_lost: Instant },
}

/// Maintains the state of a single game of Labyrinth. A Game always starts with the ball in the
/// start position defined by the Level instance associated with the Game, and ends when the
/// ball either falls in a hole or reaches the goal area of the Level. The Game instance needs
/// to be periodically updated to advance the simulation.
pub struct Game {
    pub state: State,
    pub ball_pos: Point,
    pub ball_v: Velocity,
    pub angle_x: f32, // Board angle w.r.t. x-axis in radians
    pub angle_y: f32,
    pub level: Level,
    prev_update: Option<Instant>,
}

impl Game {
    pub fn new(lvl: Level) -> Game {
        Game {
            state: State::InProgress,
            ball_pos: lvl.start,
            ball_v: Velocity { x: 0.0, y: 0.0 },
            angle_x: 0.0,
            angle_y: 0.0,
            level: lvl,
            prev_update: None,
        }
    }

    pub fn rotate_x(&mut self, angle: f32) {
        self.angle_x = glm::clamp_scalar(self.angle_x + angle, -MAX_ANGLE, MAX_ANGLE);
    }

    pub fn rotate_y(&mut self, angle: f32) {
        self.angle_y = glm::clamp_scalar(self.angle_y + angle, -MAX_ANGLE, MAX_ANGLE);
    }

    pub fn update(&mut self, time: Instant) {
        match self.state {
            State::InProgress => (),
            _ => return, // nothing to update if the game is not in progress anymore
        }

        let (p, v) = self.detect_collisions(self.do_physics(time));

        if self.level.end.contains(p) {
            self.state = State::Won;
        }

        let hole = self
            .level
            .holes
            .iter()
            .find(|h| glm::distance(&p, &glm::Vec2::from(*h)) < HOLE_R);
        if hole.is_some() {
            self.state = State::Lost {
                hole: *hole.unwrap(),
                t_lost: time,
            };
        }

        self.prev_update = Some(time);
        self.ball_pos = Point::from(&p);
        self.ball_v = Velocity::from(&v);
    }

    pub fn reset_time(&mut self) {
        self.prev_update = None;
    }

    // Calculate ball (position, velocity) based on current time
    fn do_physics(&self, time: Instant) -> (glm::Vec2, glm::Vec2) {
        let dt = time
            .duration_since(self.prev_update.unwrap_or(time))
            .as_secs_f32();
        let v = glm::Vec2::from(&self.ball_v)
            + glm::vec2(self.angle_x, self.angle_y) * ACCEL_COEFF * dt;
        let p = glm::Vec2::from(&self.ball_pos) + v * dt;
        (p, v)
    }

    // Detect collisions to walls and update ball (position, velocity) accordingly
    fn detect_collisions(&self, pv: (glm::Vec2, glm::Vec2)) -> (glm::Vec2, glm::Vec2) {
        let pv = apply_collision_response(pv, detect_board_left_edge_collision(pv.0));
        let pv =
            apply_collision_response(pv, detect_board_right_edge_collision(pv.0, self.level.size));
        let pv = apply_collision_response(pv, detect_board_top_edge_collision(pv.0));
        let pv = apply_collision_response(
            pv,
            detect_board_bottom_edge_collision(pv.0, self.level.size),
        );
        return self.level.walls.iter().fold(pv, |pv, w| {
            apply_collision_response(pv, detect_wall_collision(pv.0, *w))
        });
    }
}

fn detect_board_left_edge_collision(ball_pos: glm::Vec2) -> Option<glm::Vec2> {
    if ball_pos.x < BALL_R {
        Some(glm::vec2(BALL_R - ball_pos.x, 0.0))
    } else {
        None
    }
}

fn detect_board_right_edge_collision(ball_pos: glm::Vec2, board_size: Size) -> Option<glm::Vec2> {
    if ball_pos.x >= board_size.w - BALL_R {
        Some(glm::vec2(board_size.w - BALL_R - ball_pos.x, 0.0))
    } else {
        None
    }
}

fn detect_board_top_edge_collision(ball_pos: glm::Vec2) -> Option<glm::Vec2> {
    if ball_pos.y < BALL_R {
        Some(glm::vec2(0.0, BALL_R - ball_pos.y))
    } else {
        None
    }
}

fn detect_board_bottom_edge_collision(ball_pos: glm::Vec2, board_size: Size) -> Option<glm::Vec2> {
    if ball_pos.y >= board_size.h - BALL_R {
        Some(glm::vec2(0.0, board_size.h - BALL_R - ball_pos.y))
    } else {
        None
    }
}

fn detect_wall_collision(ball_pos: glm::Vec2, wall: Rect) -> Option<glm::Vec2> {
    let wall_half_size = glm::vec2(wall.size.w / 2.0, wall.size.h / 2.0);
    let wall_center = glm::Vec2::from(&wall.pos) + wall_half_size;
    let closest_point_in_wall = wall_center
        + glm::clamp_vec(
            &(ball_pos - wall_center),
            &(-wall_half_size),
            &wall_half_size,
        );
    let distance_to_wall = glm::distance(&ball_pos, &closest_point_in_wall);
    if distance_to_wall < BALL_R {
        Some((BALL_R - distance_to_wall) * glm::normalize(&(ball_pos - closest_point_in_wall)))
    } else {
        None
    }
}

fn apply_collision_response(
    pv: (glm::Vec2, glm::Vec2),
    resp: Option<glm::Vec2>,
) -> (glm::Vec2, glm::Vec2) {
    match resp {
        Some(r) => {
            let dir = glm::normalize(&r);
            (
                pv.0 + r,
                // "Damped reflection" around the collision response direction
                pv.1 - (1.0 + BOUNCE_COEFF) * glm::dot(&pv.1, &dir) * dir,
            )
        }
        None => pv,
    }
}
