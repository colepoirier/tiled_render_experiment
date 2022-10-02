use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    prelude::*,
    render::{
        camera::{CameraProjection, RenderTarget, Viewport, WindowOrigin},
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::RenderDevice,
    },
    sprite::Anchor,
    tasks::{AsyncComputeTaskPool, Task},
};

use bevy_pancam::{PanCam, PanCamPlugin};

use futures_lite::future;
use geo::Intersects;
use itertools::Itertools;
use layout21::raw::{self, proto::ProtoImporter, BoundBox, BoundBoxTrait, Library};

pub mod tiled_renderer;

use tiled_renderer::TiledRendererPlugin;

use crate::{
    tiled_renderer::TILE_SIZE,
    types::{
        AccumulationCam, AccumulationHandle, GeoRect, Tile, ACCUMULATION_CAMERA_PRIORITY,
        DOWNSCALING_PASS_LAYER,
    },
    utils::{get_grid_shape, tilemap_stats_and_debug},
};

mod path_to_poly;
mod types;
mod utils;

use path_to_poly::make_path_into_polygon;

use types::{
    DrawTileEvent, FlattenedElems, GeoPolygon, GeoShapeEnum, HiResCam, HiResHandle, LayerColors,
    Layers, LibLayers, LibraryWrapper, MainCamera, OpenVlsirLibCompleteEvent,
    RenderingCompleteEvent, TileIndexIter, Tilemap, TilemapLowerLeft, VlsirLib, MAIN_CAMERA_LAYER,
    MAIN_CAMERA_PRIORITY, TEXTURE_DIM,
};

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            present_mode: bevy::window::PresentMode::Immediate,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(PanCamPlugin)
        .add_plugin(TiledRendererPlugin)
        .init_resource::<LayerColors>()
        .init_resource::<OpenVlsirLibCompleteEvent>()
        .init_resource::<FlattenedElems>()
        .init_resource::<Tilemap>()
        .init_resource::<TilemapLowerLeft>()
        .init_resource::<Layers>()
        .init_resource::<LibLayers>()
        .init_resource::<VlsirLib>()
        .init_resource::<TileIndexIter>()
        .add_event::<OpenVlsirLibCompleteEvent>()
        .add_event::<DrawTileEvent>()
        .add_event::<TileIndexIter>()
        .add_event::<RenderingCompleteEvent>()
        .insert_resource(Msaa { samples: 1 })
        .add_startup_system(setup)
        .add_system(spawn_vlsir_open_task_sytem)
        .add_system(handle_vlsir_open_task_system)
        .add_system(load_lib_system)
        .add_system(iter_tile_index_system)
        .add_system(camera_changed_system)
        .run();
}

fn initialize_hi_res_resources(commands: &mut Commands, images: &mut Assets<Image>) {
    let size = Extent3d {
        width: TEXTURE_DIM,
        height: TEXTURE_DIM,
        ..default()
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("HIRES_TEXTURE"),
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

    // This fills the image with zeros
    image.resize(size);

    let handle = images.add(image);

    let mut hires_cam = Camera2dBundle {
        camera_2d: Camera2d::default(),
        camera: Camera {
            target: RenderTarget::Image(handle.clone()),
            ..default()
        },
        ..default()
    };

    hires_cam.projection.window_origin = WindowOrigin::BottomLeft;
    hires_cam
        .projection
        .update(size.width as f32, size.height as f32);

    commands.spawn_bundle(hires_cam).insert(HiResCam);
    commands.insert_resource(HiResHandle(handle));
}

fn initialize_accumulation_resources(commands: &mut Commands, images: &mut Assets<Image>) {
    let size = Extent3d {
        width: TEXTURE_DIM,
        height: TEXTURE_DIM,
        ..default()
    };

    info!("creating new accumulation texture");
    info!("accumulation texture size {size:?}");

    // info!("CREATING NEW ACCUMULATION TEXTURE... SLEEPING FOR 5s");

    // std::thread::sleep(Duration::from_secs(5));

    // This is the texture that will be rendered to.
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("ACCUMULATION_TEXTURE"),
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

    let handle = images.add(image);

    // sprite with the accumulation texture
    commands
        .spawn_bundle(SpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::new(size.width as f32, size.height as f32)),
                anchor: Anchor::BottomLeft,
                ..default()
            },
            texture: handle.clone(),
            transform: Transform::from_translation((0.0, 0.0, 1.0).into()),
            ..default()
        })
        .insert(MAIN_CAMERA_LAYER);

    // sprite that is 10% larger than the accumulation texture and underneath/futher from the camera
    // than the accumulation texture sprite to indicate where the texture is/outline it
    commands
        .spawn_bundle(SpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::new(size.width as f32 * 1.1, size.height as f32 * 1.1)),
                anchor: Anchor::BottomLeft,
                color: Color::rgb(1.0, 0.0, 0.0),
                ..default()
            },
            transform: Transform::from_translation(
                (-0.05 * size.width as f32, -0.05 * size.height as f32, 0.0).into(),
            ),
            ..default()
        })
        .insert(MAIN_CAMERA_LAYER);

    commands
        .spawn_bundle(Camera2dBundle {
            camera: Camera {
                priority: ACCUMULATION_CAMERA_PRIORITY,
                target: RenderTarget::Image(handle.clone()),
                viewport: Some(Viewport {
                    physical_size: UVec2::new(TILE_SIZE, TILE_SIZE),
                    ..default()
                }),
                ..default()
            },
            camera_2d: Camera2d {
                clear_color: ClearColorConfig::None,
            },
            ..default()
        })
        .insert(DOWNSCALING_PASS_LAYER)
        .insert(AccumulationCam);

    commands.insert_resource(AccumulationHandle(handle));
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // configure and spawn the main camera
    let mut camera = Camera2dBundle {
        camera: Camera {
            priority: MAIN_CAMERA_PRIORITY,
            ..default()
        },
        transform: Transform::from_translation((-4000.0, -550.0, 999.0).into()),
        ..Camera2dBundle::default()
    };
    camera.projection.window_origin = WindowOrigin::BottomLeft;
    camera.projection.scale = 8.5;

    commands
        .spawn_bundle(camera)
        .insert(MAIN_CAMERA_LAYER)
        .insert(MainCamera)
        .insert(PanCam::default());

    initialize_hi_res_resources(&mut commands, &mut images);

    initialize_accumulation_resources(&mut commands, &mut images);
}

fn camera_changed_system(
    camera_q: Query<
        (&Transform, &OrthographicProjection),
        (
            Or<(Changed<Transform>, Changed<OrthographicProjection>)>,
            With<MainCamera>,
        ),
    >,
) {
    for (t, proj) in camera_q.iter() {
        info!("Camera new transform {t:?}, scale {}", proj.scale);
    }
}

fn spawn_vlsir_open_task_sytem(mut commands: Commands, mut already_done: Local<bool>) {
    if !*already_done {
        let thread_pool = AsyncComputeTaskPool::get();

        let task: Task<Library> = thread_pool.spawn(async move {
            let plib = raw::proto::proto::open(
                "/home/colepoirier/Dropbox/rust_2020_onwards/doug/doug/libs/oscibear.proto",
            )
            .unwrap();
            ProtoImporter::import(&plib, None).unwrap()
        });
        let task = LibraryWrapper(task);
        commands.spawn().insert(task);
        *already_done = true;
    }
}

fn handle_vlsir_open_task_system(
    mut commands: Commands,
    mut lib: ResMut<VlsirLib>,
    mut vlsir_open_task_q: Query<(Entity, &mut LibraryWrapper)>,
    mut vlsir_open_lib_complete_event_writer: EventWriter<OpenVlsirLibCompleteEvent>,
) {
    for (entity, mut task) in vlsir_open_task_q.iter_mut() {
        if let Some(vlsir_lib) = future::block_on(future::poll_once(&mut **task)) {
            lib.lib = Some(vlsir_lib);
            vlsir_open_lib_complete_event_writer.send(OpenVlsirLibCompleteEvent);
            commands.entity(entity).despawn();
        }
    }
}

fn load_lib_system(
    mut vlsir_open_lib_complete_event_reader: EventReader<OpenVlsirLibCompleteEvent>,
    vlsir_lib: Res<VlsirLib>,
    mut layer_colors: ResMut<LayerColors>,
    mut layers: ResMut<Layers>,
    mut lib_layers: ResMut<LibLayers>,
    render_device: Res<RenderDevice>,
    mut tilemap: ResMut<Tilemap>,
    mut tile_index_iter: ResMut<TileIndexIter>,
    mut flattened_elems_res: ResMut<FlattenedElems>,
    mut min_offset_res: ResMut<TilemapLowerLeft>,
    mut ev: EventWriter<DrawTileEvent>,
) {
    let texture_dim = render_device.limits().max_texture_dimension_2d;

    for _ in vlsir_open_lib_complete_event_reader.iter() {
        let lib = vlsir_lib.lib.as_ref().unwrap();

        {
            let lib_layers = &lib.layers.read().unwrap().slots;

            for raw::Layer {
                layernum, name: _, ..
            } in lib_layers.values()
            {
                let num = *layernum as u8;
                if let Some(_) = layers.insert(num, layer_colors.get_color()) {
                    panic!(
                        "Library layers corrupted multiple definitions for layer number {}",
                        num
                    );
                }
            }
        }

        *lib_layers = LibLayers(lib.layers.read().unwrap().clone());

        let cell_ptr = lib.cells.iter().last().unwrap();

        let cell = cell_ptr.read().unwrap();

        let flattened_elems = cell.layout.as_ref().unwrap().flatten().unwrap();

        info!("num elems including instances: {}", flattened_elems.len());

        let mut bbox = BoundBox::empty();
        for elem in flattened_elems.iter() {
            bbox = elem.inner.union(&bbox);
        }

        assert!(!bbox.is_empty(), "bbox must be valid!");
        *min_offset_res = TilemapLowerLeft {
            x: bbox.p0.x as i64,
            y: bbox.p0.y as i64,
        };

        info!("flattened bbox is {bbox:?}");

        let dx = (bbox.p1.x - bbox.p0.x) as u32;
        let dy = (bbox.p1.y - bbox.p0.y) as u32;

        let num_x_tiles = (dx as f32 / texture_dim as f32).ceil() as u32;
        let num_y_tiles = (dy as f32 / texture_dim as f32).ceil() as u32;

        let mut x = bbox.p0.x as i64;
        let mut y = bbox.p0.y as i64;

        let mut tilemap_shift = raw::Point::default();

        if x < 0 {
            tilemap_shift.x = -x as isize;
        }

        if y < 0 {
            tilemap_shift.y = -y as isize;
        }

        for iy in 0..num_y_tiles {
            let ymin = y;
            y += texture_dim as i64;
            let ymax = y;
            for ix in 0..num_x_tiles {
                let xmin = x;
                x += texture_dim as i64;
                let xmax = x;

                let extents = GeoRect::new((xmin, ymin), (xmax, ymax));

                tilemap.insert(
                    (ix, iy),
                    Tile {
                        extents,
                        shapes: vec![],
                    },
                );
            }

            x = bbox.p0.x as i64;
        }

        let mut shape_count = 0;

        info!("{bbox:?}, dx: {dx}, dy: {dy}, tiles: [{num_x_tiles}, {num_y_tiles}]");

        let t = std::time::Instant::now();

        import_cell_shapes(
            tilemap_shift,
            texture_dim,
            &mut tilemap,
            &flattened_elems,
            &mut shape_count,
        );

        info!("DONE {shape_count} shapes in {:?}!", t.elapsed());

        *flattened_elems_res = FlattenedElems(flattened_elems);

        tilemap_stats_and_debug(&tilemap);

        let (x, y) = get_grid_shape(&tilemap);

        let mut index_iter = (0..y).cartesian_product(0..x);

        let (y, x) = index_iter.next().unwrap();

        *tile_index_iter = TileIndexIter(Some(index_iter));

        ev.send(DrawTileEvent((x, y)));
    }
}

fn iter_tile_index_system(
    mut tile_index_iter: ResMut<TileIndexIter>,
    mut draw_tile_ev: EventWriter<DrawTileEvent>,
    mut rendering_complete_ev: EventReader<RenderingCompleteEvent>,
) {
    for _ in rendering_complete_ev.iter() {
        if tile_index_iter.is_some() {
            if let Some((y, x)) = (**tile_index_iter).as_mut().unwrap().next() {
                // if (x < 40) && (y == 170) {
                let event = DrawTileEvent((x, y));
                info!("Sending {event:?}");
                draw_tile_ev.send(event);
                // std::thread::sleep(std::time::Duration::from_millis(200));
                // }
            }
        }
    }
}

pub fn import_cell_shapes(
    tilemap_shift: raw::Point,
    texture_dim: u32,
    tilemap: &mut Tilemap,
    elems: &Vec<raw::Element>,
    shape_count: &mut u64,
) {
    for (idx, raw::Element { inner, .. }) in elems.iter().enumerate() {
        let mut bbox = inner.bbox();

        if !bbox.is_empty() {
            bbox.p0 = bbox.p0.shift(&tilemap_shift);
            bbox.p1 = bbox.p1.shift(&tilemap_shift);

            let BoundBox { p0, p1 } = bbox;

            let min_tile_x = p0.x as u32 / texture_dim;
            let min_tile_y = p0.y as u32 / texture_dim;
            let max_tile_x = p1.x as u32 / texture_dim;
            let max_tile_y = p1.y as u32 / texture_dim;

            let geo_shape = match inner {
                raw::Shape::Rect(r) => {
                    let raw::Rect { p0, p1 } = r;

                    let xmin = p0.x as i64;
                    let ymin = p0.y as i64;
                    let xmax = p1.x as i64;
                    let ymax = p1.y as i64;

                    let rect = GeoRect::new((xmin, ymin), (xmax, ymax));

                    let geo_shape = GeoShapeEnum::Rect(rect);

                    geo_shape
                }
                raw::Shape::Polygon(p) => {
                    let poly = GeoPolygon::new(
                        p.points.iter().map(|p| (p.x as i64, p.y as i64)).collect(),
                        vec![],
                    );

                    GeoShapeEnum::Polygon(poly)
                }
                raw::Shape::Path(p) => GeoShapeEnum::Polygon(make_path_into_polygon(p)),
            };

            for x in min_tile_x..(max_tile_x + 1) {
                for y in min_tile_y..(max_tile_y + 1) {
                    let Tile { extents, shapes } = tilemap.get_mut(&(x, y)).unwrap();

                    let extents = &*extents;

                    match &geo_shape {
                        GeoShapeEnum::Rect(r) => {
                            if r.intersects(extents) {
                                shapes.push(idx);
                            }
                        }
                        GeoShapeEnum::Polygon(p) => {
                            if p.intersects(extents) {
                                shapes.push(idx);
                            }
                        }
                    }
                }
            }
        }

        *shape_count += 1;

        if *shape_count % 1_000_000 == 0 {
            info!("shapes processed: {shape_count}");
        }
    }
}
