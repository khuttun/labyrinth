use nalgebra_glm as glm;

#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

glium::implement_vertex!(Vertex, position, normal);

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
                normal: [as_float(&v["nx"]), as_float(&v["ny"]), as_float(&v["nz"])]
            }).collect();

        // println!("Vertices (length {}): {:?}", vertices.len(), vertices);

        let indices: Vec<u32> = ply.payload["face"].iter().map(
            |f| as_list_uint(&f["vertex_indices"])).flatten().collect();

        // println!("Indices (length {}): {:?}", indices.len(), indices);

        Shape {
            vertex_buffer: glium::VertexBuffer::new(facade, &vertices).unwrap(),
            index_buffer: glium::IndexBuffer::new(facade, glium::index::PrimitiveType::TrianglesList, &indices).unwrap(),
        }
    }
}

pub struct Object<'a> {
    shape: &'a Shape,
    scaling: glm::Vec3,
    rotation: (f32, glm::Vec3),
    translation: glm::Vec3,
    model_matrix: glm::Mat4x4,
}

impl<'a> Object<'a> {
    pub fn new(s: &Shape) -> Object {
        Object {
            shape: s,
            scaling: glm::vec3(1.0, 1.0, 1.0),
            rotation: (0.0, glm::vec3(1.0, 0.0, 0.0)),
            translation: glm::vec3(0.0, 0.0, 0.0),
            model_matrix: glm::identity(),
        }
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

// pub type ShapeId = usize;

pub type ObjectId = usize;

pub struct Scene<'a> {
    // shapes: Vec<Shape>,
    objects: Vec<Object<'a>>,
    view_matrix: glm::Mat4x4,
    perspective_matrix: glm::Mat4x4,
    light_position: glm::Vec4,
    default_shaders: glium::program::Program,
}

impl<'a> Scene<'a> {
    pub fn new<F>(facade: &F) -> Scene<'a>
        where F: glium::backend::Facade
    {
        Scene {
            // shapes: Vec::new(),
            objects: Vec::new(),
            view_matrix: glm::look_at(&glm::vec3(0.0, 5.0, 5.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0)),
            perspective_matrix: glm::perspective(1.0, glm::radians(&glm::vec1(50.0)).x, 0.1, 100.0),
            light_position: glm::vec4(0.0, 5.0, 0.0, 1.0),
            default_shaders: glium::program::Program::from_source(
                facade, include_str!("default.vert"), include_str!("default.frag"), None).unwrap()
        }
    }

    // pub fn create_shape<F>(&mut self, facade: &F, ply_file: &str) -> ShapeId
    //     where F: glium::backend::Facade
    // {
    //     self.shapes.push(Shape::from_ply(facade, ply_file));
    //     self.shapes.len() - 1
    // }

    // pub fn create_object(&mut self, shape: ShapeId) -> ObjectId {
    //     self.objects.push(Object::new(&self.shapes[shape]));
    //     self.objects.len() - 1
    // }

    // pub fn add_shape(&mut self, shape: Shape) -> ShapeId {
    //     self.shapes.push(shape);
    //     self.shapes.len() - 1
    // }

    // pub fn get_shape(&self, id: ShapeId) -> &Shape {
    //     &self.shapes[id]
    // }

    pub fn add_object(&mut self, obj: Object<'a>) -> ObjectId {
        self.objects.push(obj);
        self.objects.len() - 1
    }

    pub fn get_object(&mut self, id: ObjectId) -> &mut Object<'a> {
        &mut self.objects[id]
    }

    pub fn draw<S>(&self, surface: &mut S)
        where S: glium::Surface
    {
        surface.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        let pers_array: [[f32; 4]; 4] = self.perspective_matrix.into();
        let light_pos_array: [f32; 3] = (self.view_matrix * self.light_position).xyz().into();
        for obj in self.objects.iter() {
            let mv = self.view_matrix * obj.model_matrix;
            let mv_array: [[f32; 4]; 4] = mv.into();
            let mv_normal = glm::transpose(&glm::inverse(&glm::mat4_to_mat3(&mv)));
            let mv_normal_array: [[f32; 3]; 3] = mv_normal.into();
            surface.draw(
                &obj.shape.vertex_buffer,
                &obj.shape.index_buffer,
                &self.default_shaders,
                &glium::uniform! {
                    modelView: mv_array,
                    normalModelView: mv_normal_array,
                    projection: pers_array,
                    lightPosCamSpace: light_pos_array,
                },
                &glium::DrawParameters {
                    depth: glium::Depth {
                        test: glium::DepthTest::IfLess,
                        write: true,
                        .. Default::default()
                    },
                    .. Default::default()
                }).unwrap();
        }
    }
}
