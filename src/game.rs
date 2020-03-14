use std::time::Instant;

pub const BALL_R: f32 = 1.0;
pub const WALL_H: f32 = 1.5;
const ACCEL_COEFF: f32 = 100.0 / std::f32::consts::PI;

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
    pub pos: Point,
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
    InProgress {
        ball_pos: Point,
        ball_v: Velocity,
    },
    Won,
    Lost,
}

pub struct Game {
    pub state: State,
    pub prev_update: Option<Instant>,
    pub angle_x: f32, // Board angle w.r.t. x-axis in radians
    pub angle_y: f32,
    pub level: Level,
}

impl Game {
    pub fn new(lvl: Level) -> Game {
        Game {
            state: State::InProgress {
                ball_pos: lvl.start,
                ball_v: Velocity { x: 0.0, y: 0.0},
            },
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
            State::InProgress { ball_pos, ball_v } => {
                match self.prev_update {
                    None => {
                        self.prev_update = Some(time);
                    },
                    Some(prev_time) => {
                        let dt = time.duration_since(prev_time).as_secs_f32();
                        let a_x = self.angle_x * ACCEL_COEFF;
                        let a_y = self.angle_y * ACCEL_COEFF;
                        let v_x = ball_v.x + a_x * dt;
                        let v_y = ball_v.y + a_y * dt;
                        let pos_x = ball_pos.x + v_x * dt;
                        let pos_y = ball_pos.y + v_y * dt;
                        self.state = State::InProgress {
                            ball_pos: Point { x: pos_x, y: pos_y },
                            ball_v: Velocity { x: v_x, y: v_y },
                        };
                        self.prev_update = Some(time);
                    },
                }
            },
            _ => (),
        }
    }
}
