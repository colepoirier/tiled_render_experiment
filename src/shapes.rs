use crate::bbox::{BoundingBox, CalculateBoundingBox, UnvalidatedBoundingBox};
use rkyv::{vec::ArchivedVec, Archive, Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Archive, Default, Deserialize, Serialize, Clone, Copy)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug, Copy, Clone))]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl From<ArchivedPoint> for Point {
    fn from(p: ArchivedPoint) -> Self {
        Self { x: p.x, y: p.y }
    }
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

impl ArchivedPoint {
    /// Create a new point shifted by `x` in the x-dimension and by `y` in the y-dimension
    pub fn shift(&self, p: &Point) -> Point {
        Point {
            x: p.x + self.x,
            y: p.y + self.y,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Archive, Deserialize, Serialize, Clone, Copy)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Rect {
    pub p0: Point,
    pub p1: Point,
    pub layer: u8,
}

#[derive(Debug, Eq, PartialEq, Archive, Deserialize, Serialize)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Poly {
    pub points: Vec<Point>,
    pub layer: u8,
}

#[derive(Debug, Eq, PartialEq, Archive, Deserialize, Serialize)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Path {
    pub points: Vec<Point>,
    pub width: u32,
    pub layer: u8,
}

pub trait Layer {
    fn layer(&self) -> u8;
}

impl Layer for ArchivedShape {
    fn layer(&self) -> u8 {
        match self {
            ArchivedShape::Rect(r) => r.layer,
            ArchivedShape::Poly(p) => p.layer,
            ArchivedShape::Path(p) => p.layer,
        }
    }
}

mod path_to_poly {
    use super::Point;
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RectMove {
        Left,
        Right,
        Up,
        Down,
    }

    pub fn calculate_move(p0: Point, p1: Point) -> RectMove {
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
            panic!("rectilinear moves expected, but found: p0 {p0:?} and p1 {p1:?}");
        }
    }

    pub fn shift_pure_right(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift pure right");
        forward_poly_points.push(Point {
            x: p0.x,
            y: p0.y - half_width,
        });
        backward_poly_points.push(Point {
            x: p0.x,
            y: p0.y + half_width,
        });
    }

    pub fn shift_pure_left(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift pure left");
        forward_poly_points.push(Point {
            x: p0.x,
            y: p0.y + half_width,
        });
        backward_poly_points.push(Point {
            x: p0.x,
            y: p0.y - half_width,
        });
    }

    pub fn shift_pure_up(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift pure up");
        forward_poly_points.push(Point {
            x: p0.x + half_width,
            y: p0.y,
        });
        backward_poly_points.push(Point {
            x: p0.x - half_width,
            y: p0.y,
        });
    }

    pub fn shift_pure_down(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift pure down");
        forward_poly_points.push(Point {
            x: p0.x - half_width,
            y: p0.y,
        });
        backward_poly_points.push(Point {
            x: p0.x + half_width,
            y: p0.y,
        });
    }

    pub fn shift_right_up(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift right up");
        forward_poly_points.push(Point {
            x: p0.x + half_width,
            y: p0.y - half_width,
        });
        backward_poly_points.push(Point {
            x: p0.x - half_width,
            y: p0.y + half_width,
        });
    }

    pub fn shift_left_down(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift left down (calling shift right up)");
        shift_right_up(backward_poly_points, forward_poly_points, p0, half_width);
    }

    pub fn shift_right_down(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift right down");
        forward_poly_points.push(Point {
            x: p0.x - half_width,
            y: p0.y - half_width,
        });
        backward_poly_points.push(Point {
            x: p0.x + half_width,
            y: p0.y + half_width,
        });
    }

    pub fn shift_left_up(
        forward_poly_points: &mut Vec<Point>,
        backward_poly_points: &mut Vec<Point>,
        p0: Point,
        half_width: i32,
    ) {
        // println!("shift left up (calling shift right down)");
        shift_right_down(backward_poly_points, forward_poly_points, p0, half_width);
    }
}

impl ArchivedPath {
    pub fn as_poly(&self) -> Vec<Point> {
        use path_to_poly::*;
        let pts = &self.points;
        let width = self.width;

        let num_points = pts.len();

        assert_eq!(
            width % 2,
            0,
            "width must be even for our code's assumptions to hold!"
        );
        let half_width = (width / 2) as i32; // assuming that widths are even!

        if num_points == 2 {
            if pts[0].x == pts[1].x {
                // vertical
                return vec![
                    Point {
                        x: pts[0].x + half_width,
                        y: pts[0].y,
                    },
                    Point {
                        x: pts[1].x + half_width,
                        y: pts[1].y,
                    },
                    Point {
                        x: pts[1].x - half_width,
                        y: pts[1].y,
                    },
                    Point {
                        x: pts[0].x - half_width,
                        y: pts[0].y,
                    },
                ];
            } else {
                return vec![
                    // horizontal
                    Point {
                        x: pts[0].x,
                        y: pts[0].y - half_width,
                    },
                    Point {
                        x: pts[1].x,
                        y: pts[1].y - half_width,
                    },
                    Point {
                        x: pts[1].x,
                        y: pts[1].y + half_width,
                    },
                    Point {
                        x: pts[0].x,
                        y: pts[0].y + half_width,
                    },
                ];
            }
        }

        let mut forward_poly_points = Vec::with_capacity(num_points);
        let mut backward_poly_points = Vec::with_capacity(num_points);

        assert!(
            num_points > 1,
            "Expected number of points in path to be > 1"
        );
        let start_move = calculate_move(pts[0].into(), pts[1].into());

        match start_move {
            RectMove::Right => shift_pure_right(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
            RectMove::Left => shift_pure_left(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
            RectMove::Up => shift_pure_up(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
            RectMove::Down => shift_pure_down(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
        }

        let mut last_move = start_move;

        for ix in 1..(num_points - 1) {
            let p0: Point = pts[ix].into();
            let p1: Point = pts[ix + 1].into();
            let next_move = calculate_move(p0, p1);
            match (last_move, next_move) {
                (RectMove::Right, RectMove::Right) => shift_pure_right(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Left, RectMove::Left) => shift_pure_left(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Up, RectMove::Up) => shift_pure_up(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Down, RectMove::Down) => shift_pure_down(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Right, RectMove::Down) | (RectMove::Down, RectMove::Right) => {
                    shift_right_down(
                        &mut forward_poly_points,
                        &mut backward_poly_points,
                        p0,
                        half_width,
                    )
                }
                (RectMove::Right, RectMove::Up) | (RectMove::Up, RectMove::Right) => {
                    shift_right_up(
                        &mut forward_poly_points,
                        &mut backward_poly_points,
                        p0,
                        half_width,
                    )
                }
                (RectMove::Left, RectMove::Up) | (RectMove::Up, RectMove::Left) => shift_left_up(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Left, RectMove::Down) | (RectMove::Down, RectMove::Left) => {
                    shift_left_down(
                        &mut forward_poly_points,
                        &mut backward_poly_points,
                        p0,
                        half_width,
                    )
                }
                (_, _) => panic!(
                    "Received opposing last/next moves!" // "last: {last_move:?}, next: {next_move:?}"
                ),
            }
            last_move = next_move;
        }

        let end_move = calculate_move(pts[num_points - 2].into(), pts[num_points - 1].into());
        match end_move {
            RectMove::Right => shift_pure_right(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
            RectMove::Left => shift_pure_left(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
            RectMove::Up => shift_pure_up(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
            RectMove::Down => shift_pure_down(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
        }

        // println!("forward: {forward_poly_points:?}");
        // println!(
        //     "backward: {:?}",
        //     backward_poly_points.iter().collect::<Vec<&Point>>()
        // );

        let points: Vec<Point> = forward_poly_points
            .into_iter()
            .chain(backward_poly_points.into_iter().rev())
            .collect();

        // println!("points: {points:?}");

        points
    }
}

impl Path {
    pub fn as_poly(&self) -> Vec<Point> {
        use path_to_poly::*;
        let pts = &self.points;
        let width = self.width;

        let num_points = pts.len();

        assert_eq!(
            width % 2,
            0,
            "width must be even for our code's assumptions to hold!"
        );
        let half_width = (width / 2) as i32; // assuming that widths are even!

        if num_points == 2 {
            if pts[0].x == pts[1].x {
                // vertical
                return vec![
                    Point {
                        x: pts[0].x + half_width,
                        y: pts[0].y,
                    },
                    Point {
                        x: pts[1].x + half_width,
                        y: pts[1].y,
                    },
                    Point {
                        x: pts[1].x - half_width,
                        y: pts[1].y,
                    },
                    Point {
                        x: pts[0].x - half_width,
                        y: pts[0].y,
                    },
                ];
            } else {
                return vec![
                    // horizontal
                    Point {
                        x: pts[0].x,
                        y: pts[0].y - half_width,
                    },
                    Point {
                        x: pts[1].x,
                        y: pts[1].y - half_width,
                    },
                    Point {
                        x: pts[1].x,
                        y: pts[1].y + half_width,
                    },
                    Point {
                        x: pts[0].x,
                        y: pts[0].y + half_width,
                    },
                ];
            }
        }

        let mut forward_poly_points = Vec::with_capacity(num_points);
        let mut backward_poly_points = Vec::with_capacity(num_points);

        assert!(
            num_points > 1,
            "Expected number of points in path to be > 1"
        );
        let start_move = calculate_move(pts[0].into(), pts[1].into());

        match start_move {
            RectMove::Right => shift_pure_right(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
            RectMove::Left => shift_pure_left(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
            RectMove::Up => shift_pure_up(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
            RectMove::Down => shift_pure_down(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[0].into(),
                half_width,
            ),
        }

        let mut last_move = start_move;

        for ix in 1..(num_points - 1) {
            let p0: Point = pts[ix].into();
            let p1: Point = pts[ix + 1].into();
            let next_move = calculate_move(p0, p1);
            match (last_move, next_move) {
                (RectMove::Right, RectMove::Right) => shift_pure_right(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Left, RectMove::Left) => shift_pure_left(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Up, RectMove::Up) => shift_pure_up(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Down, RectMove::Down) => shift_pure_down(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Right, RectMove::Down) | (RectMove::Down, RectMove::Right) => {
                    shift_right_down(
                        &mut forward_poly_points,
                        &mut backward_poly_points,
                        p0,
                        half_width,
                    )
                }
                (RectMove::Right, RectMove::Up) | (RectMove::Up, RectMove::Right) => {
                    shift_right_up(
                        &mut forward_poly_points,
                        &mut backward_poly_points,
                        p0,
                        half_width,
                    )
                }
                (RectMove::Left, RectMove::Up) | (RectMove::Up, RectMove::Left) => shift_left_up(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    p0,
                    half_width,
                ),
                (RectMove::Left, RectMove::Down) | (RectMove::Down, RectMove::Left) => {
                    shift_left_down(
                        &mut forward_poly_points,
                        &mut backward_poly_points,
                        p0,
                        half_width,
                    )
                }
                (_, _) => panic!(
                    "Received opposing last/next moves!" // "last: {last_move:?}, next: {next_move:?}"
                ),
            }
            last_move = next_move;
        }

        let end_move = calculate_move(pts[num_points - 2].into(), pts[num_points - 1].into());
        match end_move {
            RectMove::Right => shift_pure_right(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
            RectMove::Left => shift_pure_left(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
            RectMove::Up => shift_pure_up(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
            RectMove::Down => shift_pure_down(
                &mut forward_poly_points,
                &mut backward_poly_points,
                pts[num_points - 1].into(),
                half_width,
            ),
        }

        // println!("forward: {forward_poly_points:?}");
        // println!(
        //     "backward: {:?}",
        //     backward_poly_points.iter().collect::<Vec<&Point>>()
        // );

        let points: Vec<Point> = forward_poly_points
            .into_iter()
            .chain(backward_poly_points.into_iter().rev())
            .collect();

        // println!("points: {points:?}");

        points
    }
}

#[derive(Debug, Eq, PartialEq, Archive, Deserialize, Serialize)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Shapes {
    pub bbox: BoundingBox,
    pub shapes: Vec<Shape>,
}

#[derive(Debug, Eq, PartialEq, Archive, Deserialize, Serialize)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub enum Shape {
    Rect(Rect),
    Poly(Poly),
    Path(Path),
}

impl CalculateBoundingBox for ArchivedShape {
    fn bbox(&self) -> BoundingBox {
        match self {
            Self::Rect(r) => {
                let mut bbox = UnvalidatedBoundingBox::invalid();

                bbox.min.x = r.p0.x.min(bbox.min.x);
                bbox.min.y = r.p0.y.min(bbox.min.y);
                bbox.max.x = r.p0.x.max(bbox.max.x);
                bbox.max.y = r.p0.y.max(bbox.max.y);
                bbox.min.x = r.p1.x.min(bbox.min.x);
                bbox.min.y = r.p1.y.min(bbox.min.y);
                bbox.max.x = r.p1.x.max(bbox.max.x);
                bbox.max.y = r.p1.y.max(bbox.max.y);

                BoundingBox::new(bbox)
            }
            Self::Poly(p) => {
                let mut bbox = UnvalidatedBoundingBox::invalid();
                for pt in p.points.iter() {
                    bbox.min.x = pt.x.min(bbox.min.x);
                    bbox.min.y = pt.y.min(bbox.min.y);
                    bbox.max.x = pt.x.max(bbox.max.x);
                    bbox.max.y = pt.y.max(bbox.max.y);
                }
                BoundingBox::new(bbox)
            }
            Self::Path(p) => {
                let mut bbox = UnvalidatedBoundingBox::invalid();

                let pts = p.as_poly();

                for pt in pts.iter() {
                    bbox.min.x = pt.x.min(bbox.min.x);
                    bbox.min.y = pt.y.min(bbox.min.y);
                    bbox.max.x = pt.x.max(bbox.max.x);
                    bbox.max.y = pt.y.max(bbox.max.y);
                }
                BoundingBox::new(bbox)
            }
        }
    }
}

impl CalculateBoundingBox for Shape {
    fn bbox(&self) -> BoundingBox {
        match self {
            Self::Rect(r) => {
                let mut bbox = UnvalidatedBoundingBox::invalid();

                bbox.min.x = r.p0.x.min(bbox.min.x);
                bbox.min.y = r.p0.y.min(bbox.min.y);
                bbox.max.x = r.p0.x.max(bbox.max.x);
                bbox.max.y = r.p0.y.max(bbox.max.y);
                bbox.min.x = r.p1.x.min(bbox.min.x);
                bbox.min.y = r.p1.y.min(bbox.min.y);
                bbox.max.x = r.p1.x.max(bbox.max.x);
                bbox.max.y = r.p1.y.max(bbox.max.y);

                BoundingBox::new(bbox)
            }
            Self::Poly(p) => {
                let mut bbox = UnvalidatedBoundingBox::invalid();
                for pt in p.points.iter() {
                    bbox.min.x = pt.x.min(bbox.min.x);
                    bbox.min.y = pt.y.min(bbox.min.y);
                    bbox.max.x = pt.x.max(bbox.max.x);
                    bbox.max.y = pt.y.max(bbox.max.y);
                }
                BoundingBox::new(bbox)
            }
            Self::Path(p) => {
                let mut bbox = UnvalidatedBoundingBox::invalid();

                let pts = p.as_poly();

                for pt in pts.iter() {
                    bbox.min.x = pt.x.min(bbox.min.x);
                    bbox.min.y = pt.y.min(bbox.min.y);
                    bbox.max.x = pt.x.max(bbox.max.x);
                    bbox.max.y = pt.y.max(bbox.max.y);
                }
                BoundingBox::new(bbox)
            }
        }
    }
}

impl CalculateBoundingBox for &ArchivedVec<ArchivedShape> {
    fn bbox(&self) -> BoundingBox {
        let mut bbox = UnvalidatedBoundingBox::invalid();
        for s in self.iter() {
            match s {
                ArchivedShape::Rect(r) => {
                    bbox.min.x = r.p0.x.min(bbox.min.x);
                    bbox.min.y = r.p0.y.min(bbox.min.y);
                    bbox.max.x = r.p0.x.max(bbox.max.x);
                    bbox.max.y = r.p0.y.max(bbox.max.y);
                    bbox.min.x = r.p1.x.min(bbox.min.x);
                    bbox.min.y = r.p1.y.min(bbox.min.y);
                    bbox.max.x = r.p1.x.max(bbox.max.x);
                    bbox.max.y = r.p1.y.max(bbox.max.y);
                }
                ArchivedShape::Poly(p) => {
                    for pt in p.points.iter() {
                        bbox.min.x = pt.x.min(bbox.min.x);
                        bbox.min.y = pt.y.min(bbox.min.y);
                        bbox.max.x = pt.x.max(bbox.max.x);
                        bbox.max.y = pt.y.max(bbox.max.y);
                    }
                }
                ArchivedShape::Path(p) => {
                    let pts = p.as_poly();

                    for pt in pts.iter() {
                        bbox.min.x = pt.x.min(bbox.min.x);
                        bbox.min.y = pt.y.min(bbox.min.y);
                        bbox.max.x = pt.x.max(bbox.max.x);
                        bbox.max.y = pt.y.max(bbox.max.y);
                    }
                }
            }
        }
        BoundingBox::new(bbox)
    }
}

impl CalculateBoundingBox for Vec<Shape> {
    fn bbox(&self) -> BoundingBox {
        let mut shapes = self.iter();
        let mut bbox = shapes.next().unwrap().bbox();
        for s in shapes {
            bbox.union(&s.bbox());
        }
        bbox
    }
}
