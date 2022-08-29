use bevy::{
    prelude::{Bundle, Component, Deref, DerefMut},
    render::view::RenderLayers,
    utils::HashMap,
};

use crossbeam_channel::{Receiver, Sender};

use std::ops::Range;

// constants
pub const ACCUMULATION_CAMERA_PRIORITY: isize = 1;
pub const MAIN_CAMERA_PRIORITY: isize = 2;

pub const DOWNSCALING_PASS_LAYER: RenderLayers = RenderLayers::layer(1);
pub const MAIN_CAMERA_LAYER: RenderLayers = RenderLayers::layer(2);

pub const ALPHA: f32 = 0.1;
pub const WIDTH: f32 = 10.0;

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

// TileMap related types

pub type GeoRect = geo::Rect<i64>;

#[derive(Debug)]
pub struct Tile {
    pub extents: GeoRect,
    pub shapes: Vec<usize>,
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct Tilemap(HashMap<(u32, u32), Tile>);

#[derive(Debug, Default)]
pub struct TilemapLowerLeft {
    pub x: i64,
    pub y: i64,
}

// Resources
#[derive(Debug, Default, Deref, DerefMut)]
pub struct FlattenedElems(pub Vec<Rect>);

#[derive(Debug, Default, Deref, DerefMut)]
pub struct TileIndexIter(pub Option<itertools::Product<Range<u32>, Range<u32>>>);

pub struct RenderingDoneChannel {
    pub sender: Sender<()>,
    pub receiver: Receiver<()>,
}

// Events

#[derive(Debug, Default, Clone, Copy)]
pub struct DrawTileEvent(pub (u32, u32));

#[derive(Debug, Default)]
pub struct RenderingCompleteEvent;

// Components

#[derive(Debug, Component)]
pub struct MainCamera;

#[derive(Component, Clone, Debug, Default)]
pub struct LyonShape;

#[derive(Component, Debug)]
pub struct TextureCam;

#[derive(Debug, Clone, Copy, Component)]
pub struct HiResTileMarker;

#[derive(Debug, Clone, Copy, Component)]
pub struct DownscaledTileMarker;

#[derive(Bundle, Default)]
pub struct LyonShapeBundle {
    #[bundle]
    pub lyon: bevy_prototype_lyon::entity::ShapeBundle,
    pub marker: LyonShape,
}
