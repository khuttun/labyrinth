use std::f32::consts::PI;
use std::time::Instant;

pub const BALL_R: f32 = 1.0;
pub const HOLE_R: f32 = 1.25 * BALL_R;
pub const WALL_H: f32 = 1.5;
const ACCEL_COEFF: f32 = 200.0 / std::f32::consts::PI;
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

        let (p, v) = self.detect_collisions(self.do_physics(time));
        
        if self.level.end.contains(&p) {
            self.state = State::Won;
        }

        if self.level.holes.iter().find(|h| p.distance_to(h) < HOLE_R).is_some() {
            self.state = State::Lost;
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
                Some(WallEdge::Left) => bounce_x(&mut p, &mut v, wall.pos.x - BALL_R),
                Some(WallEdge::Right) => bounce_x(&mut p, &mut v, wall.pos.x + wall.size.w + BALL_R),
                Some(WallEdge::Top) => bounce_y(&mut p, &mut v, wall.pos.y - BALL_R),
                Some(WallEdge::Bottom) => bounce_y(&mut p, &mut v, wall.pos.y + wall.size.h + BALL_R),
                None => (),
            }
        }

        (p, v)
    }
}

enum WallEdge {
    Left,
    Right,
    Top,
    Bottom,
}

fn detect_wall_collision(ball_pos: &Point, wall: &Rect) -> Option<WallEdge> {
    let wall_half_size = Size { w: wall.size.w / 2.0, h: wall.size.h / 2.0 };
    let wall_center = Point { x: wall.pos.x + wall_half_size.w, y: wall.pos.y + wall_half_size.h };
    let closest_point_in_wall = Point {
        x: wall_center.x + clamp(ball_pos.x - wall_center.x, -wall_half_size.w, wall_half_size.w),
        y: wall_center.y + clamp(ball_pos.y - wall_center.y, -wall_half_size.h, wall_half_size.h),
    };

    if ball_pos.distance_to(&closest_point_in_wall) < BALL_R {
        Some(angle_to_wall_edge(closest_point_in_wall.angle_to(ball_pos)))
    }
    else {
        None
    }
}

fn angle_to_wall_edge(a: f32) -> WallEdge {
    match a {
        a if a < -3.0 * PI / 4.0 => WallEdge::Left,
        a if a < -PI / 4.0 => WallEdge::Top,
        a if a < PI / 4.0 => WallEdge:: Right,
        a if a < 3.0 * PI / 4.0 => WallEdge::Bottom,
        _ => WallEdge::Left,
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

fn clamp(x: f32, min: f32, max: f32) -> f32 {
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
