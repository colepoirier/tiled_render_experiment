use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    render::{
        camera::{Projection, WindowOrigin},
        renderer::RenderDevice,
    },
    tasks::{AsyncComputeTaskPool, Task},
    utils::hashbrown::HashMap,
};

use std::{f32::consts::E, ops::Range};

use csv::Writer;
use futures_lite::future;
use geo::Intersects;
use itertools::Itertools;
use layout21::raw::{self, proto::ProtoImporter, BoundBox, BoundBoxTrait, Library};

pub mod tiled_renderer;

use tiled_renderer::{TiledRendererPlugin, MAIN_CAMERA_LAYER};

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

#[derive(Debug, Default, Deref, DerefMut)]
pub struct TileMap(HashMap<(u32, u32), Tile>);

#[derive(Debug, Default)]
pub struct TileMapLowerLeft {
    x: i64,
    y: i64,
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct FlattenedElems(Vec<raw::Element>);

#[derive(Debug, Default, Clone, Copy)]
pub struct OpenVlsirLibCompleteEvent;

#[derive(Debug, Default)]
struct VlsirLib {
    pub lib: Option<Library>,
}

#[derive(Debug, Component, Deref, DerefMut)]
struct LibraryWrapper(Task<Library>);

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct Layers(HashMap<u8, Color>);

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct LibLayers(raw::Layers);

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
        .init_resource::<VlsirLib>()
        .init_resource::<FlattenedElems>()
        .init_resource::<TileMap>()
        .init_resource::<TileMapLowerLeft>()
        .init_resource::<Layers>()
        .init_resource::<LibLayers>()
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
    mut tilemap: ResMut<TileMap>,
    mut tile_index_iter: ResMut<TileIndexIter>,
    mut flattened_elems_res: ResMut<FlattenedElems>,
    mut min_offset_res: ResMut<TileMapLowerLeft>,
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
        *min_offset_res = TileMapLowerLeft {
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

        stats(&tilemap);

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

pub fn import_cell_shapes(
    tilemap_shift: raw::Point,
    texture_dim: u32,
    tilemap: &mut TileMap,
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
                raw::Shape::Path(p) => {
                    let num_points = p.points.len();
                    let mut forward_poly_points = Vec::with_capacity(num_points);
                    let mut backward_poly_points = Vec::with_capacity(num_points);
                    assert_eq!(
                        p.width % 2,
                        0,
                        "width must be even for our code's assumptions to hold!"
                    );
                    let half_width = (p.width / 2) as isize; // assuming that widths are even!

                    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
                    enum RectMove {
                        Left,
                        Right,
                        Up,
                        Down,
                    };

                    fn calculate_move(p0: raw::Point, p1: raw::Point) -> RectMove {
                        if p0.x == p1.x {
                            if p0.y == p1.y {
                                panic!("Cannot handle distinct points in a path with the same coordinate: p0 {p0:?} and p1 {p1:?}");
                            } else if p0.y < p1.y {
                                RectMove::Up
                            } else {
                                RectMove::Down
                            }
                        } else if p0.y == p1.y {
                            if p0.x == p1.x {
                                panic!("Cannot handle distinct points in a path with the same coordinate: p0 {p0:?} and p1 {p1:?}");
                            } else if p0.x < p1.x {
                                RectMove::Right
                            } else {
                                RectMove::Left
                            }
                        } else {
                            panic!(
                                "rectilinear moves expected, but found: p0 {p0:?} and p1 {p1:?}"
                            );
                        }
                    }

                    fn shift_pure_right(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        forward_poly_points.push(raw::Point {
                            x: p0.x,
                            y: p0.y - half_width,
                        });
                        backward_poly_points.push(raw::Point {
                            x: p0.x,
                            y: p0.y + half_width,
                        });
                    }

                    fn shift_pure_left(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        forward_poly_points.push(raw::Point {
                            x: p0.x,
                            y: p0.y + half_width,
                        });
                        backward_poly_points.push(raw::Point {
                            x: p0.x,
                            y: p0.y - half_width,
                        });
                    }

                    fn shift_pure_up(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        forward_poly_points.push(raw::Point {
                            x: p0.x + half_width,
                            y: p0.y,
                        });
                        backward_poly_points.push(raw::Point {
                            x: p0.x - half_width,
                            y: p0.y,
                        });
                    }

                    fn shift_pure_down(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        forward_poly_points.push(raw::Point {
                            x: p0.x - half_width,
                            y: p0.y,
                        });
                        backward_poly_points.push(raw::Point {
                            x: p0.x + half_width,
                            y: p0.y,
                        });
                    }

                    fn shift_right_up(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        forward_poly_points.push(raw::Point {
                            x: p0.x + half_width,
                            y: p0.y - half_width,
                        });
                        backward_poly_points.push(raw::Point {
                            x: p0.x - half_width,
                            y: p0.y + half_width,
                        });
                    }

                    fn shift_left_down(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        shift_right_up(backward_poly_points, forward_poly_points, p0, half_width);
                    }

                    fn shift_right_down(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        forward_poly_points.push(raw::Point {
                            x: p0.x - half_width,
                            y: p0.y - half_width,
                        });
                        backward_poly_points.push(raw::Point {
                            x: p0.x + half_width,
                            y: p0.y + half_width,
                        });
                    }

                    fn shift_left_up(
                        forward_poly_points: &mut Vec<raw::Point>,
                        backward_poly_points: &mut Vec<raw::Point>,
                        p0: raw::Point,
                        half_width: isize,
                    ) {
                        shift_right_down(backward_poly_points, forward_poly_points, p0, half_width);
                    }

                    assert!(
                        num_points > 1,
                        "Expected number of points in path to be > 1"
                    );
                    let start_move = calculate_move(p.points[0], p.points[1]);

                    match start_move {
                        RectMove::Right => shift_pure_right(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[0],
                            half_width,
                        ),
                        RectMove::Left => shift_pure_left(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[0],
                            half_width,
                        ),
                        RectMove::Up => shift_pure_up(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[0],
                            half_width,
                        ),
                        RectMove::Down => shift_pure_down(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[0],
                            half_width,
                        ),
                    }

                    let mut last_move = start_move;

                    for ix in 1..(num_points - 1) {
                        let p0 = p.points[ix];
                        let p1 = p.points[ix + 1];
                        let next_move = calculate_move(p0, p1);
                        match (last_move, next_move) {
                            (RectMove::Right, RectMove::Right) => shift_pure_right(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (RectMove::Left, RectMove::Left) => shift_pure_left(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (RectMove::Up, RectMove::Up) => shift_pure_up(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (RectMove::Down, RectMove::Down) => shift_pure_down(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (RectMove::Right, RectMove::Down) | (RectMove::Down, RectMove::Right) => shift_right_down(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (RectMove::Right, RectMove::Up) | (RectMove::Up, RectMove::Right) => shift_right_up(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (RectMove::Left, RectMove::Up) | (RectMove::Up, RectMove::Left) => shift_left_up(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (RectMove::Left, RectMove::Down) | (RectMove::Down, RectMove::Left) => shift_left_down(&mut forward_poly_points, &mut backward_poly_points, p.points[0], half_width),
                            (_, _) => panic!("Received opposing last/next moves! last: {last_move:?}, next: {next_move:?}"),
                        }
                        last_move = next_move;
                    }

                    let end_move =
                        calculate_move(p.points[num_points - 2], p.points[num_points - 1]);
                    match end_move {
                        RectMove::Right => shift_pure_right(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[num_points - 1],
                            half_width,
                        ),
                        RectMove::Left => shift_pure_left(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[num_points - 1],
                            half_width,
                        ),
                        RectMove::Up => shift_pure_up(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[num_points - 1],
                            half_width,
                        ),
                        RectMove::Down => shift_pure_down(
                            &mut forward_poly_points,
                            &mut backward_poly_points,
                            p.points[num_points - 1],
                            half_width,
                        ),
                    }

                    let poly = GeoPolygon::new(
                        forward_poly_points
                            .into_iter()
                            .chain(backward_poly_points.into_iter().rev())
                            .map(|p| (p.x as i64, p.y as i64))
                            .collect(),
                        vec![],
                    );

                    GeoShapeEnum::Polygon(poly)
                }
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
