use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::{CameraProjection, RenderTarget, WindowOrigin},
        mesh::PrimitiveTopology,
        render_resource::{
            AsBindGroup, Extent3d, ShaderRef, TextureDescriptor, TextureDimension, TextureFormat,
            TextureUsages,
        },
        renderer::RenderQueue,
        view::RenderLayers,
    },
    sprite::{Anchor, Material2d, Material2dPlugin, MaterialMesh2dBundle},
};
use bevy_prototype_lyon::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{DrawTileEvent, FlattenedElems, Layers, LibLayers, TileMap};
use layout21::raw;

pub const ALPHA: f32 = 0.1;
pub const WIDTH: f32 = 10.0;

pub const MAIN_CAMERA_LAYER: RenderLayers = RenderLayers::layer(2);

pub struct TiledRendererPlugin;

impl Plugin for TiledRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ShapePlugin)
            .add_plugin(Material2dPlugin::<PostProcessingMaterial>::default())
            .insert_resource({
                let (sender, receiver) = bounded::<()>(1);
                RenderingDoneChannel { sender, receiver }
            })
            .add_system(despawn_system)
            .add_system(spawn_system);
    }
}

struct RenderingDoneChannel {
    sender: Sender<()>,
    receiver: Receiver<()>,
}

// Marks the first pass cube (rendered to a texture.)
#[derive(Component)]
struct LyonShape;

// Marks the first pass cube (rendered to a texture.)
#[derive(Component)]
struct TextureCam;

#[derive(Debug, Clone, Copy, Component)]
pub struct HiResTileMarker;

#[derive(Debug, Clone, Copy, Component)]
pub struct DownscaledTileMarker;

fn spawn_system(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut post_processing_materials: ResMut<Assets<PostProcessingMaterial>>,
    render_queue: Res<RenderQueue>,
    tilemap: Res<TileMap>,
    flattened_elems: Res<FlattenedElems>,
    lib_layers: Res<LibLayers>,
    layers: Res<Layers>,
    mut ev: EventReader<DrawTileEvent>,
    rendering_done_channel: Res<RenderingDoneChannel>,
) {
    for DrawTileEvent(key) in ev.iter() {
        let size = Extent3d {
            width: 4096,
            height: 4096,
            ..default()
        };

        // This is the texture that will be rendered to.
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..default()
        };

        // fill image.data with zeroes
        image.resize(size);
        // halfsized_image.resize(half_size);

        let source_image_handle = images.add(image);
        // let halfsized_image_handle = images.add(halfsized_image);

        let tile = tilemap.get(key).unwrap();

        // let read_lib_layers = lib_layers.read().unwrap();

        for idx in tile.shapes.iter() {
            let el = &(**flattened_elems)[*idx];

            let layer = lib_layers
                .get(el.layer)
                .expect("This Element's LayerKey does not exist in this Library's Layers")
                .layernum as u8;

            let color = layers.get(&layer).unwrap();

            if let raw::Shape::Rect(r) = &el.inner {
                let raw::Rect { p0, p1 } = r;
                let xmin = p0.x / 4;
                let ymin = p0.y / 4;
                let xmax = p1.x / 4;
                let ymax = p1.y / 4;

                let lyon_poly = shapes::Polygon {
                    points: vec![
                        (xmin as f32, ymin as f32).into(),
                        (xmax as f32, ymin as f32).into(),
                        (xmax as f32, ymax as f32).into(),
                        (xmin as f32, ymax as f32).into(),
                    ],
                    closed: true,
                };

                let transform = Transform::from_translation(Vec3::new(0.0, 0.0, layer as f32));

                let lyon_shape = GeometryBuilder::build_as(
                    &lyon_poly,
                    DrawMode::Outlined {
                        fill_mode: FillMode {
                            color: *color.clone().set_a(ALPHA),
                            options: FillOptions::default(),
                        },
                        outline_mode: StrokeMode {
                            color: *color,
                            options: StrokeOptions::default().with_line_width(WIDTH),
                        },
                    },
                    transform,
                );

                commands
                    .spawn_bundle(lyon_shape)
                    .insert_bundle(VisibilityBundle::default())
                    .insert(LyonShape);
            }
        }

        let x = tile.extents.min().x / 4;
        let y = tile.extents.min().y / 4;

        info!("setting camera transform to {x}, {y}");

        let transform = Transform::from_translation(Vec3::new(x as f32, y as f32, 999.0));

        let mut camera = Camera2dBundle {
            camera_2d: Camera2d::default(),
            camera: Camera {
                target: RenderTarget::Image(source_image_handle.clone()),
                ..default()
            },
            transform,
            ..default()
        };

        camera.projection.window_origin = WindowOrigin::BottomLeft;

        info!("{:?}", camera.projection);
        camera
            .projection
            .update(size.width as f32, size.height as f32);

        info!("{:?}", camera.projection);

        commands.spawn_bundle(camera).insert(TextureCam);

        let downscaling_pass_layer = RenderLayers::layer(1);

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[-1.0, 1.0, 0.0], [-1.0, -3.0, 0.0], [3.0, 1.0, 0.0]],
        );

        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 0.0], [0.0, 2.0], [2.0, 0.0]],
        );

        mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
        );

        let mesh_handle = meshes.add(mesh);

        // This material has the texture that has been rendered.
        let material_handle = post_processing_materials.add(PostProcessingMaterial {
            source_image: source_image_handle.clone(),
        });

        // Post processing 2d quad, with material using the render texture done by the main camera, with a custom shader.
        commands
            .spawn_bundle(MaterialMesh2dBundle {
                mesh: mesh_handle.into(),
                material: material_handle,
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, 1.5),
                    ..default()
                },
                ..default()
            })
            .insert(downscaling_pass_layer);

        let size = Extent3d {
            width: 64,
            height: 64,
            ..default()
        };

        // This is the texture that will be rendered to.
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..default()
        };

        // fill image.data with zeroes
        image.resize(size);

        let downscaled_image_handle = images.add(image);

        // The downscaling pass camera.
        commands
            .spawn_bundle(Camera2dBundle {
                camera: Camera {
                    // renders after the first main camera which has default value: 0.
                    priority: 1,
                    target: RenderTarget::Image(downscaled_image_handle.clone()),
                    ..default()
                },
                ..Camera2dBundle::default()
            })
            .insert(downscaling_pass_layer)
            .insert(TextureCam);

        commands
            .spawn_bundle(SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(size.width as f32, size.height as f32)),
                    anchor: Anchor::BottomLeft,
                    ..default()
                },
                texture: downscaled_image_handle.clone(),
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
                ..default()
            })
            .insert(MAIN_CAMERA_LAYER);

        let s = rendering_done_channel.sender.clone();

        render_queue.on_submitted_work_done(move || {
            s.send(()).unwrap();
            info!("work done event sent!");
        });
    }
}

fn despawn_system(
    mut commands: Commands,
    cam_q: Query<Entity, With<TextureCam>>,
    shape_q: Query<Entity, With<LyonShape>>,
    rendering_done_channel: Res<RenderingDoneChannel>,
) {
    if let Ok(()) = rendering_done_channel.receiver.try_recv() {
        info!("RENDERING DONE");
        for cam in cam_q.iter() {
            info!("despawn camera");
            commands.entity(cam).despawn();
        }
        for s in shape_q.iter() {
            // info!("despawn shape");
            commands.entity(s).despawn();
        }
    }
}

#[derive(AsBindGroup, TypeUuid, Clone)]
#[uuid = "bc2f08eb-a0fb-43f1-a908-54871ea597d5"]
struct PostProcessingMaterial {
    /// In this example, this image will be the result of the main camera.
    #[texture(0)]
    #[sampler(1)]
    source_image: Handle<Image>,
}

impl Material2d for PostProcessingMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/fullscreen.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/downscale.wgsl".into()
    }
}
