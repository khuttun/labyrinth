use nalgebra_glm as glm;
use std::rc::Rc;

#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coords: [f32; 2],
}

glium::implement_vertex!(Vertex, position, normal, tex_coords);

pub struct Shape {
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u32>,
}

fn as_float(property: &ply_rs::ply::Property) -> f32 {
    match property {
        ply_rs::ply::Property::Float(value) => *value,
        _ => panic!("Property was not Float"),
    }
}

fn as_list_uint(property: &ply_rs::ply::Property) -> Vec<u32> {
    match property {
        ply_rs::ply::Property::ListUInt(value) => value.clone(),
        _ => panic!("Property was not ListUInt"),
    }
}

impl Shape {
    pub fn from_ply<F>(facade: &F, ply_file: &str) -> Shape
        where F: glium::backend::Facade
    {
        let p = ply_rs::parser::Parser::<ply_rs::ply::DefaultElement>::new();
        let mut f = std::fs::File::open(ply_file).unwrap();
        let ply = p.read_ply(&mut f).unwrap();

        let vertices: Vec<Vertex> = ply.payload["vertex"].iter().map(
            |v| Vertex{
                position: [as_float(&v["x"]), as_float(&v["y"]), as_float(&v["z"])],
                normal: [as_float(&v["nx"]), as_float(&v["ny"]), as_float(&v["nz"])],
                tex_coords: [as_float(&v["s"]), as_float(&v["t"])],
            }).collect();

        // println!("Vertices (length {}): {:?}", vertices.len(), vertices);

        let indices: Vec<u32> = ply.payload["face"].iter().map(
            |f| {
                let vis = as_list_uint(&f["vertex_indices"]);
                assert!(vis.len() == 3, "{} contains non-triangle faces", ply_file);
                vis
            }).flatten().collect();

        // println!("Indices (length {}): {:?}", indices.len(), indices);

        Shape {
            vertex_buffer: glium::VertexBuffer::new(facade, &vertices).unwrap(),
            index_buffer: glium::IndexBuffer::new(facade, glium::index::PrimitiveType::TrianglesList, &indices).unwrap(),
        }
    }
}

pub struct TexelRef<'a> {
    data: &'a mut [u8]
}

impl<'a> TexelRef<'a> {
    pub fn r(&mut self) -> &mut u8 {
        &mut self.data[0]
    }

    pub fn g(&mut self) -> &mut u8 {
        &mut self.data[1]
    }

    pub fn b(&mut self) -> &mut u8 {
        &mut self.data[2]
    }

    pub fn a(&mut self) -> &mut u8 {
        &mut self.data[3]
    }
}

pub struct Texture {
    data: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

impl Texture {
    pub fn solid_color(r: u8, g: u8, b: u8) -> Texture {
        Texture { data: vec![r, g, b, 255], w: 1, h: 1 }
    }

    pub fn solid_color_sized(r: u8, g: u8, b: u8, w: u32, h: u32) -> Texture {
        Texture { data: [r, g, b, 255].iter().cloned().cycle().take((4 * w * h) as usize).collect(), w: w, h: h }
    }

    pub fn texel(&mut self, u: usize, v: usize) -> TexelRef {
        let begin = 4 * u + 4 * v * self.w as usize;
        TexelRef { data: &mut self.data[begin .. begin + 4] }
    }

    fn into_glium_texture<F>(self, facade: &F) -> glium::texture::texture2d::Texture2d
        where F: glium::backend::Facade
    {
        glium::texture::texture2d::Texture2d::new(
            facade,
            glium::texture::RawImage2d::from_raw_rgba(self.data, (self.w, self.h)),
        ).unwrap()
    }
}

pub struct Object {
    shape: Rc<Shape>,
    texture: glium::texture::texture2d::Texture2d,
    scaling: glm::Vec3,
    rotation: (f32, glm::Vec3),
    translation: glm::Vec3,
    model_matrix: glm::Mat4x4,
}

impl Object {
    pub fn new<F>(facade: &F, s: &Rc<Shape>) -> Object
        where F: glium::backend::Facade
    {
        Object {
            shape: Rc::clone(s),
            texture: Texture::solid_color(50, 50, 50).into_glium_texture(facade),
            scaling: glm::vec3(1.0, 1.0, 1.0),
            rotation: (0.0, glm::vec3(1.0, 0.0, 0.0)),
            translation: glm::vec3(0.0, 0.0, 0.0),
            model_matrix: glm::identity(),
        }
    }

    pub fn set_texture<F>(&mut self, facade: &F, t: Texture)
        where F: glium::backend::Facade
    {
        self.texture = t.into_glium_texture(facade);
    }

    pub fn set_scaling(&mut self, x: f32, y: f32, z: f32) {
        self.scaling = glm::vec3(x, y, z);
        self.update_model_matrix();
    }

    pub fn set_rotation(&mut self, angle_rad: f32, x: f32, y: f32, z: f32) {
        self.rotation = (angle_rad, glm::vec3(x, y, z));
        self.update_model_matrix();
    }

    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.translation = glm::vec3(x, y, z);
        self.update_model_matrix();
    }

    fn update_model_matrix(&mut self) {
        self.model_matrix =
            glm::translation(&self.translation) *
            glm::rotation(self.rotation.0, &self.rotation.1) *
            glm::scaling(&self.scaling);
    }
}

pub type ObjectId = usize;

struct SceneObject {
    object: Object,
    parent: Option<ObjectId>,
}

// Iterate scene object hierarchy towards the root
struct SceneObjectHierarchyIterator<'a> {
    objects: &'a Vec<SceneObject>,
    current: Option<ObjectId>,
}

impl<'a> SceneObjectHierarchyIterator<'a> {
    fn new(scene: &'a Scene, id: ObjectId) -> Self {
        SceneObjectHierarchyIterator { objects: &scene.objects, current: Some(id) }
    }
}

impl<'a> Iterator for SceneObjectHierarchyIterator<'a> {
    type Item = &'a Object;
    fn next(&mut self) -> Option<Self::Item> {
        match self.current {
            None => None,
            Some(id) => {
                let scobj = &self.objects[id];
                self.current = scobj.parent;
                Some(&scobj.object)
            }
        }
    }
}

pub struct Scene {
    objects: Vec<SceneObject>,
    view_matrix: glm::Mat4x4,
    perspective_matrix: glm::Mat4x4,
    light_position: glm::Vec4,
    default_shaders: glium::program::Program,
}

impl Scene {
    pub fn new<F>(facade: &F, aspect: f32) -> Scene
        where F: glium::backend::Facade
    {
        Scene {
            objects: Vec::new(),
            view_matrix: glm::look_at(&glm::vec3(0.0, 5.0, 5.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0)),
            perspective_matrix: glm::perspective(aspect, glm::radians(&glm::vec1(45.0)).x, 0.1, 100.0),
            light_position: glm::vec4(0.0, 5.0, 0.0, 1.0),
            default_shaders: glium::program::Program::new(
                facade,
                glium::program::ProgramCreationInput::SourceCode {
                    vertex_shader: include_str!("default.vert"),
                    fragment_shader: include_str!("default.frag"),
                    geometry_shader: None,
                    tessellation_control_shader: None,
                    tessellation_evaluation_shader: None,
                    transform_feedback_varyings: None,
                    outputs_srgb: true, // set true so that glium doesn't enable GL_FRAMEBUFFER_SRGB
                    uses_point_size: false,
                }
            ).unwrap(),
        }
    }

    pub fn add_object(&mut self, obj: Object, prnt: Option<ObjectId>) -> ObjectId {
        self.objects.push(SceneObject {object: obj, parent: prnt });
        self.objects.len() - 1
    }

    pub fn get_object(&mut self, id: ObjectId) -> &mut Object {
        &mut self.objects[id].object
    }

    pub fn look_at(&mut self, cam_x: f32, cam_y: f32, cam_z: f32, center_x: f32, center_y: f32, center_z: f32) {
        self.view_matrix = glm::look_at(
            &glm::vec3(cam_x, cam_y, cam_z),
            &glm::vec3(center_x, center_y, center_z),
            &glm::vec3(0.0, 1.0, 0.0));
    }

    pub fn set_light_position(&mut self, x: f32, y: f32, z: f32) {
        self.light_position = glm::vec4(x, y, z, 1.0);
    }

    pub fn draw<S>(&self, surface: &mut S)
        where S: glium::Surface
    {
        surface.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        let light_pos_array: [f32; 3] = self.light_position.xyz().into();
        for (id, scobj) in self.objects.iter().enumerate() {
            let effective_model_matrix = SceneObjectHierarchyIterator::new(self, id)
                .fold(glm::identity(), |acc, obj| obj.model_matrix * acc);
            let mvp_array: [[f32; 4]; 4] = (self.perspective_matrix * self.view_matrix * effective_model_matrix).into();
            surface.draw(
                &scobj.object.shape.vertex_buffer,
                &scobj.object.shape.index_buffer,
                &self.default_shaders,
                &glium::uniform! {
                    modelViewProjection: mvp_array,
                    lightPos: light_pos_array,
                    tex: &scobj.object.texture,
                },
                &glium::DrawParameters {
                    blend: glium::Blend::alpha_blending(),
                    depth: glium::Depth {
                        test: glium::DepthTest::IfLess,
                        write: true,
                        .. Default::default()
                    },
                    backface_culling: glium::draw_parameters::BackfaceCullingMode::CullClockwise,
                    .. Default::default()
                }).unwrap();
        }
    }
}
