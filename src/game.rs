use std::time::Instant;

pub const BALL_R: f32 = 1.0;
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
    fn contains(&self, p: Point) -> bool {
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
            p.x = BALL_R;
            v.x = -BOUNCE_COEFF * v.x;
        }
        if p.x >= self.level.size.w - BALL_R { // right
            p.x = self.level.size.w - BALL_R;
            v.x = -BOUNCE_COEFF * v.x;
        }
        if p.y < BALL_R { // top
            p.y = BALL_R;
            v.y = -BOUNCE_COEFF * v.y;
        }
        if p.y >= self.level.size.h - BALL_R { // bottom
            p.y = self.level.size.h - BALL_R;
            v.y = -BOUNCE_COEFF * v.y;
        }

        // Wall collisions
        for wall in self.level.walls.iter() {
            if wall.adjacent_left(BALL_R).contains(p) {
                p.x = wall.pos.x - BALL_R;
                v.x = -BOUNCE_COEFF * v.x;
            }
            if wall.adjacent_right(BALL_R).contains(p) {
                p.x = wall.pos.x + wall.size.w + BALL_R;
                v.x = -BOUNCE_COEFF * v.x;
            }
            if wall.adjacent_top(BALL_R).contains(p) {
                p.y = wall.pos.y - BALL_R;
                v.y = -BOUNCE_COEFF * v.y;
            }
            if wall.adjacent_bottom(BALL_R).contains(p) {
                p.y = wall.pos.y + wall.size.h + BALL_R;
                v.y = -BOUNCE_COEFF * v.y;
            }
        }

        self.ball_pos = p;
        self.ball_v = v;
        self.prev_update = Some(time);
    }
}
