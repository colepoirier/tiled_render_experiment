use bbox::BoundingBox;
use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    render::{camera::WindowOrigin, renderer::RenderDevice},
    utils::hashbrown::HashMap,
};

use rkyv::{archived_root, Deserialize, Infallible};

use memmap2::Mmap;
use std::ops::Range;
use std::{fs::File, time::Instant};

use csv::Writer;
use geo::Intersects;
use itertools::Itertools;

mod bbox;
mod shapes;
mod tiled_renderer;

use bbox::CalculateBoundingBox;

use shapes::{ArchivedRect, ArchivedShape, Shapes};

use tiled_renderer::{TiledRendererPlugin, MAIN_CAMERA_LAYER};

use crate::shapes::Point;

pub type GeoRect = geo::Rect<i64>;
pub type GeoPolygon = geo::Polygon<i64>;

#[derive(Debug, Clone)]
pub enum GeoShapeEnum {
    Rect(GeoRect),
    Polygon(GeoPolygon),
}

#[derive(Debug)]
pub struct Tile {
    pub extents: GeoRect,
    pub shapes: Vec<usize>,
}

#[derive(Debug, Deref, DerefMut)]
pub struct TileMap(HashMap<(u32, u32), Tile>);

#[derive(Debug, Default)]
pub struct TileMapLowerLeft {
    x: i32,
    y: i32,
}

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct Layers(HashMap<u8, Color>);

#[derive(Debug)]
pub struct LayerColors {
    colors: std::iter::Cycle<std::vec::IntoIter<Color>>,
}

impl Default for LayerColors {
    fn default() -> Self {
        Self {
            // IBM Design Language Color Library - Color blind safe palette
            // https://web.archive.org/web/20220304221053/https://ibm-design-language.eu-de.mybluemix.net/design/language/resources/color-library/
            // Color Names: Ultramarine 40, Indigo 50, Magenta 50 , Orange 40, Gold 20
            // It just looks pretty
            colors: vec!["648FFF", "785EF0", "DC267F", "FE6100", "FFB000"]
                .into_iter()
                .map(|c| Color::hex(c).unwrap())
                .collect::<Vec<Color>>()
                .into_iter()
                .cycle(),
        }
    }
}

impl LayerColors {
    pub fn get_color(&mut self) -> Color {
        self.colors.next().unwrap()
    }
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct TileIndexIter(Option<itertools::Product<Range<u32>, Range<u32>>>);

#[derive(Debug, Default, Clone, Copy)]
struct DrawTileEvent((u32, u32));

#[derive(Debug, Default)]
struct RenderingCompleteEvent;

#[derive(Debug, Component)]
pub struct MainCamera;

#[derive(Debug, Deref)]
pub struct MemMappedFile(Mmap);

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            width: 1920.0 / 4.0,
            height: 1080.0 / 4.0,
            present_mode: bevy::window::PresentMode::Immediate,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(PanCamPlugin)
        .add_plugin(TiledRendererPlugin)
        .init_resource::<LayerColors>()
        .insert_resource(TileMap(HashMap::<(u32, u32), Tile>::new()))
        .insert_resource({
            let f = File::open("test64x64.rkyv").unwrap();

            let mmap = unsafe { Mmap::map(&f).unwrap() };
            MemMappedFile(mmap)
        })
        .init_resource::<TileMapLowerLeft>()
        .init_resource::<Layers>()
        .init_resource::<TileIndexIter>()
        .add_event::<DrawTileEvent>()
        .add_event::<TileIndexIter>()
        .add_event::<RenderingCompleteEvent>()
        .init_resource::<Msaa>()
        .add_startup_system(setup)
        .add_system(load_lib_system)
        .add_system(iter_tile_index_system)
        .add_system(camera_changed_system)
        .run();
}

fn setup(mut commands: Commands) {
    let mut camera = Camera2dBundle {
        camera: Camera {
            // renders after the cameras with lower values for priority
            priority: 3,
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

fn load_lib_system(
    render_device: Res<RenderDevice>,
    mut tilemap: ResMut<TileMap>,
    mut tile_index_iter: ResMut<TileIndexIter>,
    mut min_offset_res: ResMut<TileMapLowerLeft>,
    mut ev: EventWriter<DrawTileEvent>,
    mut already_ran: Local<bool>,
    mmap: Res<MemMappedFile>,
) {
    if !*already_ran {
        let extents = render_device.limits().max_texture_dimension_2d;

        let archived_shapes = unsafe { archived_root::<Shapes>(&mmap) };

        let num_shapes = archived_shapes.shapes.len();

        info!("num elems including instances: {}", num_shapes);

        let bbox: BoundingBox = archived_shapes.bbox.deserialize(&mut Infallible).unwrap();

        info!("{bbox:?}");

        let boundary_min_x = bbox.min().x;
        let boundary_min_y = bbox.min().y;
        let boundary_max_x = bbox.max().x;
        let boundary_max_y = bbox.max().y;

        let shapes = &archived_shapes.shapes;

        *min_offset_res = TileMapLowerLeft {
            x: bbox.min().x,
            y: bbox.min().y,
        };

        let tilemap_shift = Point {
            x: -bbox.min().x,
            y: -bbox.min().y,
        };

        let tilemap_max_coords = Point {
            x: bbox.max().x - 1,
            y: bbox.max().y - 1,
        }
        .shift(&tilemap_shift);

        info!("{tilemap_max_coords:?}");

        info!("tilemap_shift {tilemap_shift:?}");

        let dx = u32::try_from(bbox.max().x - bbox.min().x).unwrap();
        let dy = u32::try_from(bbox.max().y - bbox.min().y).unwrap();

        let num_x_tiles = (dx as f32 / extents as f32).ceil() as u32;
        let num_y_tiles = (dy as f32 / extents as f32).ceil() as u32;

        info!("dx {dx} dy {dy} num_x_tiles {num_x_tiles} num_y_tiles {num_y_tiles}");

        let mut x = bbox.min().x;
        let mut y = bbox.min().y;

        for iy in 0..num_y_tiles {
            let ymin = y;
            y += extents as i32;
            let ymax = y;
            for ix in 0..num_x_tiles {
                let xmin = x;
                x += extents as i32;
                let xmax = x;

                let extents = GeoRect::new((xmin as i64, ymin as i64), (xmax as i64, ymax as i64));

                tilemap.insert(
                    (ix, iy),
                    Tile {
                        extents,
                        shapes: vec![],
                    },
                );
            }

            x = bbox.min().x;
        }

        let mut shape_count = 0;

        let t = Instant::now();

        for (idx, s) in shapes.iter().enumerate() {
            // info!("{s:?}");

            let bbox = s.bbox();

            // info!("pre-shift: {bbox:?}");

            let bbox = s.bbox().shift(&tilemap_shift);

            // info!("post-shift: {bbox:?}");

            // let min_tile_x = (bbox.min().x / extents as i32).min(0) as u32;
            // let max_tile_x = (bbox.max().x / extents as i32).max(num_x_tiles as i32) as u32;
            // let min_tile_y = (bbox.min().y / extents as i32).min(0) as u32;
            // let max_tile_y = (bbox.max().y / extents as i32).max(num_y_tiles as i32) as u32;

            // info!("boundary_min_x: {boundary_min_x}, boundary_max_x: {boundary_max_x}, boundary_min_y: {boundary_min_y}, boundary_max_y: {boundary_max_y}");

            let min_tile_x = bbox.min().x.clamp(0, tilemap_max_coords.x) as u32 / extents;
            let min_tile_y = bbox.min().y.clamp(0, tilemap_max_coords.y) as u32 / extents;
            let max_tile_x = bbox.max().x.clamp(0, tilemap_max_coords.x) as u32 / extents;
            let max_tile_y = bbox.max().y.clamp(0, tilemap_max_coords.y) as u32 / extents;

            // info!("min_tile_x: {min_tile_x}, max_tile_x: {max_tile_x}, min_tile_y: {min_tile_y}, max_tile_y: {max_tile_y}");

            // panic!();

            let geo_shape = match s {
                ArchivedShape::Rect(r) => {
                    // info!("{r:?}");
                    let ArchivedRect { p0, p1, .. } = r;

                    let xmin = p0.x as i64;
                    let ymin = p0.y as i64;
                    let xmax = p1.x as i64;
                    let ymax = p1.y as i64;

                    let rect = GeoRect::new((xmin, ymin), (xmax, ymax));

                    let geo_shape = GeoShapeEnum::Rect(rect);

                    geo_shape
                }
                ArchivedShape::Poly(p) => {
                    // info!("{p:?}");
                    let poly = GeoPolygon::new(
                        p.points.iter().map(|p| (p.x as i64, p.y as i64)).collect(),
                        vec![],
                    );

                    GeoShapeEnum::Polygon(poly)
                }
                ArchivedShape::Path(p) => {
                    // info!("{p:?}");

                    let p = p.as_poly();

                    let poly = GeoPolygon::new(
                        p.into_iter().map(|p| (p.x as i64, p.y as i64)).collect(),
                        vec![],
                    );

                    GeoShapeEnum::Polygon(poly)
                }
            };

            // info!("min_tile_x: {min_tile_x}, max_tile_x: {max_tile_x}, min_tile_y: {min_tile_y}, max_tile_y: {max_tile_y}");
            // info!("{geo_shape:?}");
            // info!("{:?}", tilemap.get(&(min_tile_x, min_tile_y)));

            // panic!();

            // std::thread::sleep(std::time::Duration::from_secs(5));

            for x in min_tile_x..(max_tile_x + 1) {
                for y in min_tile_y..(max_tile_y + 1) {
                    // info!("tile x {x}, y {y}");
                    let Tile { extents, shapes } = tilemap.get_mut(&(x, y)).unwrap();

                    let extents = &*extents;

                    // info!("extents {extents:?}");

                    match &geo_shape {
                        GeoShapeEnum::Rect(r) => {
                            // info!("extents {extents:?}");
                            // info!("rect:   {r:?}");
                            if r.intersects(extents) {
                                shapes.push(idx);
                                // info!("pushing idx [{idx}] to tile {extents:?}");
                            }
                        }
                        GeoShapeEnum::Polygon(p) => {
                            if p.intersects(extents) {
                                shapes.push(idx);
                                // info!("pushing idx [{idx}] to tile {extents:?}");
                            }
                        }
                    }
                }
            }

            // info!("shape_count: {shape_count}");

            shape_count += 1;

            if shape_count % 1_000_000 == 0 {
                info!("shapes processed: {shape_count}");
            }
        }

        info!("DONE {shape_count} shapes in {:?}!", t.elapsed());

        stats(&tilemap);

        let (x, y) = get_grid_shape(&tilemap);

        let mut index_iter = (0..y).cartesian_product(0..x);

        let (y, x) = index_iter.next().unwrap();

        *tile_index_iter = TileIndexIter(Some(index_iter));

        // panic!();

        ev.send(DrawTileEvent((x, y)));

        *already_ran = true;
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

    // for _ in rendering_complete_ev.iter() {
    //     if tile_index_iter.is_some() {
    //         let idx_iter = (**tile_index_iter).as_mut().unwrap();

    //         loop {
    //             if let Some((y, x)) = idx_iter.next() {
    //                 if tilemap.get(&(x, y)).unwrap().shapes.len() != 0 {
    //                     let event = DrawTileEvent((x, y));
    //                     info!("Sending {event:?}");
    //                     draw_tile_ev.send(event);
    //                     break;
    //                 } else {
    //                     info!("Tile ({x}, {y}) is empty, not sending DrawTileEvent");
    //                 }
    //             } else {
    //                 break;
    //             }
    //         }
    //     }
    // }
}

fn get_grid_shape(grid: &TileMap) -> (u32, u32) {
    let (mut x_min, mut x_max, mut y_min, mut y_max) = (0, 0, 0, 0);
    for &(x, y) in grid.keys() {
        if x < x_min {
            x_min = x;
        } else if x > x_max {
            x_max = x;
        }

        if y < y_min {
            y_min = y;
        } else if y > y_max {
            y_max = y;
        }
    }

    (x_max - x_min + 1, y_max - y_min + 1)
}

fn stats(grid: &TileMap) {
    let mut counts: Vec<usize> = vec![];

    for v in grid.values() {
        counts.push(v.shapes.len());
    }

    let num_occupied_bins = counts.iter().filter(|x| **x != 0).collect::<Vec<_>>().len();
    let min = counts.iter().min().unwrap();
    let max = counts.iter().max().unwrap();
    let num_rects_incl_duplicates = counts.iter().sum::<usize>();
    // average shapes per occupied bin
    let avg_spob = counts.iter().sum::<usize>() / counts.len();

    let grid_size = get_grid_shape(&grid);

    let mut wtr = Writer::from_path("table_heatmap_data.csv").unwrap();

    for iy in 0..grid_size.1 {
        let mut row = vec![];
        for ix in 0..grid_size.0 {
            let count = grid.get(&(ix, iy)).unwrap().shapes.len();
            row.push(count.to_string());
        }

        wtr.write_record(&row[..]).unwrap();
    }

    wtr.flush().unwrap();

    let num_bins = (grid_size.0 * grid_size.1) as usize;
    let grid_occupancy = num_occupied_bins as f32 / num_bins as f32;
    info!(
        "grid_size: {grid_size:?}, num_bins: {num_bins}, num_occupied_bins: {num_occupied_bins}, num_rects_incl_duplicates: {num_rects_incl_duplicates}"
    );
    info!("grid_occupancy: {grid_occupancy}");
    info!(
        "avg shapes per occupied bin: {}",
        num_rects_incl_duplicates as f32 / num_occupied_bins as f32
    );
    info!("min: {min}, max: {max}, avg_spob: {avg_spob}");
}

#[derive(Default)]
pub struct PanCamPlugin;

impl Plugin for PanCamPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(camera_movement).add_system(camera_zoom);
    }
}

// Zoom doesn't work on bevy 0.5 due to: https://github.com/bevyengine/bevy/pull/2015
fn camera_zoom(
    mut query: Query<(&PanCam, &mut OrthographicProjection)>,
    mut scroll_events: EventReader<MouseWheel>,
) {
    let pixels_per_line = 100.; // Maybe make configurable?
    let scroll = scroll_events
        .iter()
        .map(|ev| match ev.unit {
            MouseScrollUnit::Pixel => ev.y,
            MouseScrollUnit::Line => ev.y * pixels_per_line,
        })
        .sum::<f32>();

    if scroll == 0. {
        return;
    }

    for (cam, mut projection) in query.iter_mut() {
        if cam.enabled {
            projection.scale = (projection.scale * (1. + -scroll * 0.001)).max(0.00001);
        }
    }
}

fn camera_movement(
    mut windows: ResMut<Windows>,
    mouse_buttons: Res<Input<MouseButton>>,
    mut query: Query<(&PanCam, &mut Transform, &OrthographicProjection)>,
    mut last_pos: Local<Option<Vec2>>,
) {
    let window = windows.get_primary_mut().unwrap();

    // Use position instead of MouseMotion, otherwise we don't get acceleration
    // movement
    let current_pos = match window.cursor_position() {
        Some(current_pos) => current_pos,
        None => return,
    };
    let delta = current_pos - last_pos.unwrap_or(current_pos);

    for (cam, mut transform, projection) in query.iter_mut() {
        if cam.enabled
            && cam
                .grab_buttons
                .iter()
                .any(|btn| mouse_buttons.pressed(*btn))
        {
            let scaling = Vec2::new(
                window.width() / (projection.right - projection.left),
                window.height() / (projection.top - projection.bottom),
            ) * projection.scale;

            transform.translation -= (delta * scaling).extend(0.);
        }
    }
    *last_pos = Some(current_pos);
}

#[derive(Component)]
pub struct PanCam {
    pub grab_buttons: Vec<MouseButton>,
    pub enabled: bool,
}

impl Default for PanCam {
    fn default() -> Self {
        Self {
            grab_buttons: vec![MouseButton::Left, MouseButton::Right, MouseButton::Middle],
            enabled: true,
        }
    }
}
