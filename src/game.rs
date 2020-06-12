use nalgebra_glm as glm;
use std::f32::consts::PI;
use std::time::Instant;

pub const BALL_R: f32 = 20.0;
pub const HOLE_R: f32 = 1.2 * BALL_R;
pub const MAX_ANGLE: f32 = PI / 32.0;
const ACCEL_COEFF: f32 = 300.0 * BALL_R / std::f32::consts::PI;
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
    pub fn distance_to(&self, other: &Point) -> f32 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2)).sqrt()
    }

    pub fn angle_to(&self, other: &Point) -> f32 {
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
        // TODO: check that no walls/holes/board edges collide
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
    Lost {
        hole: Point,
        t_lost: Instant,
    },
}

pub struct Game {
    pub state: State,
    pub ball_pos: Point,
    pub ball_v: Velocity,
    pub angle_x: f32, // Board angle w.r.t. x-axis in radians
    pub angle_y: f32,
    prev_update: Option<Instant>,
    level: Level,
}

impl Game {
    pub fn new(lvl: Level) -> Game {
        Game {
            state: State::InProgress,
            ball_pos: lvl.start,
            ball_v: Velocity { x: 0.0, y: 0.0},
            angle_x: 0.0,
            angle_y: 0.0,
            prev_update: None,
            level: lvl,
        }
    }

    pub fn rotate_x(&mut self, angle: f32) {
        self.angle_x = clamp(self.angle_x + angle, -MAX_ANGLE, MAX_ANGLE);
    }

    pub fn rotate_y(&mut self, angle: f32) {
        self.angle_y = clamp(self.angle_y + angle, -MAX_ANGLE, MAX_ANGLE);
    }

    pub fn update(&mut self, time: Instant) {
        match self.state {
            State::InProgress => (),
            _ => return, // nothing to update if the game is not in progress anymore
        }

        let (p, v) = self.detect_collisions(self.do_physics(time));
        
        if self.level.end.contains(&p) {
            self.state = State::Won;
        }

        let hole = self.level.holes.iter().find(|h| p.distance_to(h) < HOLE_R);
        if hole.is_some() {
            self.state = State::Lost { hole: *hole.unwrap(), t_lost: time };
        }

        self.prev_update = Some(time);
        self.ball_pos = p;
        self.ball_v = v;
    }

    fn do_physics(&self, time: Instant) -> (Point, Velocity) {
        let dt = time.duration_since(self.prev_update.unwrap_or(time)).as_secs_f32();

        let v = Velocity {
            x: self.ball_v.x + self.angle_x * ACCEL_COEFF * dt,
            y: self.ball_v.y + self.angle_y * ACCEL_COEFF * dt,
        };
        let p = Point {
            x: self.ball_pos.x + v.x * dt,
            y: self.ball_pos.y + v.y * dt,
        };

        (p, v)
    }

    fn detect_collisions(&self, (mut p, mut v): (Point, Velocity)) -> (Point, Velocity) {
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
            match detect_wall_collision(&p, wall) {
                Some(response) => {
                    p.x = p.x + response.0.x * response.1;
                    p.y = p.y + response.0.y * response.1;
                    
                    // "Damped reflection" around the collision response direction
                    let v0 = glm::vec2(v.x, v.y);
                    let v1 = v0 - (1.0 + BOUNCE_COEFF) * glm::dot(&v0, &response.0) * response.0;
                    v.x = v1.x;
                    v.y = v1.y;
                },
                None => (),
            }
        }

        (p, v)
    }
}

// Return the collision response (direction, amount) if ball collides with the wall
fn detect_wall_collision(ball_pos: &Point, wall: &Rect) -> Option<(glm::Vec2, f32)> {
    let wall_half_size = Size { w: wall.size.w / 2.0, h: wall.size.h / 2.0 };
    let wall_center = Point { x: wall.pos.x + wall_half_size.w, y: wall.pos.y + wall_half_size.h };
    let closest_point_in_wall = Point {
        x: wall_center.x + clamp(ball_pos.x - wall_center.x, -wall_half_size.w, wall_half_size.w),
        y: wall_center.y + clamp(ball_pos.y - wall_center.y, -wall_half_size.h, wall_half_size.h),
    };
    let distance_to_wall = ball_pos.distance_to(&closest_point_in_wall);

    if distance_to_wall < BALL_R {
        Some((
            glm::normalize(&(glm::vec2(ball_pos.x, ball_pos.y) - glm::vec2(closest_point_in_wall.x, closest_point_in_wall.y))),
            BALL_R - distance_to_wall
        ))
    }
    else {
        None
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

pub fn clamp(x: f32, min: f32, max: f32) -> f32 {
    match x {
        x if x < min => min,
        x if x > max => max,
        _ => x,
    }
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
