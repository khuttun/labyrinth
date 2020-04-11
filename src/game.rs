use std::f32::consts::PI;
use std::time::Instant;

pub const BALL_R: f32 = 1.0;
pub const HOLE_R: f32 = 1.25 * BALL_R;
pub const WALL_H: f32 = 1.5;
const ACCEL_COEFF: f32 = 100.0 / std::f32::consts::PI;
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

impl Point {
    fn distance_to(&self, other: &Point) -> f32 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2)).sqrt()
    }

    fn angle_to(&self, other: &Point) -> f32 {
        (other.y - self.y).atan2(other.x - self.x)
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
    fn contains(&self, p: &Point) -> bool {
        p.x >= self.pos.x && p.x < self.pos.x + self.size.w && p.y >= self.pos.y && p.y < self.pos.y + self.size.h
    }

    // Functions for creating a new Rect sharing an edge with self
    fn adjacent_left(&self, w: f32) -> Rect {
        Rect { pos: Point { x: self.pos.x - w, y: self.pos.y}, size: Size { w: w, h: self.size.h } }
    }
    fn adjacent_right(&self, w: f32) -> Rect {
        Rect { pos: Point { x: self.pos.x + self.size.w, y: self.pos.y}, size: Size { w: w, h: self.size.h } }
    }
    fn adjacent_top(&self, h: f32) -> Rect {
        Rect { pos: Point { x: self.pos.x, y: self.pos.y - h}, size: Size { w: self.size.w, h: h } }
    }
    fn adjacent_bottom(&self, h: f32) -> Rect {
        Rect { pos: Point { x: self.pos.x, y: self.pos.y + self.size.h}, size: Size { w: self.size.w, h: h } }
    }

    // Functions for getting the Rect corner points
    fn top_left(&self) -> Point {
        self.pos
    }
    fn top_right(&self) -> Point {
        Point {
            x: self.pos.x + self.size.w,
            y: self.pos.y,
        }
    }
    fn bottom_left(&self) -> Point {
        Point {
            x: self.pos.x,
            y: self.pos.y + self.size.h,
        }
    }
    fn bottom_right(&self) -> Point {
        Point {
            x: self.pos.x + self.size.w,
            y: self.pos.y + self.size.h,
        }
    }
}

// Level coordinate system origin is in top-left corner of the board.
// Positive x-axis direction is right and positive y-axis direction down.
pub struct Level {
    pub name: String,
    pub size: Size,
    pub start: Point,
    pub end: Rect,
    pub walls: Vec<Rect>,
    pub holes: Vec<Point>,
}

impl Level {
    pub fn from_json(file_name: &str) -> Level {
        let data = json::parse(&std::fs::read_to_string(file_name).unwrap()).unwrap();
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
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

#[derive(Copy, Clone, Debug)]
pub enum State {
    InProgress,
    Won,
    Lost,
}

pub struct Game {
    pub state: State,
    pub ball_pos: Point,
    ball_v: Velocity,
    prev_update: Option<Instant>,
    angle_x: f32, // Board angle w.r.t. x-axis in radians
    angle_y: f32,
    level: Level,
}

impl Game {
    pub fn new(lvl: Level) -> Game {
        Game {
            state: State::InProgress,
            ball_pos: lvl.start,
            ball_v: Velocity { x: 0.0, y: 0.0},
            prev_update: None,
            angle_x: 0.0,
            angle_y: 0.0,
            level: lvl,
        }
    }

    pub fn set_x_angle(&mut self, angle: f32) {
        self.angle_x = angle;
    }

    pub fn set_y_angle(&mut self, angle: f32) {
        self.angle_y = angle;
    }

    pub fn update(&mut self, time: Instant) {
        match self.state {
            State::InProgress => (),
            _ => return, // nothing to update if the game is not in progress anymore
        }

        // Physics
        let dt = time.duration_since(self.prev_update.unwrap_or(time)).as_secs_f32();
        let mut v = Velocity {
            x: self.ball_v.x + self.angle_x * ACCEL_COEFF * dt,
            y: self.ball_v.y + self.angle_y * ACCEL_COEFF * dt,
        };
        let mut p = Point {
            x: self.ball_pos.x + v.x * dt,
            y: self.ball_pos.y + v.y * dt,
        };

        // Board edge collisions
        if p.x < BALL_R { // left
            bounce_x(&mut p, &mut v, BALL_R);
        }
        if p.x >= self.level.size.w - BALL_R { // right
            bounce_x(&mut p, &mut v, self.level.size.w - BALL_R);
        }
        if p.y < BALL_R { // top
            bounce_y(&mut p, &mut v, BALL_R);
        }
        if p.y >= self.level.size.h - BALL_R { // bottom
            bounce_y(&mut p, &mut v, self.level.size.h - BALL_R);
        }

        // Wall collisions
        for wall in self.level.walls.iter() {
            if collides_left(&p, wall) {
                bounce_x(&mut p, &mut v, wall.pos.x - BALL_R);
            }
            else if collides_right(&p, wall) {
                bounce_x(&mut p, &mut v, wall.pos.x + wall.size.w + BALL_R);
            }
            else if collides_top(&p, wall) {
                bounce_y(&mut p, &mut v, wall.pos.y - BALL_R);
            }
            else if collides_bottom(&p, wall) {
                bounce_y(&mut p, &mut v, wall.pos.y + wall.size.h + BALL_R);
            }
        }

        if self.level.end.contains(&p) {
            self.state = State::Won;
        }

        if self.level.holes.iter().find(|h| p.distance_to(h) < HOLE_R).is_some() {
            self.state = State::Lost;
        }

        self.ball_pos = p;
        self.ball_v = v;
        self.prev_update = Some(time);
    }
}

fn bounce_x(pos: &mut Point, vel: &mut Velocity, edge_x: f32) {
    pos.x = edge_x;
    vel.x = -BOUNCE_COEFF * vel.x;
}

fn bounce_y(pos: &mut Point, vel: &mut Velocity, edge_y: f32) {
    pos.y = edge_y;
    vel.y = -BOUNCE_COEFF * vel.y;
}

trait InRangeExt {
    fn is_in_range(self, a: Self, b: Self) -> bool;
}
impl InRangeExt for f32 {
    fn is_in_range(self, a: f32, b: f32) -> bool {
        self >= a && self < b
    }
}

// Ball is considered to be colliding with a wall edge, if it's
// - adjacent to the edge with position < ball radius from the edge, or
// - within the 1/8 circle of the same radius as the ball in either end of the edge

fn collides_left(ball_pos: &Point, wall: &Rect) -> bool {
    wall.adjacent_left(BALL_R).contains(ball_pos)
        || (wall.top_left().distance_to(ball_pos) < BALL_R
            && wall.top_left().angle_to(ball_pos).is_in_range(-PI, -3.0 * PI / 4.0))
        || (wall.bottom_left().distance_to(ball_pos) < BALL_R
            && wall.bottom_left().angle_to(ball_pos).is_in_range(3.0 * PI / 4.0, PI))
}

fn collides_right(ball_pos: &Point, wall: &Rect) -> bool {
    wall.adjacent_right(BALL_R).contains(ball_pos)
        || (wall.top_right().distance_to(ball_pos) < BALL_R
            && wall.top_right().angle_to(ball_pos).is_in_range(-PI / 4.0, 0.0))
        || (wall.bottom_right().distance_to(ball_pos) < BALL_R
            && wall.bottom_right().angle_to(ball_pos).is_in_range(0.0, PI / 4.0))
}

fn collides_top(ball_pos: &Point, wall: &Rect) -> bool {
    wall.adjacent_top(BALL_R).contains(ball_pos)
        || (wall.top_left().distance_to(ball_pos) < BALL_R
            && wall.top_left().angle_to(ball_pos).is_in_range(-3.0 * PI / 4.0, -PI / 2.0))
        || (wall.top_right().distance_to(ball_pos) < BALL_R
            && wall.top_right().angle_to(ball_pos).is_in_range(-PI / 2.0, -PI / 4.0))
}

fn collides_bottom(ball_pos: &Point, wall: &Rect) -> bool {
    wall.adjacent_bottom(BALL_R).contains(ball_pos)
        || (wall.bottom_left().distance_to(ball_pos) < BALL_R
            && wall.bottom_left().angle_to(ball_pos).is_in_range(PI / 2.0, 3.0 * PI / 4.0))
        || (wall.bottom_right().distance_to(ball_pos) < BALL_R
            && wall.bottom_right().angle_to(ball_pos).is_in_range(PI / 4.0, PI / 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[derive(Copy, Clone, Debug)]
    struct Roughly(f32);
    impl PartialEq<f32> for Roughly {
        fn eq(&self, other: &f32) -> bool {
            (self.0 - other).abs() < 1e-6
        }
    }
    
    #[test]
    fn point() {
        assert_eq!(Roughly(0.0), Point {x: -1.0, y: 1.0}.distance_to(&Point {x: -1.0, y: 1.0}));
        assert_eq!(Roughly(2.0), Point {x: -1.0, y: 1.0}.distance_to(&Point {x: 1.0, y: 1.0}));
        assert_eq!(Roughly(2.0f32.sqrt()), Point {x: -1.0, y: 1.0}.distance_to(&Point {x: -2.0, y: 0.0}));

        assert_eq!(Roughly(0.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 1.0, y: 1.0}));
        assert_eq!(Roughly(0.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 2.0, y: 1.0}));
        assert_eq!(Roughly(PI / 4.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 2.0, y: 2.0}));
        assert_eq!(Roughly(PI / 2.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 1.0, y: 2.0}));
        assert_eq!(Roughly(3.0 * PI / 4.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 0.0, y: 2.0}));
        assert_eq!(Roughly(PI), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 0.0, y: 1.0}));
        assert_eq!(Roughly(-3.0 * PI / 4.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 0.0, y: 0.0}));
        assert_eq!(Roughly(-PI / 2.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 1.0, y: 0.0}));
        assert_eq!(Roughly(-PI / 4.0), Point {x: 1.0, y: 1.0}.angle_to(&Point {x: 2.0, y: 0.0}));
    }
}
