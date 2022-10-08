use bevy::{
    prelude::{Bundle, Color, Component, Deref, DerefMut, Handle, Image},
    render::view::RenderLayers,
    tasks::Task,
    utils::HashMap,
};

use crossbeam_channel::{Receiver, Sender};
use layout21::raw::{self, Library};

use std::ops::Range;

//
// constants
//

pub const ACCUMULATION_CAMERA_PRIORITY: isize = 1;
pub const MAIN_CAMERA_PRIORITY: isize = 2;

pub const DOWNSCALING_PASS_LAYER: RenderLayers = RenderLayers::layer(1);
pub const MAIN_CAMERA_LAYER: RenderLayers = RenderLayers::layer(2);

pub const ALPHA: f32 = 0.1;
pub const WIDTH: f32 = 10.0;

pub const NUM_TILES: u32 = 64;
pub const TILE_SIZE_IN_PX: u32 = 64;
pub const TEXTURE_DIM: u32 = NUM_TILES * TILE_SIZE_IN_PX;

#[derive(Debug, Eq, PartialEq, Default, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// Create a new point shifted by `x` in the x-dimension and by `y` in the y-dimension
    pub fn shift(&self, p: &Point) -> Point {
        Point {
            x: p.x + self.x,
            y: p.y + self.y,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Rect {
    pub p0: Point,
    pub p1: Point,
    pub layer: u8,
}

impl Rect {
    /// Create a new point shifted by `x` in the x-dimension and by `y` in the y-dimension
    pub fn shift(&self, p: &Point) -> Rect {
        Rect {
            p0: self.p0.shift(p),
            p1: self.p1.shift(p),
            layer: self.layer,
        }
    }
}

//
// Tilemap related types
//

pub type GeoRect = geo::Rect<i64>;
pub type GeoPolygon = geo::Polygon<i64>;

#[derive(Debug)]
pub struct Tile {
    pub extents: GeoRect,
    pub shapes: Vec<usize>,
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct Tilemap(pub HashMap<(u32, u32), Tile>);

#[derive(Debug, Default)]
pub struct TilemapLowerLeft {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Default, Deref)]
pub struct TileSizeInWorldSpace(pub u32);

#[derive(Debug, Default, Deref, DerefMut)]
pub struct TileIndexIter(pub Option<itertools::Product<Range<u32>, Range<u32>>>);

//
// Resources
//

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct Layers(HashMap<u8, Color>);

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct LibLayers(pub raw::Layers);

pub struct RenderingDoneChannel {
    pub sender: Sender<()>,
    pub receiver: Receiver<()>,
}

#[derive(Deref)]
pub struct HiResHandle(pub Handle<Image>);

#[derive(Deref)]
pub struct AccumulationHandle(pub Handle<Image>);

//
// Events
//

#[derive(Debug, Default, Clone, Copy)]
pub struct DrawTileEvent(pub (u32, u32));

#[derive(Debug, Default)]
pub struct RenderingCompleteEvent;

//
// Components
//

#[derive(Debug, Component)]
pub struct MainCamera;

#[derive(Component, Clone, Debug, Default)]
pub struct LyonShape;

#[derive(Component, Debug)]
pub struct HiResCam;

#[derive(Component, Debug)]
pub struct AccumulationCam;

#[derive(Bundle, Default)]
pub struct LyonShapeBundle {
    #[bundle]
    pub lyon: bevy_prototype_lyon::entity::ShapeBundle,
    pub marker: LyonShape,
}

#[derive(Debug, Clone)]
pub enum GeoShapeEnum {
    Rect(GeoRect),
    Polygon(GeoPolygon),
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct FlattenedElems(pub Vec<raw::Element>);

#[derive(Debug, Default, Clone, Copy)]
pub struct OpenVlsirLibCompleteEvent;

#[derive(Debug, Default)]
pub struct VlsirLib {
    pub lib: Option<Library>,
}

#[derive(Debug, Component, Deref, DerefMut)]
pub struct LibraryWrapper(pub Task<Library>);

// LayerColor

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
