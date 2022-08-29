use bevy::{
    prelude::*,
    render::{camera::WindowOrigin, renderer::RenderDevice},
};

use bevy_pancam::{PanCam, PanCamPlugin};

use geo::Intersects;
use itertools::Itertools;

use std::{fs::File, io::Write};

mod tiled_renderer;
use tiled_renderer::TiledRendererPlugin;

mod types;
use types::{
    DrawTileEvent, FlattenedElems, GeoRect, MainCamera, Point, Rect, RenderingCompleteEvent, Tile,
    TileIndexIter, Tilemap, TilemapLowerLeft, MAIN_CAMERA_LAYER, MAIN_CAMERA_PRIORITY,
};

mod utils;
use utils::{generate_random_elements, get_grid_shape, tilemap_stats_and_debug};

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
        .init_resource::<FlattenedElems>()
        .init_resource::<Tilemap>()
        .init_resource::<TilemapLowerLeft>()
        .init_resource::<TileIndexIter>()
        .add_event::<DrawTileEvent>()
        .add_event::<TileIndexIter>()
        .add_event::<RenderingCompleteEvent>()
        .init_resource::<Msaa>()
        .add_startup_system(setup)
        .add_system(create_tilemap_system)
        .add_system(iter_tile_index_system)
        .add_system(camera_changed_system)
        // .add_system(list_cameras_system)
        .run();
}

fn setup(mut commands: Commands) {
    let mut camera = Camera2dBundle {
        camera: Camera {
            priority: MAIN_CAMERA_PRIORITY,
            ..default()
        },
        transform: Transform::from_translation((-9284.8, -100.0, 999.0).into()),
        ..Camera2dBundle::default()
    };
    camera.projection.window_origin = WindowOrigin::BottomLeft;
    camera.projection.scale = 13.0;

    commands
        .spawn_bundle(camera)
        .insert(MAIN_CAMERA_LAYER)
        .insert(MainCamera)
        .insert(PanCam::default());
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

fn iter_tile_index_system(
    mut tile_index_iter: ResMut<TileIndexIter>,
    mut draw_tile_ev: EventWriter<DrawTileEvent>,
    mut rendering_complete_ev: EventReader<RenderingCompleteEvent>,
) {
    for _ in rendering_complete_ev.iter() {
        if tile_index_iter.is_some() {
            if let Some((y, x)) = (**tile_index_iter).as_mut().unwrap().next() {
                let event = DrawTileEvent((x, y));
                info!("Sending {event:?}");
                draw_tile_ev.send(event);
            }
        }
    }
}

fn create_tilemap_system(
    render_device: Res<RenderDevice>,
    mut tilemap: ResMut<Tilemap>,
    mut tile_index_iter: ResMut<TileIndexIter>,
    mut flattened_elems_res: ResMut<FlattenedElems>,
    mut min_offset_res: ResMut<TilemapLowerLeft>,
    mut ev: EventWriter<DrawTileEvent>,
    mut has_run: Local<bool>,
) {
    if !*has_run {
        let texture_dim = render_device.limits().max_texture_dimension_2d;

        let num_elements = 1_000;
        let min_p = Point { x: 0, y: 0 };
        let max_p = Point {
            x: texture_dim as i32 * 64,
            y: texture_dim as i32 * 64,
        };
        let mut flattened_elems = generate_random_elements(num_elements, min_p, max_p);

        flattened_elems.sort_by(|a, b| a.p1.x.cmp(&b.p1.x));

        let mut f = File::create("dbg_random_shapes.txt").unwrap();

        for r in flattened_elems.iter() {
            f.write(format!("{r:?}\n").as_bytes()).unwrap();
        }

        info!("num elems including instances: {}", flattened_elems.len());

        let mut bbox = (
            Point {
                x: i32::MAX,
                y: i32::MAX,
            },
            Point {
                x: i32::MIN,
                y: i32::MIN,
            },
        );

        for elem in flattened_elems.iter() {
            bbox.0.x = bbox.0.x.min(elem.p0.x).min(elem.p1.x);
            bbox.0.y = bbox.0.y.min(elem.p0.y).min(elem.p1.y);
            bbox.1.x = bbox.1.x.max(elem.p0.x).max(elem.p1.x);
            bbox.1.y = bbox.1.y.max(elem.p0.y).max(elem.p1.y);
        }

        *min_offset_res = TilemapLowerLeft {
            x: bbox.0.x as i64,
            y: bbox.0.y as i64,
        };

        info!("flattened bbox is {bbox:?}");

        let dx = bbox.1.x - bbox.0.x;
        let dy = bbox.1.y - bbox.0.y;

        info!("(dx {dx}, dy {dy})");

        let num_x_tiles = ((dx as f32 / texture_dim as f32) + 0.00001).ceil() as u32;
        let num_y_tiles = ((dy as f32 / texture_dim as f32) + 0.00001).ceil() as u32;

        info!("num_x_tiles: {num_x_tiles}, num_y_tiles: {num_y_tiles}");

        // panic!();

        let mut x = bbox.0.x as i64;
        let mut y = bbox.0.y as i64;

        let tilemap_shift = Point {
            x: -x as i32,
            y: -y as i32,
        };

        info!("{tilemap_shift:?}");

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

            x = bbox.0.x as i64;
        }

        let mut shape_count = 0;

        info!("{bbox:?}, dx: {dx}, dy: {dy}, tiles: [{num_x_tiles}, {num_y_tiles}]");

        let t = std::time::Instant::now();

        for (idx, rect) in flattened_elems.iter().enumerate() {
            let Rect { p0, p1, .. } = rect.shift(&tilemap_shift);

            let min_tile_x = p0.x as u32 / texture_dim;
            let min_tile_y = p0.y as u32 / texture_dim;
            let max_tile_x = p1.x as u32 / texture_dim;
            let max_tile_y = p1.y as u32 / texture_dim;

            // info!("min_tile_x: {min_tile_x}");
            // info!("max_tile_x: {max_tile_x}");
            // info!("min_tile_y: {min_tile_y}");
            // info!("max_tile_y: {max_tile_y}");

            let rect = GeoRect::new((p0.x as i64, p0.y as i64), (p1.x as i64, p1.y as i64));

            for x in min_tile_x..=max_tile_x {
                for y in min_tile_y..=max_tile_y {
                    info!("x {x}, y {y}");
                    let Tile { extents, shapes } = tilemap.get_mut(&(x, y)).unwrap();

                    let extents = &*extents;

                    if rect.intersects(extents) {
                        shapes.push(idx);
                    }
                }
            }

            shape_count += 1;

            if shape_count % 1_000_000 == 0 {
                info!("shapes processed: {shape_count}");
            }
        }

        info!("DONE {shape_count} shapes in {:?}!", t.elapsed());

        *flattened_elems_res = FlattenedElems(flattened_elems);

        tilemap_stats_and_debug(&tilemap);

        let (x, y) = get_grid_shape(&tilemap);

        let mut index_iter = (0..y).cartesian_product(0..x);

        let (y, x) = index_iter.next().unwrap();

        *tile_index_iter = TileIndexIter(Some(index_iter));

        ev.send(DrawTileEvent((x, y)));

        *has_run = true;
    }
}
