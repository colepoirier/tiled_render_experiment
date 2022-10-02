use layout21::raw;

use crate::types::GeoPolygon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RectMove {
    Left,
    Right,
    Up,
    Down,
}

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
        panic!("rectilinear moves expected, but found: p0 {p0:?} and p1 {p1:?}");
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

pub fn make_path_into_polygon(path: &raw::Path) -> GeoPolygon {
    let num_points = path.points.len();

    let mut forward_poly_points = Vec::with_capacity(num_points);
    let mut backward_poly_points = Vec::with_capacity(num_points);
    assert_eq!(
        path.width % 2,
        0,
        "width must be even for our code's assumptions to hold!"
    );
    let half_width = (path.width / 2) as isize; // assuming that widths are even!

    assert!(
        num_points > 1,
        "Expected number of points in path to be > 1"
    );
    let start_move = calculate_move(path.points[0], path.points[1]);

    match start_move {
        RectMove::Right => shift_pure_right(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[0],
            half_width,
        ),
        RectMove::Left => shift_pure_left(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[0],
            half_width,
        ),
        RectMove::Up => shift_pure_up(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[0],
            half_width,
        ),
        RectMove::Down => shift_pure_down(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[0],
            half_width,
        ),
    }

    let mut last_move = start_move;

    for ix in 1..(num_points - 1) {
        let p0 = path.points[ix];
        let p1 = path.points[ix + 1];
        let next_move = calculate_move(p0, p1);
        match (last_move, next_move) {
            (RectMove::Right, RectMove::Right) => shift_pure_right(
                &mut forward_poly_points,
                &mut backward_poly_points,
                path.points[0],
                half_width,
            ),
            (RectMove::Left, RectMove::Left) => shift_pure_left(
                &mut forward_poly_points,
                &mut backward_poly_points,
                path.points[0],
                half_width,
            ),
            (RectMove::Up, RectMove::Up) => shift_pure_up(
                &mut forward_poly_points,
                &mut backward_poly_points,
                path.points[0],
                half_width,
            ),
            (RectMove::Down, RectMove::Down) => shift_pure_down(
                &mut forward_poly_points,
                &mut backward_poly_points,
                path.points[0],
                half_width,
            ),
            (RectMove::Right, RectMove::Down) | (RectMove::Down, RectMove::Right) => {
                shift_right_down(
                    &mut forward_poly_points,
                    &mut backward_poly_points,
                    path.points[0],
                    half_width,
                )
            }
            (RectMove::Right, RectMove::Up) | (RectMove::Up, RectMove::Right) => shift_right_up(
                &mut forward_poly_points,
                &mut backward_poly_points,
                path.points[0],
                half_width,
            ),
            (RectMove::Left, RectMove::Up) | (RectMove::Up, RectMove::Left) => shift_left_up(
                &mut forward_poly_points,
                &mut backward_poly_points,
                path.points[0],
                half_width,
            ),
            (RectMove::Left, RectMove::Down) | (RectMove::Down, RectMove::Left) => shift_left_down(
                &mut forward_poly_points,
                &mut backward_poly_points,
                path.points[0],
                half_width,
            ),
            (_, _) => panic!(
                "Received opposing last/next moves! last: {last_move:?}, next: {next_move:?}"
            ),
        }
        last_move = next_move;
    }

    let end_move = calculate_move(path.points[num_points - 2], path.points[num_points - 1]);
    match end_move {
        RectMove::Right => shift_pure_right(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[num_points - 1],
            half_width,
        ),
        RectMove::Left => shift_pure_left(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[num_points - 1],
            half_width,
        ),
        RectMove::Up => shift_pure_up(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[num_points - 1],
            half_width,
        ),
        RectMove::Down => shift_pure_down(
            &mut forward_poly_points,
            &mut backward_poly_points,
            path.points[num_points - 1],
            half_width,
        ),
    }

    GeoPolygon::new(
        forward_poly_points
            .into_iter()
            .chain(backward_poly_points.into_iter().rev())
            .map(|p| (p.x as i64, p.y as i64))
            .collect(),
        vec![],
    )
}
