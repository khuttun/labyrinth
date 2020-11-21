use nalgebra_glm as glm;
use raw_window_handle::HasRawWindowHandle;
use std::{iter, rc::Rc};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coords: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
struct Uniforms {
    model_view: [[f32; 4]; 4],
    model_view_normal: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    light_pos_cam_space: [f32; 4], // Only xyz components used. The vector is 4D to satisfy GLSL uniform alignment requirements.
}

impl Uniforms {
    fn from(mv: &glm::Mat4, proj: &glm::Mat4, light: &glm::Vec4) -> Uniforms {
        Uniforms {
            model_view: mv.clone().into(),
            model_view_normal: glm::transpose(&glm::inverse(mv)).into(),
            projection: proj.clone().into(),
            light_pos_cam_space: light.clone().into(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Config {
    pub msaa_samples: u32,
    pub mipmap_levels: u32,
    pub vsync: bool,
}

impl Config {
    pub fn new() -> Config {
        Config {
            msaa_samples: 4,
            mipmap_levels: 6,
            vsync: true,
        }
    }
}

pub struct Instance {
    config: Config,
    width: u32,
    height: u32,
    device: wgpu::Device,
    queue: wgpu::Queue,
    swap_chain: wgpu::SwapChain,
    multisampled_fb_texture_view: wgpu::TextureView,
    depth_texture_view: wgpu::TextureView,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
}

impl Instance {
    pub async fn new<W: HasRawWindowHandle>(
        config: Config,
        window: &W,
        width: u32,
        height: u32,
    ) -> Instance {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    shader_validation: true,
                },
                None,
            )
            .await
            .unwrap();

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width,
            height,
            present_mode: if config.vsync {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::Immediate
            },
        };

        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let multisampled_fb_texture_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("MSAA framebuffer texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth: 1,
                },
                mip_level_count: 1,
                sample_count: config.msaa_samples,
                dimension: wgpu::TextureDimension::D2,
                format: sc_desc.format,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture_desc = wgpu::TextureDescriptor {
            label: Some("Depth texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: config.msaa_samples,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        };

        let depth_texture_view = device
            .create_texture(&depth_texture_desc)
            .create_view(&wgpu::TextureViewDescriptor::default());

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Uniform bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::UniformBuffer {
                        dynamic: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            multisampled: false,
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Float,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        count: None,
                    },
                ],
            });

        let vs_module = device.create_shader_module(wgpu::include_spirv!("default.vert.spv"));
        let fs_module = device.create_shader_module(wgpu::include_spirv!("default.frag.spv"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render pipeline layout"),
                bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::Back,
                polygon_mode: wgpu::PolygonMode::Fill,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
                clamp_depth: false,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: sc_desc.format,
                // Alpha blending as done in glium::Blend::alpha_blending().
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: depth_texture_desc.format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilStateDescriptor::default(),
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttributeDescriptor {
                            // position
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float3,
                        },
                        wgpu::VertexAttributeDescriptor {
                            // normal
                            offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float3,
                        },
                        wgpu::VertexAttributeDescriptor {
                            // tex_coords
                            offset: 2 * std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float2,
                        },
                    ],
                }],
            },
            sample_count: config.msaa_samples,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        Instance {
            config,
            width,
            height,
            device,
            queue,
            swap_chain,
            multisampled_fb_texture_view,
            depth_texture_view,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            render_pipeline,
        }
    }

    pub fn create_scene(&self) -> Scene {
        Scene::new(self.width as f32 / self.height as f32)
    }

    pub fn create_shape(&self, name: &str, ply: &str) -> Shape {
        Shape::from_ply(self, name, ply)
    }

    pub fn create_texture(&self, name: &str, w: u32, h: u32, rgba_data: &[u8]) -> Texture {
        Texture::from_image(self, name, w, h, rgba_data)
    }

    pub fn create_object(&self, s: &Rc<Shape>, t: &Rc<Texture>) -> Node {
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform buffer"),
            size: std::mem::size_of::<Uniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform bind group"),
            layout: &self.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        });
        Node::new(NodeKind::Object {
            shape: Rc::clone(s),
            texture: Rc::clone(t),
            uniform_buffer: buf,
            uniform_bind_group: bind_group,
        })
    }

    pub fn create_transformation(&self) -> Node {
        Node::new(NodeKind::Transformation)
    }

    pub fn render_scene(&self, scene: &Scene) {
        // 1. update uniforms
        let light_pos_cam_space = scene.view_matrix * scene.light_position;
        for (id, n) in scene.nodes.iter().enumerate() {
            match &n.node.kind {
                NodeKind::Object {
                    shape: _,
                    texture: _,
                    uniform_buffer,
                    uniform_bind_group: _,
                } => {
                    let effective_model_matrix = SceneIterator::new(scene, id)
                        .fold(glm::identity(), |acc, node| node.model_matrix * acc);
                    let model_view = scene.view_matrix * effective_model_matrix;
                    self.queue.write_buffer(
                        &uniform_buffer,
                        0,
                        bytemuck::cast_slice(&[Uniforms::from(
                            &model_view,
                            &scene.perspective_matrix,
                            &light_pos_cam_space,
                        )]),
                    );
                }
                NodeKind::Transformation => (), // no uniforms to update
            }
        }

        // 2. Draw
        let frame = self
            .swap_chain
            .get_current_frame()
            .expect("Timeout getting texture")
            .output;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &self.multisampled_fb_texture_view,
                    resolve_target: Some(&frame.view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_pipeline(&self.render_pipeline);

            for n in scene.nodes.iter() {
                match &n.node.kind {
                    NodeKind::Object {
                        shape,
                        texture,
                        uniform_buffer: _,
                        uniform_bind_group,
                    } => {
                        render_pass.set_bind_group(0, &uniform_bind_group, &[]);
                        render_pass.set_bind_group(1, &texture.bind_group, &[]);
                        render_pass.set_vertex_buffer(0, shape.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(shape.index_buffer.slice(..));
                        render_pass.draw_indexed(0..shape.index_count as u32, 0, 0..1);
                    }
                    NodeKind::Transformation => (), // nothing to draw
                }
            }
        }

        self.queue.submit(iter::once(encoder.finish()));
    }
}

pub struct Scene {
    nodes: Vec<SceneNode>,
    view_matrix: glm::Mat4x4,
    perspective_matrix: glm::Mat4x4,
    light_position: glm::Vec4,
}

impl Scene {
    fn new(aspect: f32) -> Scene {
        Scene {
            nodes: Vec::new(),
            view_matrix: glm::look_at(
                &glm::vec3(0.0, 5.0, 5.0),
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 1.0, 0.0),
            ),
            perspective_matrix: glm::perspective(
                aspect,
                glm::radians(&glm::vec1(45.0)).x,
                2.0,
                2000.0,
            ),
            light_position: glm::vec4(0.0, 5.0, 0.0, 1.0),
        }
    }

    pub fn add_node(&mut self, node: Node, parent: Option<NodeId>) -> NodeId {
        self.nodes.push(SceneNode { node, parent });
        self.nodes.len() - 1
    }

    pub fn get_node(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id].node
    }

    pub fn look_at(
        &mut self,
        cam_x: f32,
        cam_y: f32,
        cam_z: f32,
        center_x: f32,
        center_y: f32,
        center_z: f32,
    ) {
        self.view_matrix = glm::look_at(
            &glm::vec3(cam_x, cam_y, cam_z),
            &glm::vec3(center_x, center_y, center_z),
            &glm::vec3(0.0, 1.0, 0.0),
        );
    }

    pub fn set_light_position(&mut self, x: f32, y: f32, z: f32) {
        self.light_position = glm::vec4(x, y, z, 1.0);
    }
}

pub struct Shape {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: usize,
}

impl Shape {
    fn from_ply(inst: &Instance, name: &str, ply: &str) -> Shape {
        let mut bytes = ply.as_bytes();
        let ply = ply_rs::parser::Parser::<ply_rs::ply::DefaultElement>::new()
            .read_ply(&mut bytes)
            .unwrap();

        let vertices: Vec<Vertex> = ply.payload["vertex"]
            .iter()
            .map(|v| Vertex {
                position: [as_float(&v["x"]), as_float(&v["y"]), as_float(&v["z"])],
                normal: [as_float(&v["nx"]), as_float(&v["ny"]), as_float(&v["nz"])],
                tex_coords: [as_float(&v["s"]), as_float(&v["t"])],
            })
            .collect();

        let indices: Vec<u32> = ply.payload["face"]
            .iter()
            .map(|f| {
                let vis = as_list_uint(&f["vertex_indices"]);
                assert!(vis.len() == 3, "{} contains non-triangle faces", name);
                vis
            })
            .flatten()
            .collect();

        let vertex_buffer = inst.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("Vertex buffer {}", name)),
            size: (vertices.len() * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::VERTEX,
            mapped_at_creation: true,
        });
        vertex_buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(bytemuck::cast_slice(&vertices));
        vertex_buffer.unmap();

        let index_buffer = inst.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("Index buffer {}", name)),
            size: (indices.len() * std::mem::size_of::<u32>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::INDEX,
            mapped_at_creation: true,
        });
        index_buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(bytemuck::cast_slice(&indices));
        index_buffer.unmap();

        Shape {
            vertex_buffer,
            index_buffer,
            index_count: indices.len(),
        }
    }
}

pub struct Texture {
    bind_group: wgpu::BindGroup,
}

impl Texture {
    fn from_image(inst: &Instance, name: &str, w: u32, h: u32, rgba_data: &[u8]) -> Texture {
        // Create the texture, view, sampler, bind group...
        let tex = inst.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Texture {}", name)),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth: 1,
            },
            mip_level_count: inst.config.mipmap_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = inst.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("Sampler {}", name)),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = inst.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Texture bind group {}", name)),
            layout: &inst.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Upload the main texture data
        inst.queue.write_texture(
            wgpu::TextureCopyView {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            rgba_data,
            wgpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: 4 * w,
                rows_per_image: h,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth: 1,
            },
        );

        // Create mipmaps and upload their data
        let original: image::ImageBuffer<image::Rgba<u8>, _> =
            image::ImageBuffer::from_raw(w, h, rgba_data).unwrap();
        for level in 1..inst.config.mipmap_levels {
            let mipmap = image::imageops::resize(
                &original,
                w / 2u32.pow(level),
                h / 2u32.pow(level),
                image::imageops::FilterType::Triangle,
            );
            inst.queue.write_texture(
                wgpu::TextureCopyView {
                    texture: &tex,
                    mip_level: level,
                    origin: wgpu::Origin3d::ZERO,
                },
                &mipmap,
                wgpu::TextureDataLayout {
                    offset: 0,
                    bytes_per_row: 4 * mipmap.width(),
                    rows_per_image: mipmap.height(),
                },
                wgpu::Extent3d {
                    width: mipmap.width(),
                    height: mipmap.height(),
                    depth: 1,
                },
            );
        }

        Texture { bind_group }
    }
}

enum NodeKind {
    Object {
        shape: Rc<Shape>,
        texture: Rc<Texture>,
        uniform_buffer: wgpu::Buffer,
        uniform_bind_group: wgpu::BindGroup,
    },
    Transformation,
}

pub struct Node {
    kind: NodeKind,
    scaling: glm::Vec3,
    rotation: glm::Mat4x4,
    translation: glm::Vec3,
    model_matrix: glm::Mat4x4,
}

impl Node {
    fn new(kind: NodeKind) -> Node {
        Node {
            kind,
            scaling: glm::vec3(1.0, 1.0, 1.0),
            rotation: glm::identity(),
            translation: glm::vec3(0.0, 0.0, 0.0),
            model_matrix: glm::identity(),
        }
    }

    pub fn set_scaling(&mut self, x: f32, y: f32, z: f32) {
        self.scaling = glm::vec3(x, y, z);
        self.update_model_matrix();
    }

    pub fn set_rotation(&mut self, x_angle: f32, y_angle: f32, z_angle: f32) {
        let axis = glm::vec3(x_angle, y_angle, z_angle);
        self.rotation = glm::rotation(glm::length(&axis), &axis);
        self.update_model_matrix();
    }

    pub fn rotate_in_world_space(&mut self, angle: f32, x: f32, y: f32, z: f32) {
        let axis_model_space =
            glm::normalize(&(glm::inverse(&self.model_matrix) * glm::vec4(x, y, z, 0.0)).xyz());
        self.rotation = glm::rotate(&self.rotation, angle, &axis_model_space);
        self.update_model_matrix();
    }

    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.translation = glm::vec3(x, y, z);
        self.update_model_matrix();
    }

    fn update_model_matrix(&mut self) {
        self.model_matrix =
            glm::translation(&self.translation) * self.rotation * glm::scaling(&self.scaling);
    }
}

pub type NodeId = usize;

struct SceneNode {
    node: Node,
    parent: Option<NodeId>,
}

// Iterate scene nodes towards the root
struct SceneIterator<'a> {
    nodes: &'a Vec<SceneNode>,
    next: Option<NodeId>,
}

impl<'a> SceneIterator<'a> {
    fn new(scene: &'a Scene, id: NodeId) -> Self {
        SceneIterator {
            nodes: &scene.nodes,
            next: Some(id),
        }
    }
}

impl<'a> Iterator for SceneIterator<'a> {
    type Item = &'a Node;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next {
            None => None,
            Some(id) => {
                let n = &self.nodes[id];
                self.next = n.parent;
                Some(&n.node)
            }
        }
    }
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
