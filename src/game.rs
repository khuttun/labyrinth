pub const BALL_R: f32 = 1.0;
pub const WALL_H: f32 = 1.5;

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
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
