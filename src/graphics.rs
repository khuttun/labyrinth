use nalgebra_glm as glm;
use raw_window_handle::HasRawWindowHandle;
use std::{iter, num::NonZeroU32, rc::Rc};

const MAX_LIGHTS: usize = 4;

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
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    swap_chain_descriptor: wgpu::SwapChainDescriptor,
    swap_chain: Option<wgpu::SwapChain>,
    msaa_framebuffer: wgpu::TextureView,
    depth_buffer: wgpu::TextureView,
    shadow_maps: wgpu::Texture,
    shadow_maps_bind_group: wgpu::BindGroup,
    scene_uniform_bind_group_layout: wgpu::BindGroupLayout,
    object_uniform_bind_group_layout: wgpu::BindGroupLayout,
    object_texture_bind_group_layout: wgpu::BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
    shadow_pass_uniform_buffer: wgpu::Buffer,
    shadow_pass_uniform_bind_group: wgpu::BindGroup,
    shadow_pass_pipeline: wgpu::RenderPipeline,
    render_2d_pipeline: wgpu::RenderPipeline,
}

impl Instance {
    pub async fn new(config: Config, width: u32, height: u32) -> Instance {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let swap_chain_descriptor = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            #[cfg(not(target_os = "android"))]
            format: wgpu::TextureFormat::Bgra8Unorm,
            #[cfg(target_os = "android")]
            format: wgpu::TextureFormat::Rgba8Unorm,
            width,
            height,
            present_mode: if config.vsync {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::Immediate
            },
        };

        let msaa_framebuffer = device
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
                format: swap_chain_descriptor.format,
                usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let depth_buffer_desc = wgpu::TextureDescriptor {
            label: Some("Depth buffer texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: config.msaa_samples,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        };

        let depth_buffer = device
            .create_texture(&depth_buffer_desc)
            .create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_maps_desc = wgpu::TextureDescriptor {
            label: Some("Shadow maps texture"),
            size: wgpu::Extent3d {
                width: width.min(height),
                height: width.min(height),
                depth: MAX_LIGHTS as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        };

        let shadow_maps = device.create_texture(&shadow_maps_desc);

        let shadow_maps_view = shadow_maps.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow map sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let shadow_maps_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow maps bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            filtering: false,
                            comparison: true,
                        },
                        count: None,
                    },
                ],
            });

        let shadow_maps_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow bind group"),
            layout: &shadow_maps_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_maps_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_map_sampler),
                },
            ],
        });

        let scene_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Scene uniform bind group layout"),
                entries: &[
                    // Scene uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Light uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let object_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Object uniform bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let object_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Object texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            filtering: true,
                            comparison: false,
                        },
                        count: None,
                    },
                ],
            });

        let vs_module = device.create_shader_module(&wgpu::include_spirv!("default.vert.spv"));
        let fs_module = device.create_shader_module(&wgpu::include_spirv!("default.frag.spv"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render pipeline layout"),
                bind_group_layouts: &[
                    &scene_uniform_bind_group_layout,
                    &shadow_maps_bind_group_layout,
                    &object_uniform_bind_group_layout,
                    &object_texture_bind_group_layout,
                ],
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
                format: swap_chain_descriptor.format,
                // Blending for straight alpha
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
                format: depth_buffer_desc.format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilStateDescriptor::default(),
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: Some(Vertex::index_format()),
                vertex_buffers: &[Vertex::buffer_descriptor()],
            },
            sample_count: config.msaa_samples,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let shadow_pass_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow pass uniform buffer"),
            size: std::mem::size_of::<RawMat4>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_pass_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow pass uniform bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let shadow_pass_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow pass uniform bind group"),
            layout: &shadow_pass_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_pass_uniform_buffer.as_entire_binding(),
            }],
        });

        let shadow_pass_vs_module =
            device.create_shader_module(&wgpu::include_spirv!("shadow.vert.spv"));

        let shadow_pass_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow pipeline layout"),
                bind_group_layouts: &[
                    &shadow_pass_uniform_bind_group_layout,
                    &object_uniform_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let shadow_pass_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow pipeline"),
            layout: Some(&shadow_pass_pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &shadow_pass_vs_module,
                entry_point: "main",
            },
            fragment_stage: None,
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::Back,
                polygon_mode: wgpu::PolygonMode::Fill,
                depth_bias: 10,
                depth_bias_slope_scale: 2.0,
                depth_bias_clamp: 0.0,
                clamp_depth: false,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: shadow_maps_desc.format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilStateDescriptor::default(),
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: Some(Vertex::index_format()),
                vertex_buffers: &[Vertex::buffer_descriptor()],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let vs_2d_module = device.create_shader_module(&wgpu::include_spirv!("2d.vert.spv"));
        let fs_2d_module = device.create_shader_module(&wgpu::include_spirv!("2d.frag.spv"));

        let render_2d_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("2D Render pipeline layout"),
                bind_group_layouts: &[
                    &object_uniform_bind_group_layout,
                    &object_texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_2d_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("2D Render pipeline"),
            layout: Some(&render_2d_pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_2d_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_2d_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                // 2D rendering is mainly meant for rendering UI produced by some immediate mode UI
                // library. Don't cull triangle backfaces as there's no guarantees in what order
                // the libraries produce the triangles.
                cull_mode: wgpu::CullMode::None,
                polygon_mode: wgpu::PolygonMode::Fill,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
                clamp_depth: false,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: swap_chain_descriptor.format,
                // Blending for premultiplied alpha
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: Some(Vertex2d::index_format()),
                vertex_buffers: &[Vertex2d::buffer_descriptor()],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        Instance {
            config,
            width,
            height,
            instance,
            device,
            queue,
            swap_chain_descriptor,
            swap_chain: None,
            msaa_framebuffer,
            depth_buffer,
            shadow_maps,
            shadow_maps_bind_group,
            scene_uniform_bind_group_layout,
            object_uniform_bind_group_layout,
            object_texture_bind_group_layout,
            render_pipeline,
            shadow_pass_uniform_buffer,
            shadow_pass_uniform_bind_group,
            shadow_pass_pipeline,
            render_2d_pipeline,
        }
    }

    pub fn set_window<W: HasRawWindowHandle>(&mut self, window: Option<&W>) {
        self.swap_chain = if let Some(w) = window {
            Some(self.device.create_swap_chain(
                &unsafe { self.instance.create_surface(w) },
                &self.swap_chain_descriptor,
            ))
        } else {
            None
        }
    }

    pub fn create_scene(&self) -> Scene {
        Scene::new(self)
    }

    pub fn create_shape(&self, name: &str, ply: &str) -> Shape {
        Shape::from_ply(self, name, ply)
    }

    pub fn create_shape_2d(&self, name: &str, vertices: &[Vertex2d], indices: &[u32]) -> Shape {
        Shape::from_2d_triangles(self, name, vertices, indices)
    }

    pub fn create_texture(&self, name: &str, w: u32, h: u32, rgba_data: &[u8]) -> Texture {
        Texture::from_image(self, name, w, h, rgba_data)
    }

    pub fn create_object(&self, s: &Rc<Shape>, t: &Rc<Texture>) -> Node {
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Object uniform buffer"),
            size: std::mem::size_of::<ObjectUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Object uniform bind group"),
            layout: &self.object_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        });
        Node::new(NodeKind::Object(Object {
            shape: Rc::clone(s),
            texture: Rc::clone(t),
            uniform_buffer: buf,
            uniform_bind_group: bind_group,
        }))
    }

    pub fn create_transformation(&self) -> Node {
        Node::new(NodeKind::Transformation)
    }

    pub fn create_object_2d(&self, s: &Rc<Shape>, t: &Rc<Texture>) -> Object2d {
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("2D Object uniform buffer"),
            size: std::mem::size_of::<Object2dUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("2D Object uniform bind group"),
            layout: &self.object_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        });
        Object2d::new(Object {
            shape: Rc::clone(s),
            texture: Rc::clone(t),
            uniform_buffer: buf,
            uniform_bind_group: bind_group,
        })
    }

    pub fn add_light_to(
        &self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        z: f32,
        point_at_x: f32,
        point_at_y: f32,
        point_at_z: f32,
    ) {
        let light_index = scene.lights.len();
        assert!(
            light_index < MAX_LIGHTS,
            "Too many lights added to the scene"
        );
        scene.lights.push(Light {
            position: glm::vec3(x, y, z),
            point_at: glm::vec3(point_at_x, point_at_y, point_at_z),
            shadow_map: self.shadow_maps.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("Shadow map {}", light_index)),
                format: None,
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                level_count: None,
                base_array_layer: light_index as u32,
                array_layer_count: NonZeroU32::new(1),
            }),
        });
    }

    pub fn render_scene(&self, scene: &Scene, objects2d: &[Object2d]) {
        if self.swap_chain.is_none() {
            return;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render encoder"),
            });

        // 1. Update uniforms
        self.queue.write_buffer(
            &scene.scene_uniform_buffer,
            0,
            bytemuck::cast_slice(&[SceneUniforms::from(
                &(scene.perspective_matrix * scene.view_matrix),
                scene.lights.len() as u32,
            )]),
        );
        for (i, light) in scene.lights.iter().enumerate() {
            self.queue.write_buffer(
                &scene.light_uniform_buffer,
                (i * std::mem::size_of::<LightUniforms>()) as wgpu::BufferAddress,
                bytemuck::cast_slice(&[LightUniforms::from(&scene.light_projection_matrix, light)]),
            );
        }
        for (id, n) in scene.nodes.iter().enumerate() {
            match &n.node.kind {
                NodeKind::Object(Object {
                    shape: _,
                    texture: _,
                    uniform_buffer,
                    uniform_bind_group: _,
                }) => {
                    let effective_model_matrix = SceneIterator::new(scene, id)
                        .fold(glm::identity(), |acc, node| node.model_matrix * acc);
                    self.queue.write_buffer(
                        &uniform_buffer,
                        0,
                        bytemuck::cast_slice(&[ObjectUniforms::from(&effective_model_matrix)]),
                    );
                }
                NodeKind::Transformation => (), // no uniforms to update
            }
        }
        for obj in objects2d.iter() {
            self.queue.write_buffer(
                &obj.object.uniform_buffer,
                0,
                bytemuck::cast_slice(&[Object2dUniforms::from(&obj.model_matrix)]),
            );
        }

        // 2. Create shadow maps
        for (i, light) in scene.lights.iter().enumerate() {
            // The "view-projection" matrix for the light already exists in the light uniform buffer
            // -> copy it to the shadow pass uniform buffer
            encoder.copy_buffer_to_buffer(
                &scene.light_uniform_buffer,
                (i * std::mem::size_of::<LightUniforms>()) as wgpu::BufferAddress,
                &self.shadow_pass_uniform_buffer,
                0,
                std::mem::size_of::<RawMat4>() as wgpu::BufferAddress,
            );

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(
                        wgpu::RenderPassDepthStencilAttachmentDescriptor {
                            attachment: &light.shadow_map,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: true,
                            }),
                            stencil_ops: None,
                        },
                    ),
                });

                render_pass.set_pipeline(&self.shadow_pass_pipeline);
                render_pass.set_bind_group(0, &self.shadow_pass_uniform_bind_group, &[]);

                for n in scene.nodes.iter() {
                    match &n.node.kind {
                        NodeKind::Object(Object {
                            shape,
                            texture: _,
                            uniform_buffer: _,
                            uniform_bind_group,
                        }) => {
                            render_pass.set_bind_group(1, &uniform_bind_group, &[]);
                            render_pass.set_vertex_buffer(0, shape.vertex_buffer.slice(..));
                            render_pass.set_index_buffer(
                                shape.index_buffer.slice(..),
                                Vertex::index_format(),
                            );
                            render_pass.draw_indexed(0..shape.index_count as u32, 0, 0..1);
                        }
                        NodeKind::Transformation => (), // nothing to draw
                    }
                }
            }
        }

        let frame = self.swap_chain.as_ref().unwrap().get_current_frame();
        if let Err(e) = frame {
            eprintln!("Failed to get frame: {}", e);
            return;
        }
        let frame = frame.unwrap().output;

        // 3. Render the scene
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene render pass"),
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &self.msaa_framebuffer,
                    resolve_target: Some(&frame.view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &self.depth_buffer,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &scene.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.shadow_maps_bind_group, &[]);

            for n in scene.nodes.iter() {
                match &n.node.kind {
                    NodeKind::Object(Object {
                        shape,
                        texture,
                        uniform_buffer: _,
                        uniform_bind_group,
                    }) => {
                        render_pass.set_bind_group(2, &uniform_bind_group, &[]);
                        render_pass.set_bind_group(3, &texture.bind_group, &[]);
                        render_pass.set_vertex_buffer(0, shape.vertex_buffer.slice(..));
                        render_pass
                            .set_index_buffer(shape.index_buffer.slice(..), Vertex::index_format());
                        render_pass.draw_indexed(0..shape.index_count as u32, 0, 0..1);
                    }
                    NodeKind::Transformation => (), // nothing to draw
                }
            }
        }

        // 4. Render any 2D graphics on top of the scene
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("2D Render pass"),
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_2d_pipeline);

            for obj in objects2d.iter() {
                render_pass.set_bind_group(0, &obj.object.uniform_bind_group, &[]);
                render_pass.set_bind_group(1, &obj.object.texture.bind_group, &[]);
                render_pass.set_vertex_buffer(0, obj.object.shape.vertex_buffer.slice(..));
                render_pass.set_index_buffer(
                    obj.object.shape.index_buffer.slice(..),
                    Vertex2d::index_format(),
                );
                render_pass.draw_indexed(0..obj.object.shape.index_count as u32, 0, 0..1);
            }
        }

        self.queue.submit(iter::once(encoder.finish()));
    }
}

pub struct Scene {
    nodes: Vec<SceneNode>,
    lights: Vec<Light>,
    view_matrix: glm::Mat4x4,
    perspective_matrix: glm::Mat4x4,
    light_projection_matrix: glm::Mat4x4,
    scene_uniform_buffer: wgpu::Buffer,
    light_uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl Scene {
    fn new(inst: &Instance) -> Scene {
        let scene_uniform_buffer = inst.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Scene uniform buffer"),
            size: std::mem::size_of::<SceneUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        let light_uniform_buffer = inst.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light uniform buffer"),
            size: (MAX_LIGHTS * std::mem::size_of::<LightUniforms>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::UNIFORM
                | wgpu::BufferUsage::COPY_DST
                | wgpu::BufferUsage::COPY_SRC,
            mapped_at_creation: false,
        });
        let uniform_bind_group = inst.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scene uniform bind group"),
            layout: &inst.scene_uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: scene_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_uniform_buffer.as_entire_binding(),
                },
            ],
        });
        Scene {
            nodes: Vec::new(),
            lights: Vec::new(),
            view_matrix: glm::look_at(
                &glm::vec3(0.0, 5.0, 5.0),
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 1.0, 0.0),
            ),
            // Use the _zo version to work with wgpu 0..1 depth coordinates
            perspective_matrix: glm::perspective_zo(
                inst.width as f32 / inst.height as f32,
                glm::radians(&glm::vec1(45.0)).x,
                2.0,
                2000.0,
            ),
            light_projection_matrix: glm::perspective_zo(
                1.0,
                glm::radians(&glm::vec1(75.0)).x,
                2.0,
                2000.0,
            ),
            scene_uniform_buffer,
            light_uniform_buffer,
            uniform_bind_group,
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
}

pub struct Shape {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: usize,
}

impl Shape {
    fn from_ply(inst: &Instance, name: &str, ply: &str) -> Shape {
        // Helper functions for reading PLY properties
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

    fn from_2d_triangles(
        inst: &Instance,
        name: &str,
        vertices: &[Vertex2d],
        indices: &[u32],
    ) -> Shape {
        let vertex_buffer = inst.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("Vertex buffer {}", name)),
            size: (vertices.len() * std::mem::size_of::<Vertex2d>()) as wgpu::BufferAddress,
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
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = inst.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Texture bind group {}", name)),
            layout: &inst.object_texture_bind_group_layout,
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

struct Object {
    shape: Rc<Shape>,
    texture: Rc<Texture>,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

enum NodeKind {
    Object(Object),
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

pub struct Object2d {
    object: Object,
    scaling: glm::Vec2,
    translation: glm::Vec2,
    model_matrix: glm::Mat3x3,
}

impl Object2d {
    fn new(object: Object) -> Object2d {
        Object2d {
            object,
            scaling: glm::vec2(1.0, 1.0),
            translation: glm::vec2(0.0, 0.0),
            model_matrix: glm::identity(),
        }
    }

    pub fn set_scaling(&mut self, x: f32, y: f32) {
        self.scaling = glm::vec2(x, y);
        self.update_model_matrix();
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.translation = glm::vec2(x, y);
        self.update_model_matrix();
    }

    fn update_model_matrix(&mut self) {
        self.model_matrix = glm::translation2d(&self.translation) * glm::scaling2d(&self.scaling);
    }
}

struct Light {
    position: glm::Vec3,
    point_at: glm::Vec3,
    shadow_map: wgpu::TextureView,
}

// Data for one vertex
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn buffer_descriptor<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        wgpu::VertexBufferDescriptor {
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
        }
    }

    fn index_format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint32
    }
}

// Data for one 2D vertex
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
pub struct Vertex2d {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
    pub color: [u8; 4],
}

impl Vertex2d {
    fn buffer_descriptor<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        wgpu::VertexBufferDescriptor {
            stride: std::mem::size_of::<Vertex2d>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    // position
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float2,
                },
                wgpu::VertexAttributeDescriptor {
                    // tex_coords
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float2,
                },
                wgpu::VertexAttributeDescriptor {
                    // color
                    offset: 2 * std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uchar4Norm,
                },
            ],
        }
    }

    fn index_format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint32
    }
}

type RawMat4 = [[f32; 4]; 4];

// Uniforms related to a single light
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
struct LightUniforms {
    view_projection: RawMat4, // A matrix projecting a coordinate from world space to the light's "clip space"
    pos_world_space: [f32; 4], // Only xyz components used. The vector is 4D to satisfy GLSL uniform alignment requirements.
}

impl LightUniforms {
    fn from(projection: &glm::Mat4, light: &Light) -> LightUniforms {
        LightUniforms {
            view_projection: (projection
                * glm::look_at(&light.position, &light.point_at, &glm::vec3(0.0, 1.0, 0.0)))
            .into(),
            pos_world_space: glm::vec3_to_vec4(&light.position).into(),
        }
    }
}

// Uniforms related to the whole scene
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
struct SceneUniforms {
    view_projection: RawMat4,
    num_lights: [u32; 4], // Only x component used
}

impl SceneUniforms {
    fn from(view_projection: &glm::Mat4, num_lights: u32) -> SceneUniforms {
        SceneUniforms {
            view_projection: view_projection.clone().into(),
            num_lights: [num_lights, 0, 0, 0],
        }
    }
}

// Uniforms related to one scene object
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
struct ObjectUniforms {
    model: RawMat4,
    model_normal: RawMat4,
}

impl ObjectUniforms {
    fn from(m: &glm::Mat4) -> ObjectUniforms {
        ObjectUniforms {
            model: m.clone().into(),
            model_normal: glm::transpose(&glm::inverse(m)).into(),
        }
    }
}

// Uniforms related to one 2D object
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
struct Object2dUniforms {
    model: RawMat4, // Actually mat3, but passed like this for proper alignment
}

impl Object2dUniforms {
    fn from(m: &glm::Mat3) -> Object2dUniforms {
        Object2dUniforms {
            model: glm::mat3_to_mat4(m).into(),
        }
    }
}
