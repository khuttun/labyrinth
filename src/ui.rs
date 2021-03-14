use std::rc::Rc;
use std::time::Duration;

use crate::graphics;

pub struct Instance {
    ctx: egui::CtxRef,
    texture: Option<EguiTexture>,
    width_points: f32,
    height_points: f32,
    scale: f32,
}

impl Instance {
    pub fn new(width_pixels: u32, height_pixels: u32, scale: f32) -> Instance {
        Instance {
            ctx: egui::CtxRef::default(),
            texture: None,
            width_points: width_pixels as f32 / scale,
            height_points: height_pixels as f32 / scale,
            scale,
        }
    }

    pub fn update(
        &mut self,
        gfx: &graphics::Instance,
        elapsed: Duration,
    ) -> Vec<graphics::Object2d> {
        self.ctx.begin_frame(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(self.width_points, self.height_points),
            )),
            pixels_per_point: Some(self.scale),
            ..Default::default()
        });
        egui::Window::new("Timer window")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(egui::pos2(10.0, 10.0))
            .show(&self.ctx, |ui| {
                ui.label(format!(
                    "{:02}:{:02}",
                    elapsed.as_secs() / 60,
                    elapsed.as_secs() % 60
                ));
            });
        let (_output, shapes) = self.ctx.end_frame();

        let egui_texture = self.ctx.texture();
        let texture = match &self.texture {
            Some(t) if t.version == egui_texture.version => t,
            _ => {
                println!(
                    "Creating egui texture {} x {}",
                    egui_texture.width, egui_texture.height
                );
                self.texture = Some(EguiTexture {
                    version: egui_texture.version,
                    texture: Rc::new(
                        gfx.create_texture(
                            "egui texture",
                            egui_texture.width as u32,
                            egui_texture.height as u32,
                            &egui_texture
                                .pixels
                                .iter()
                                .flat_map(|&x| std::iter::repeat(x).take(4))
                                .collect::<Vec<u8>>(),
                        ),
                    ),
                });
                self.texture.as_ref().unwrap()
            }
        };

        self.ctx
            .tessellate(shapes)
            .iter()
            .map(|clipped_mesh| {
                // TODO: it could be more efficient to reuse the buffers instead of creating new ones each update
                let mut obj = gfx.create_object_2d(
                    &Rc::new(
                        gfx.create_shape_2d(
                            "egui mesh",
                            &clipped_mesh
                                .1
                                .vertices
                                .iter()
                                .map(|v| graphics::Vertex2d {
                                    position: [v.pos.x, v.pos.y],
                                    tex_coords: [v.uv.x, v.uv.y],
                                    color: v.color.to_array(),
                                })
                                .collect::<Vec<graphics::Vertex2d>>(),
                            &clipped_mesh.1.indices,
                        ),
                    ),
                    &texture.texture,
                );
                // X and Y have opposite signs here as egui coordinate system has Y direction opposite our graphics code
                obj.set_scaling(2.0 / self.width_points, -2.0 / self.height_points);
                obj.set_position(-1.0, 1.0);
                obj
            })
            .collect::<Vec<graphics::Object2d>>()
    }
}

struct EguiTexture {
    texture: Rc<graphics::Texture>,
    version: u64,
}
