use instant::Instant;
use nalgebra_glm as glm;

use crate::game;

pub trait GameAi {
    fn init(&mut self, level: &game::Level);
    fn pause(&mut self);
    fn next_move(&mut self, game: &game::Game, time: Instant) -> glm::Vec2;
}

pub struct PathTracerAi {
    prev_update: Option<Instant>,
    path_index: usize,
}

impl PathTracerAi {
    pub fn new() -> PathTracerAi {
        PathTracerAi {
            prev_update: None,
            path_index: 1,
        }
    }
}

impl GameAi for PathTracerAi {
    fn init(&mut self, level: &game::Level) {
        assert!(level.path.len() >= 2);
        self.prev_update = None;
        self.path_index = 1;
    }

    fn pause(&mut self) {
        self.prev_update = None;
    }

    fn next_move(&mut self, game: &game::Game, time: Instant) -> glm::Vec2 {
        // Determine how much rotation can be done on this update
        const MAX_ROTATION_PER_SEC: f32 = 0.25;
        let max_rotation = match self.prev_update {
            Some(t0) => MAX_ROTATION_PER_SEC * time.duration_since(t0).as_secs_f32(),
            None => MAX_ROTATION_PER_SEC * 0.01,
        };
        self.prev_update = Some(time);

        // Calculate vector to target path point
        let ball_pos = glm::Vec2::from(&game.ball_pos);
        let mut to_target = glm::Vec2::from(&game.level.path[self.path_index]) - ball_pos;

        // If target path point was reached, update to next one
        const TARGET_R: f32 = 10.0;
        if self.path_index < game.level.path.len() - 1 && glm::length(&to_target) <= TARGET_R {
            self.path_index = self.path_index + 1;
            to_target = glm::Vec2::from(&game.level.path[self.path_index]) - ball_pos;
            println!("PathTracerAi: tracking path point {}", self.path_index);
        }

        // Calculate target velocity and target board angle
        let target_v = to_target;
        let v_diff = target_v - glm::Vec2::from(&game.ball_v);
        let v_diff_len = glm::length(&v_diff);
        let target_angle = if v_diff_len > 0.0 {
            const MAX_V_DIFF: f32 = 100.0;
            let len = game::MAX_ANGLE * glm::clamp_scalar(v_diff_len, 0.0, MAX_V_DIFF) / MAX_V_DIFF;
            len * v_diff / v_diff_len
        } else {
            glm::zero()
        };

        // Calculate how to rotate the board
        let angle_diff = target_angle - glm::vec2(game.angle_x, game.angle_y);
        let angle_diff_len = glm::length(&angle_diff);
        let rotation = if angle_diff_len > 0.0 {
            let len = glm::clamp_scalar(angle_diff_len, 0.0, max_rotation);
            len * angle_diff / angle_diff_len
        } else {
            glm::zero()
        };

        rotation
    }
}
