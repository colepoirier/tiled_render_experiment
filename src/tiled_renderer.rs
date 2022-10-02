use bevy::{
    asset::HandleId,
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::Viewport,
        mesh::PrimitiveTopology,
        render_resource::{AsBindGroup, ShaderRef},
        renderer::RenderQueue,
    },
    sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle},
};
use bevy_prototype_lyon::prelude::*;
use crossbeam_channel::bounded;

use crate::{
    path_to_poly::make_path_into_polygon,
    types::{
        DrawTileEvent, FlattenedElems, Layers, LibLayers, LyonShape, LyonShapeBundle,
        RenderingCompleteEvent, RenderingDoneChannel, Tilemap, TilemapLowerLeft, ALPHA,
        DOWNSCALING_PASS_LAYER, WIDTH,
    },
};
use crate::{
    types::{AccumulationCam, HiResCam},
    HiResHandle,
};
use layout21::raw;

pub const TILE_SIZE: u32 = 64;

pub struct TiledRendererPlugin;

#[derive(StageLabel)]
enum TiledRenderStage {
    SpawnCameras,
    SpawnShapes,
    Despawn,
}

impl Plugin for TiledRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ShapePlugin)
            .add_plugin(Material2dPlugin::<PostProcessingMaterial>::default())
            .insert_resource({
                let (sender, receiver) = bounded::<()>(1);
                RenderingDoneChannel { sender, receiver }
            })
            .add_stage_after(
                CoreStage::Update,
                TiledRenderStage::SpawnCameras,
                SystemStage::parallel(),
            )
            .add_stage_after(
                TiledRenderStage::SpawnCameras,
                TiledRenderStage::SpawnShapes,
                SystemStage::parallel(),
            )
            .add_stage_after(
                TiledRenderStage::SpawnShapes,
                TiledRenderStage::Despawn,
                SystemStage::parallel(),
            )
            .add_system_to_stage(TiledRenderStage::SpawnCameras, spawn_cameras_system)
            .add_system_to_stage(TiledRenderStage::SpawnShapes, spawn_shapes_system)
            .add_system_to_stage(TiledRenderStage::Despawn, despawn_system);
        // .add_system(debug_image_handles);
    }
}

#[allow(unused)]
fn debug_image_handles(q: Query<&Handle<Image>>) {
    for h in q.iter() {
        info!(
            "{:x}",
            if let HandleId::Id(_, id) = h.id {
                id
            } else {
                0
            }
        );
    }
}

fn spawn_shapes_system(
    mut commands: Commands,
    tilemap: Res<Tilemap>,
    flattened_elems: Res<FlattenedElems>,
    lib_layers: Res<LibLayers>,
    layers: Res<Layers>,
    mut draw_ev: EventReader<DrawTileEvent>,
    mut existing_lyon_shapes: Query<
        (
            &mut bevy_prototype_lyon::entity::Path,
            &mut Transform,
            &mut Visibility,
        ),
        With<LyonShape>,
    >,
) {
    for DrawTileEvent(key) in draw_ev.iter() {
        let tile = tilemap.get(key).unwrap();

        // let read_lib_layers = lib_layers.read().unwrap();
        // let mut bundle_vec = Vec::with_capacity(tile.shapes.len());

        info!("Num shapes in this tile: {}", tile.shapes.len());

        let mut existing_shapes_iter = existing_lyon_shapes.iter_mut();
        for idx in tile.shapes.iter() {
            let el = &(**flattened_elems)[*idx];

            let layer = lib_layers
                .get(el.layer)
                .expect("This Element's LayerKey does not exist in this Library's Layers")
                .layernum as u8;

            let color = layers.get(&layer).unwrap();

            match &el.inner {
                raw::Shape::Rect(r) => {
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

                    let bundle = LyonShapeBundle {
                        lyon: lyon_shape,
                        marker: LyonShape,
                    };

                    if let Some((mut existing_path, mut existing_transform, mut vis)) =
                        existing_shapes_iter.next()
                    {
                        *existing_path = bundle.lyon.path;
                        *existing_transform = bundle.lyon.transform;
                        vis.is_visible = true;
                    } else {
                        commands.spawn_bundle(bundle);
                    }
                }
                raw::Shape::Polygon(poly) => {
                    let lyon_poly = shapes::Polygon {
                        points: poly
                            .points
                            .iter()
                            .map(|c| Vec2::new(c.x as f32, c.y as f32))
                            .collect::<Vec<Vec2>>(),
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
                                options: StrokeOptions::default().with_line_width(WIDTH),
                                color: *color,
                            },
                        },
                        transform,
                    );

                    let bundle = LyonShapeBundle {
                        lyon: lyon_shape,
                        marker: LyonShape,
                    };

                    if let Some((mut existing_path, mut existing_transform, mut vis)) =
                        existing_shapes_iter.next()
                    {
                        *existing_path = bundle.lyon.path;
                        *existing_transform = bundle.lyon.transform;
                        vis.is_visible = true;
                    } else {
                        commands.spawn_bundle(bundle);
                    }
                }
                raw::Shape::Path(path) => {
                    let poly = make_path_into_polygon(path);
                    let lyon_poly = shapes::Polygon {
                        points: poly
                            .exterior()
                            .points()
                            .map(|c| Vec2::new(c.x() as f32, c.y() as f32))
                            .collect::<Vec<Vec2>>(),
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
                                options: StrokeOptions::default().with_line_width(WIDTH),
                                color: *color,
                            },
                        },
                        transform,
                    );

                    let bundle = LyonShapeBundle {
                        lyon: lyon_shape,
                        marker: LyonShape,
                    };

                    if let Some((mut existing_path, mut existing_transform, mut vis)) =
                        existing_shapes_iter.next()
                    {
                        *existing_path = bundle.lyon.path;
                        *existing_transform = bundle.lyon.transform;
                        vis.is_visible = true;
                    } else {
                        commands.spawn_bundle(bundle);
                    }
                }
            }
        }
    }
}

fn spawn_cameras_system(
    mut commands: Commands,
    hires_image: Res<HiResHandle>,
    mut mesh_and_material: Local<Option<(Handle<Mesh>, Handle<PostProcessingMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut post_processing_materials: ResMut<Assets<PostProcessingMaterial>>,
    render_queue: Res<RenderQueue>,
    tilemap: Res<Tilemap>,
    lower_left_res: Res<TilemapLowerLeft>,
    mut draw_ev: EventReader<DrawTileEvent>,
    rendering_done_channel: Res<RenderingDoneChannel>,
    mut rendering_complete_ev: EventWriter<RenderingCompleteEvent>,
    mut hires_cam_q: Query<(&mut Camera, &mut Transform), With<HiResCam>>,
    mut accumulation_cam_q: Query<&mut Camera, (With<AccumulationCam>, Without<HiResCam>)>,
) {
    for DrawTileEvent(key) in draw_ev.iter() {
        if key.0 % 15 == 0 {
            rendering_complete_ev.send_default();
            continue;
        }

        let tile = tilemap.get(key).unwrap();

        let tile_x = tile.extents.min().x - lower_left_res.x;
        let tile_y = tile.extents.min().y - lower_left_res.y;

        assert!(tile_x >= 0, "tile_x should be positive");
        assert!(tile_y >= 0, "tile_y should be positive");
        let x = tile_x;
        let y = tile_y;

        let transform = Transform::from_translation(Vec3::new(x as f32, y as f32, 999.0));

        for (mut cam, mut cam_transform) in hires_cam_q.iter_mut() {
            cam.is_active = true;
            *cam_transform = transform;
        }

        let (mesh_handle, material_handle) =
            if let Some((mesh_handle, material_handle)) = mesh_and_material.as_ref() {
                ((*mesh_handle).clone(), (*material_handle).clone())
            } else {
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
                    source_image: hires_image.clone(),
                });

                *mesh_and_material = Some((mesh_handle.clone(), material_handle.clone()));

                (mesh_handle, material_handle)
            };

        // Post processing 2d quad, with material using the render texture done by the main camera, with a custom shader.
        commands
            .spawn_bundle(MaterialMesh2dBundle {
                mesh: mesh_handle.into(),
                material: material_handle,
                ..default()
            })
            .insert(DOWNSCALING_PASS_LAYER);

        let physical_position =
            UVec2::new((x / TILE_SIZE as i64) as u32, (y / TILE_SIZE as i64) as u32);

        info!("viewport: {physical_position:?}");

        for mut cam in accumulation_cam_q.iter_mut() {
            cam.is_active = true;
            cam.viewport = Some(Viewport {
                physical_position,
                physical_size: UVec2::new(TILE_SIZE, TILE_SIZE),
                ..default()
            });
        }

        let s = rendering_done_channel.sender.clone();

        let key = key.clone();

        render_queue.on_submitted_work_done(move || {
            s.send(()).unwrap();
            info!("work done event sent for tile {key:?}!");
        });
    }
}

fn despawn_system(
    mut hires_cam_q: Query<&mut Camera, With<HiResCam>>,
    mut accumulation_cam_q: Query<&mut Camera, (With<AccumulationCam>, Without<HiResCam>)>,
    mut shape_q: Query<&mut Visibility, With<LyonShape>>,
    rendering_done_channel: Res<RenderingDoneChannel>,
    mut rendering_complete_ev: EventWriter<RenderingCompleteEvent>,
) {
    if let Ok(()) = rendering_done_channel.receiver.try_recv() {
        info!("RENDERING DONE");
        for mut cam in hires_cam_q.iter_mut() {
            cam.is_active = false;
        }

        for mut cam in accumulation_cam_q.iter_mut() {
            cam.is_active = false;
        }

        for mut vis in shape_q.iter_mut() {
            vis.is_visible = false;
        }

        rendering_complete_ev.send_default();
    }
}

#[derive(AsBindGroup, TypeUuid, Clone)]
#[uuid = "bc2f08eb-a0fb-43f1-a908-54871ea597d5"]
struct PostProcessingMaterial {
    /// This image will be the result of the main camera.
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
