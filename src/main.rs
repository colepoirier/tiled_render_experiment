use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::{CameraProjection, RenderTarget, WindowOrigin},
        mesh::{Indices, PrimitiveTopology},
        render_resource::{
            AsBindGroup, Extent3d, ShaderRef, TextureDescriptor, TextureDimension, TextureFormat,
            TextureUsages,
        },
        renderer::{RenderDevice, RenderQueue},
        view::RenderLayers,
    },
    sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle},
    tasks::{AsyncComputeTaskPool, Task},
    utils::hashbrown::HashMap,
};

use bevy_prototype_lyon::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use csv::Writer;
use futures_lite::future;
use geo::Intersects;
use layout21::raw::{self, proto::ProtoImporter, BoundBox, BoundBoxTrait, Library};

pub type GeoRect = geo::Rect<i64>;
pub type GeoPolygon = geo::Polygon<i64>;

pub const ALPHA: f32 = 0.1;
pub const WIDTH: f32 = 10.0;

pub const MAIN_CAMERA_LAYER: RenderLayers = RenderLayers::layer(2);

// #[derive(Debug, Deref, DerefMut)]
// pub struct Rect(GeoRect);

#[derive(Debug, Clone)]
pub enum GeoShapeEnum {
    Rect(GeoRect),
    Polygon(GeoPolygon),
}

// pub struct ShapesToInsert {
//     geo_shape: GeoShapeEnum,
//     lyon_shape: ShapeBundle,
// }

// impl Default for Rect {
//     fn default() -> Self {
//         Rect(GeoRect::new((0, 0), (0, 0)))
//     }
// }

#[derive(Default)]
pub struct Tile {
    pub extents: GeoRect,
    pub shapes: Vec<usize>,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct HiResTileMarker;

#[derive(Debug, Clone, Copy, Component)]
pub struct DownscaledTileMarker;

impl std::fmt::Debug for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Tile {{ {:?}, num_shapes: {} }}",
            self.extents,
            self.shapes.len()
        )
    }
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct TileMapResource(TileMap);

pub type TileMap = HashMap<(u32, u32), Tile>;

#[derive(Debug, Default, Deref, DerefMut)]
pub struct FlattenedElems(Vec<raw::Element>);

struct RenderingDone {
    sender: Sender<()>,
    receiver: Receiver<()>,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(ShapePlugin)
        .add_plugin(PanCamPlugin)
        .add_plugin(Material2dPlugin::<PostProcessingMaterial>::default())
        .init_resource::<LayerColors>()
        .init_resource::<VlsirLib>()
        .init_resource::<FlattenedElems>()
        .init_resource::<TileMap>()
        .init_resource::<Layers>()
        .init_resource::<LibLayers>()
        .insert_resource({
            let (sender, receiver) = bounded::<()>(1);
            RenderingDone { sender, receiver }
        })
        .add_event::<OpenVlsirLibCompleteEvent>()
        .add_event::<DrawTileEvent>()
        .init_resource::<Msaa>()
        .add_startup_system(setup)
        .add_system(despawn_system)
        .add_system(spawn_system)
        .add_system(spawn_vlsir_open_task_sytem)
        .add_system(handle_vlsir_open_task_system)
        .add_system(load_lib_system)
        .run();
}

// Marks the first pass cube (rendered to a texture.)
#[derive(Component)]
struct LyonShape;

// Marks the first pass cube (rendered to a texture.)
#[derive(Component)]
struct TextureCam;

#[derive(Debug, Default, Clone, Copy)]
pub struct OpenVlsirLibCompleteEvent;

fn setup(mut commands: Commands) {
    commands
        .spawn_bundle(Camera2dBundle {
            camera: Camera {
                // renders after the cameras with lower values for priority
                priority: 2,
                ..default()
            },
            ..Camera2dBundle::default()
        })
        .insert(MAIN_CAMERA_LAYER)
        .insert(PanCam::default());
}

fn spawn_vlsir_open_task_sytem(mut commands: Commands, mut already_done: Local<bool>) {
    if !*already_done {
        let thread_pool = AsyncComputeTaskPool::get();

        let task: Task<Library> = thread_pool.spawn(async move {
            // enable to test UI Lib Info "Library:" loading spinner animation
            // std::thread::sleep(std::time::Duration::from_secs(5));
            let plib = raw::proto::proto::open(
                "/home/colepoirier/Dropbox/rust_2020_onwards/doug/doug/libs/oscibear.proto",
                // "/home/colepoirier/Dropbox/rust_2020_onwards/doug/doug/libs/
                // caravel_chameleon_soc.proto"
                // "/home/colepoirier/Dropbox/rust_2020_onwards/doug/doug/libs/dff1_lib.proto",
                // "/home/colepoirier/Dropbox/rust_2020_onwards/doug/doug/libs/
                // nangate45_bp_multi_6_final.proto"
            )
            .unwrap();
            ProtoImporter::import(&plib, None).unwrap()
        });
        let task = LibraryWrapper(task);
        commands.spawn().insert(task);
        *already_done = true;
    }
}

#[derive(Debug, Default)]
struct VlsirLib {
    pub lib: Option<Library>,
}

#[derive(Debug, Component, Deref, DerefMut)]
struct LibraryWrapper(Task<Library>);

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

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct Layers(HashMap<u8, Color>);

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct LibLayers(raw::Layers);

fn load_lib_system(
    mut vlsir_open_lib_complete_event_reader: EventReader<OpenVlsirLibCompleteEvent>,
    vlsir_lib: Res<VlsirLib>,
    mut layer_colors: ResMut<LayerColors>,
    mut layers: ResMut<Layers>,
    mut lib_layers: ResMut<LibLayers>,
    render_device: Res<RenderDevice>,
    mut tilemap: ResMut<TileMap>,
    mut flattened_elems_res: ResMut<FlattenedElems>,
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
        // let cell = lib.cells.iter().rev().nth(1).unwrap().read().unwrap();

        let flattened_elems = cell.layout.as_ref().unwrap().flatten().unwrap();

        info!("num elems including instances: {}", flattened_elems.len());

        let mut bbox = BoundBox::empty();
        for elem in flattened_elems.iter() {
            bbox = elem.inner.union(&bbox);
        }

        assert!(!bbox.is_empty(), "bbox must be valid!");

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

                // info!("tile ({ix}, {iy}): xmin: {xmin}, xmax: {xmax}, ymin:
                // {ymin}, ymax: {ymax}");
            }

            x = bbox.p0.x as i64;
        }

        let mut shape_count = 0;

        info!("{bbox:?}, dx: {dx}, dy: {dy}, tiles: [{num_x_tiles}, {num_y_tiles}]");

        let t = std::time::Instant::now();

        // let lib_layers = &**lib_layers;

        import_cell_shapes(
            tilemap_shift,
            texture_dim,
            &mut tilemap,
            &flattened_elems,
            &mut shape_count,
            // &lib_layers,
            // &layers,
        );

        info!("DONE {shape_count} shapes in {:?}!", t.elapsed());

        *flattened_elems_res = FlattenedElems(flattened_elems);

        stats(&tilemap);

        ev.send(DrawTileEvent((14, 260)));
    }
}

fn get_grid_shape(grid: &Vec<(u32, u32)>) -> (u32, u32) {
    let (mut x_min, mut x_max, mut y_min, mut y_max) = (0, 0, 0, 0);
    for &(x, y) in grid.iter() {
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

    // let mut max = (0, 0);

    // for (idx, grid) in layers.iter().enumerate() {
    //     let size = grid.len();

    //     if size > max.1 {
    //         max = (idx, size);
    //     }
    // }

    // println!("{max:?}");

    // let grid = &layers[max.0];

    for v in grid.values() {
        counts.push(v.shapes.len());
    }

    // info!("{counts:?}");

    let num_occupied_bins = counts.iter().filter(|x| **x != 0).collect::<Vec<_>>().len();
    let min = counts.iter().min().unwrap();
    let max = counts.iter().max().unwrap();
    let num_rects_incl_duplicates = counts.iter().sum::<usize>();
    // avg_spob = average shapes per occupied bin
    let avg_spob = counts.iter().sum::<usize>() / counts.len();
    // let dev_spob = ((counts.iter().map(|c| (c - avg_spob).pow(2)).sum::<usize>()
    //     / (counts.len() - 1).max(1)) as f64)
    //     .powf(0.5);

    let grid_size = get_grid_shape(&grid.keys().map(|x| *x).collect::<Vec<(u32, u32)>>());

    // let mut table = vec![];

    let mut wtr = Writer::from_path("table_heatmap_data.csv").unwrap();

    for iy in 0..grid_size.1 {
        let mut row = vec![];
        for ix in 0..grid_size.0 {
            let count = grid.get(&(ix, iy)).unwrap().shapes.len();
            row.push(count.to_string());
        }

        wtr.write_record(&row[..]).unwrap();

        // println!("{row}");
    }

    wtr.flush().unwrap();

    // let mut f = std::fs::File::create("table_heatmap_data").unwrap();

    // f.write(format!("{table:?}").as_bytes()).unwrap();

    // println!("{table:?}");

    let num_bins = (grid_size.0 * grid_size.1) as usize;
    let grid_occupancy = num_occupied_bins as f32 / num_bins as f32;
    // info!("bin_shape_counts: {counts:?}");
    info!(
        "grid_size: {grid_size:?}, num_bins: {num_bins}, num_occupied_bins: {num_occupied_bins}, num_rects_incl_duplicates: {num_rects_incl_duplicates}"
    );
    info!("grid_occupancy: {grid_occupancy}");
    info!(
        "avg shapes per occupied bin: {}",
        num_rects_incl_duplicates as f32 / num_occupied_bins as f32
    );
    // info!("min: {min}, max: {max}, avg_spob: {avg_spob}, dev_spob: {dev_spob}");
    info!("min: {min}, max: {max}, avg_spob: {avg_spob}");

    // println!("number of empty bins: {num_empty_bins}");
}

pub fn import_cell_shapes(
    tilemap_shift: raw::Point,
    texture_dim: u32,
    tilemap: &mut TileMap,
    // cell: &Ptr<raw::Cell>,
    elems: &Vec<raw::Element>,
    shape_count: &mut u64,
    // lib_layers: &raw::Layers,
    // layers: &HashMap<u8, Color>,
) {
    // let read_cell = cell.read().unwrap();
    // let read_lib_layers = lib_layers.read().unwrap();

    // let layout = read_cell.layout.as_ref().unwrap();

    for (idx, raw::Element { inner, .. }) in elems.iter().enumerate() {
        // continue;
        // let layer = lib_layers
        //     .get(*layer)
        //     .expect("This Element's LayerKey does not exist in this Library's
        // Layers")     .layernum as u8;

        // let color = layers.get(&layer).unwrap();

        // .expect(&format!(
        //     "This Element's layer num: {layer} does not exist in our Layers Resource:
        // {layers:?}" ));

        let mut bbox = inner.bbox();

        if !bbox.is_empty() {
            // info!("{bbox:?}");

            bbox.p0 = bbox.p0.shift(&tilemap_shift);
            bbox.p1 = bbox.p1.shift(&tilemap_shift);

            // bbox.p0 = bbox.p0.shift(&offset);
            // bbox.p1 = bbox.p1.shift(&offset);

            let BoundBox { p0, p1 } = bbox;

            // info!("{bbox:?}");

            let min_tile_x = p0.x as u32 / texture_dim;
            let min_tile_y = p0.y as u32 / texture_dim;
            let max_tile_x = p1.x as u32 / texture_dim;
            let max_tile_y = p1.y as u32 / texture_dim;

            // info!("inner: {inner:?}");

            let geo_shape = match inner {
                raw::Shape::Rect(r) => {
                    let raw::Rect { p0, p1 } = r;

                    // let p0 = p0.shift(offset);
                    // let p1 = p1.shift(offset);

                    let xmin = p0.x as i64;
                    let ymin = p0.y as i64;
                    let xmax = p1.x as i64;
                    let ymax = p1.y as i64;

                    let rect = GeoRect::new((xmin, ymin), (xmax, ymax));

                    // let lyon_poly = shapes::Polygon {
                    //     points: vec![
                    //         (xmin as f32, ymin as f32).into(),
                    //         (xmax as f32, ymin as f32).into(),
                    //         (xmax as f32, ymax as f32).into(),
                    //         (xmin as f32, ymax as f32).into(),
                    //     ],
                    //     closed: true,
                    // };

                    // let transform = Transform::from_translation(Vec3::new(0.0, 0.0, layer as
                    // f32));

                    // let lyon_shape = GeometryBuilder::build_as(
                    //     &lyon_poly,
                    //     DrawMode::Outlined {
                    //         fill_mode: FillMode {
                    //             color: *color.clone().set_a(ALPHA),
                    //             options: FillOptions::default(),
                    //         },
                    //         outline_mode: StrokeMode {
                    //             options: StrokeOptions::default().with_line_width(WIDTH),
                    //             color: *color,
                    //         },
                    //     },
                    //     transform,
                    // );

                    let geo_shape = GeoShapeEnum::Rect(rect);

                    // info!("georect: {geo_shape:?}");

                    // (
                    geo_shape
                    //     lyon_shape
                    // )
                }
                raw::Shape::Polygon(p) => {
                    let poly = GeoPolygon::new(
                        p.points
                            .iter()
                            .map(|p| {
                                // let p = p.shift(offset);
                                (p.x as i64, p.y as i64)
                            })
                            .collect(),
                        vec![],
                    );

                    // let lyon_poly = shapes::Polygon {
                    //     points: poly
                    //         .exterior()
                    //         .coords()
                    //         .map(|c| Vec2::new(c.x as f32, c.y as f32))
                    //         .collect::<Vec<Vec2>>(),
                    //     closed: true,
                    // };

                    let geo_shape = GeoShapeEnum::Polygon(poly);

                    // let transform = Transform::from_translation(Vec3::new(0.0, 0.0, layer as
                    // f32));

                    // let lyon_shape = GeometryBuilder::build_as(
                    //     &lyon_poly,
                    //     DrawMode::Outlined {
                    //         fill_mode: FillMode {
                    //             color: *color.clone().set_a(ALPHA),
                    //             options: FillOptions::default(),
                    //         },
                    //         outline_mode: StrokeMode {
                    //             options: StrokeOptions::default().with_line_width(WIDTH),
                    //             color: *color,
                    //         },
                    //     },
                    //     transform,
                    // );

                    // (
                    geo_shape
                    //     lyon_shape
                    // )
                }
                raw::Shape::Path(p) => {
                    // let lyon_path = shapes::Polygon {
                    //     points: p
                    //         .points
                    //         .iter()
                    //         // .map(|p| p.shift(offset))
                    //         .map(|raw::Point { x, y }| Vec2::new(*x as f32, *y as f32))
                    //         .collect::<Vec<Vec2>>(),
                    //     closed: false,
                    // };

                    let num_points = p.points.len();
                    let mut forward_poly_points = Vec::with_capacity(num_points);
                    let mut backward_poly_points = Vec::with_capacity(num_points);
                    assert_eq!(
                        p.width % 2,
                        0,
                        "width must be even for our code's assumptions to hold!"
                    );
                    let half_width = (p.width / 2) as isize; // assuming that widths are even!
                    for ix in 0..num_points {
                        let p0 = p.points[ix];
                        let p1 = p.points[(ix + 1) % num_points];
                        // let corrected_point = p0.shift(offset);
                        if p0.x == p1.x {
                            // horizontal
                            forward_poly_points.push(raw::Point {
                                x: p0.x,
                                y: p0.y - half_width,
                            });
                            backward_poly_points.push(raw::Point {
                                x: p0.x,
                                y: p0.y + half_width,
                            });
                        } else {
                            // vertical
                            forward_poly_points.push(raw::Point {
                                x: p0.x + half_width,
                                y: p0.y,
                            });
                            backward_poly_points.push(raw::Point {
                                x: p0.x - half_width,
                                y: p0.y,
                            });
                        }
                    }
                    let poly = GeoPolygon::new(
                        forward_poly_points
                            .into_iter()
                            .chain(backward_poly_points.into_iter().rev())
                            .map(|p| (p.x as i64, p.y as i64))
                            .collect(),
                        vec![],
                    );

                    let geo_shape = GeoShapeEnum::Polygon(poly);

                    // let transform = Transform::from_translation(Vec3::new(0.0, 0.0, layer as
                    // f32));

                    // let lyon_shape = GeometryBuilder::build_as(
                    //     &lyon_path,
                    //     DrawMode::Outlined {
                    //         fill_mode: FillMode {
                    //             color: *color.clone().set_a(ALPHA),
                    //             options: FillOptions::default(),
                    //         },
                    //         outline_mode: StrokeMode {
                    //             options: StrokeOptions::default().with_line_width(p.width as
                    // f32),             color: *color,
                    //         },
                    //     },
                    //     transform,
                    // );

                    // (
                    geo_shape
                    //     lyon_shape
                    // )
                }
            };

            // info!("offset {offset:?}");

            for x in min_tile_x..(max_tile_x + 1) {
                for y in min_tile_y..(max_tile_y + 1) {
                    // info!("trying to get tile ({x}, {y}) to insert shape");
                    let Tile { extents, shapes } = tilemap.get_mut(&(x, y)).unwrap();
                    // .expect(&format!(
                    //     "trying to get tile ({x}, {y}) to insert
                    // shape:\n{bbox:#?},\n{geo_shape:#?},\n{inner:#?},\noffset:
                    // {offset:#?},\ntilemap_shift: {tilemap_shift:#?}" ));

                    let extents = &*extents;

                    match &geo_shape {
                        GeoShapeEnum::Rect(r) => {
                            if r.intersects(extents) {
                                shapes.push(idx);
                            }
                        }
                        GeoShapeEnum::Polygon(p) => {
                            // info!("{} {extents:?} x {p:?}", *shape_count + 1);
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

    // for raw::Instance {
    //     inst_name: _,
    //     cell,
    //     loc,
    //     reflect_vert: _,
    //     angle: _,
    // } in layout.insts.iter()
    // {
    //     import_cell_shapes(
    //         tilemap_shift,
    //         texture_dim,
    //         tilemap,
    //         cell,
    //         shape_count,
    //         loc,
    //         lib_layers,
    //         layers,
    //     );
    // }
}

#[derive(Debug, Default, Clone, Copy)]
struct DrawTileEvent((u32, u32));

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
    rendering_done: Res<RenderingDone>,
    asset_server: Res<AssetServer>,
) {
    asset_server.watch_for_changes().unwrap();
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
            // info!("{el:?}");

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

                // info!("{lyon_poly:?}");

                let transform = Transform::from_translation(Vec3::new(0.0, 0.0, layer as f32));

                // let color = Color::rgb(1.0, 0.0, 0.0);

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
                    // DrawMode::Fill(FillMode {
                    //     color: *color.clone().set_a(ALPHA),
                    //     options: FillOptions::default(),
                    // }),
                    transform,
                    // Transform::default(),
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

        // let transform = Transform::from_translation(Vec3::new(-1000.0, -640_000.0,
        // 15.0));

        // let transform = Transform::from_translation(Vec3::new(0.0, 0.0, 15.0));

        // let transform = Transform::from_translation(Vec3::new(
        //     -(size.width as f32) / 2.0,
        //     -(size.height as f32) / 2.0,
        //     15.0,
        // ));

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

        let post_processing_pass_layer = RenderLayers::layer(1);

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
            .insert(post_processing_pass_layer);

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

        // The post-processing pass camera.
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
            .insert(post_processing_pass_layer)
            .insert(TextureCam);

        commands
            .spawn_bundle(SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(size.width as f32, size.height as f32)),
                    ..default()
                },
                texture: downscaled_image_handle.clone(),
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
                ..default()
            })
            .insert(MAIN_CAMERA_LAYER);

        // camera.projection.window_origin = WindowOrigin::BottomLeft;

        let s = rendering_done.sender.clone();

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
    rendering_done: Res<RenderingDone>,
) {
    if let Ok(()) = rendering_done.receiver.try_recv() {
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

// // Region below declares of the custom material handling post processing effect

// /// Our custom post processing material
// #[derive(AsBindGroup, TypeUuid, Clone)]
// #[uuid = "bc2f08eb-a0fb-43f1-a908-54871ea597d5"]
// struct PostProcessingMaterial {
//     /// In this example, this image will be the result of the main camera.
//     #[texture(0)]
//     #[sampler(1)]
//     source_image: Handle<Image>,
// }

// impl Material2d for PostProcessingMaterial {
//     fn vertex_shader() -> ShaderRef {
//         "shaders/fullscreen.wgsl".into()
//     }
//     fn fragment_shader() -> ShaderRef {
//         "shaders/downscaling.wgsl".into()
//     }
//     // fn fragment_shader() -> ShaderRef {
//     //     "shaders/unrolled.wgsl".into()
//     // }
// }

/// Our custom post processing material
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
