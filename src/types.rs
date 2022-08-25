use bevy::{
    prelude::{Bundle, Color, Component, Deref, DerefMut},
    render::view::RenderLayers,
    tasks::Task,
    utils::HashMap,
};

use layout21::raw::{self, Library};

use crossbeam_channel::{Receiver, Sender};

use std::ops::Range;

// constants
pub const ACCUMULATION_CAMERA_PRIORITY: isize = 1;
pub const MAIN_CAMERA_PRIORITY: isize = 2;

pub const DOWNSCALING_PASS_LAYER: RenderLayers = RenderLayers::layer(1);
pub const MAIN_CAMERA_LAYER: RenderLayers = RenderLayers::layer(2);

pub const ALPHA: f32 = 0.1;
pub const WIDTH: f32 = 10.0;

// TileMap related types

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
    pub x: i64,
    pub y: i64,
}

// Resources
#[derive(Debug, Default, Deref, DerefMut)]
pub struct FlattenedElems(pub Vec<raw::Element>);

#[derive(Debug, Default)]
pub struct VlsirLib {
    pub lib: Option<Library>,
}

#[derive(Debug, Component, Deref, DerefMut)]
pub struct LibraryWrapper(pub Task<Library>);

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct Layers(HashMap<u8, Color>);

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct LibLayers(pub raw::Layers);

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
pub struct TileIndexIter(pub Option<itertools::Product<Range<u32>, Range<u32>>>);

pub struct RenderingDoneChannel {
    pub sender: Sender<()>,
    pub receiver: Receiver<()>,
}

// Events

#[derive(Debug, Default, Clone, Copy)]
pub struct OpenVlsirLibCompleteEvent;

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
