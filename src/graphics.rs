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

        Shape{
            vertex_buffer: glium::VertexBuffer::new(facade, &vertices).unwrap(),
            index_buffer: glium::IndexBuffer::new(facade, glium::index::PrimitiveType::TrianglesList, &indices).unwrap(),
        }
    }
}

pub struct Object<'a> {
    shape: &'a Shape,
    scaling: nalgebra_glm::TVec3<f32>,
    rotation: (f32, nalgebra_glm::TVec3<f32>),
    translation: nalgebra_glm::TVec3<f32>,
    model_matrix: nalgebra_glm::TMat4<f32>,
}

impl<'a> Object<'a> {
    pub fn new(s: &Shape) -> Object {
        Object{
            shape: s,
            scaling: nalgebra_glm::vec3(1.0, 1.0, 1.0),
            rotation: (0.0, nalgebra_glm::vec3(1.0, 0.0, 0.0)),
            translation: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            model_matrix: nalgebra_glm::identity(),
        }
    }

    pub fn set_scaling(&mut self, x: f32, y: f32, z: f32) {
        self.scaling = nalgebra_glm::vec3(x, y, z);
        self.update_model_matrix();
    }

    pub fn set_rotation(&mut self, angle_rad: f32, x: f32, y: f32, z: f32) {
        self.rotation = (angle_rad, nalgebra_glm::vec3(x, y, z));
        self.update_model_matrix();
    }

    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.translation = nalgebra_glm::vec3(x, y, z);
        self.update_model_matrix();
    }

    fn update_model_matrix(&mut self) {
        self.model_matrix =
            nalgebra_glm::translation(&self.translation) *
            nalgebra_glm::rotation(self.rotation.0, &self.rotation.1) *
            nalgebra_glm::scaling(&self.scaling);
    }
}
