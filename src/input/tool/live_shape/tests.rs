use crate::domain::BoardGrid;
use crate::draw::{BLACK, PolygonKind, Shape};

fn recognize(points: &[(i32, i32)], sensitivity: u8) -> Option<Shape> {
    super::recognize(points, BLACK, 3.0, BoardGrid::default(), sensitivity)
}

fn triangle_points(shape: Option<Shape>) -> Option<Vec<(i32, i32)>> {
    match shape {
        Some(Shape::Polygon {
            kind: PolygonKind::Triangle,
            points,
            ..
        }) => Some(points),
        _ => None,
    }
}

/// Walk a closed outline in `step` pixel increments from `start` (a share of
/// the perimeter), swaying sideways by up to `wobble` pixels like a shaky hand.
fn trace(corners: &[(f64, f64)], start: f64, step: f64, wobble: f64) -> Vec<(i32, i32)> {
    let sides: Vec<_> = (0..corners.len())
        .map(|index| (corners[index], corners[(index + 1) % corners.len()]))
        .collect();
    let perimeter: f64 = sides
        .iter()
        .map(|&(a, b)| (b.0 - a.0).hypot(b.1 - a.1))
        .sum();

    let count = (perimeter / step).round() as usize;
    (0..=count)
        .map(|index| {
            let mut along = (start + index as f64 / count as f64).fract() * perimeter;
            for &(a, b) in &sides {
                let length = (b.0 - a.0).hypot(b.1 - a.1);
                if along <= length {
                    let t = along / length;
                    let sway = wobble * (index as f64 * 0.9).sin() / length;
                    return (
                        (a.0 + (b.0 - a.0) * t - (b.1 - a.1) * sway).round() as i32,
                        (a.1 + (b.1 - a.1) * t + (b.0 - a.0) * sway).round() as i32,
                    );
                }
                along -= length;
            }
            (corners[0].0.round() as i32, corners[0].1.round() as i32)
        })
        .collect()
}

fn regular(sides: usize, radius: (f64, f64), turn: f64) -> Vec<(f64, f64)> {
    (0..sides)
        .map(|index| {
            let angle = turn + std::f64::consts::TAU * index as f64 / sides as f64;
            (
                200.0 + radius.0 * angle.cos(),
                200.0 + radius.1 * angle.sin(),
            )
        })
        .collect()
}

fn assert_corners_near(points: &[(i32, i32)], corners: &[(f64, f64)], tolerance: f64) {
    assert_eq!(points.len(), 3, "{points:?}");
    for &(x, y) in corners {
        assert!(
            points
                .iter()
                .any(|&(px, py)| (f64::from(px) - x).hypot(f64::from(py) - y) <= tolerance),
            "no vertex near ({x}, {y}) in {points:?}"
        );
    }
}

const EQUILATERAL: [(f64, f64); 3] = [(200.0, 120.0), (270.0, 241.0), (130.0, 241.0)];
const RIGHT: [(f64, f64); 3] = [(100.0, 100.0), (100.0, 260.0), (300.0, 260.0)];
const OBTUSE: [(f64, f64); 3] = [(100.0, 250.0), (400.0, 250.0), (180.0, 170.0)];
const SKINNY: [(f64, f64); 3] = [(100.0, 100.0), (130.0, 300.0), (160.0, 100.0)];

#[test]
fn recognizes_triangles_from_any_start_in_either_direction() {
    for corners in [EQUILATERAL, RIGHT, OBTUSE, SKINNY] {
        for start in [0.0, 0.3, 0.72] {
            for reversed in [false, true] {
                let mut corners = corners;
                if reversed {
                    corners.reverse();
                }
                let path = trace(&corners, start, 4.0, 1.5);

                let points = triangle_points(recognize(&path, 2))
                    .unwrap_or_else(|| panic!("{corners:?} from {start} stayed ink"));

                assert_corners_near(&points, &corners, 5.0);
            }
        }
    }
}

#[test]
fn right_triangle_is_recognized_although_its_box_center_is_on_the_hypotenuse() {
    let path = trace(&RIGHT, 0.3, 4.0, 1.5);

    let points = triangle_points(recognize(&path, 0)).expect("right triangle");

    assert_corners_near(&points, &RIGHT, 2.0);
}

#[test]
fn fitted_edges_meet_at_true_corners_despite_overshoot() {
    // Each corner overshoots 12px along the incoming side and comes back.
    let mut outline = Vec::new();
    for index in 0..3 {
        let previous = EQUILATERAL[(index + 2) % 3];
        let corner = EQUILATERAL[index];
        let length = (corner.0 - previous.0).hypot(corner.1 - previous.1);
        let tip = (
            corner.0 + (corner.0 - previous.0) / length * 12.0,
            corner.1 + (corner.1 - previous.1) / length * 12.0,
        );
        outline.extend([corner, tip, corner]);
    }
    let path = trace(&outline, 0.1, 3.0, 0.5);

    let points = triangle_points(recognize(&path, 2)).expect("overshooting triangle");

    assert_corners_near(&points, &EQUILATERAL, 3.0);
}

#[test]
fn nearly_axis_aligned_sides_are_levelled() {
    let tilted = [(102.0, 100.0), (99.0, 260.0), (300.0, 264.0)];
    let path = trace(&tilted, 0.4, 3.0, 0.0);

    let points = triangle_points(recognize(&path, 2)).expect("tilted right triangle");

    let [top, bottom_left, bottom_right] = [0, 1, 2].map(|corner| {
        *points
            .iter()
            .min_by_key(|&&(x, y)| {
                (f64::from(x) - tilted[corner].0).hypot(f64::from(y) - tilted[corner].1) as i32
            })
            .unwrap()
    });
    assert_eq!(top.0, bottom_left.0, "{points:?}");
    assert_eq!(bottom_left.1, bottom_right.1, "{points:?}");
}

#[test]
fn pixel_staircases_on_dense_diagonals_do_not_inflate_the_outline() {
    // Single-pixel steps make each tilted side up to 41% longer than it is,
    // so with a small overlap past the start the raw path would be too long
    // for a triangle at the most precise level.
    let tilted = regular(3, (80.0, 80.0), 0.3);
    let mut path = trace(&tilted, 0.0, 0.25, 0.0);
    path.extend_from_within(..path.len() * 3 / 100);

    let points = triangle_points(recognize(&path, 0)).expect("densely sampled triangle");

    assert_corners_near(&points, &tilted, 2.0);
}

#[test]
fn round_and_four_sided_strokes_never_become_triangles() {
    let lobed: Vec<_> = (0..64)
        .map(|index| {
            let angle = std::f64::consts::TAU * f64::from(index) / 64.0;
            let wobble = 1.0 + 0.13 * (3.0 * angle).sin();
            (
                200.0 + 50.0 * wobble * angle.cos(),
                80.0 + 40.0 * wobble * angle.sin(),
            )
        })
        .collect();
    let outlines = [
        ("circle", regular(48, (60.0, 60.0), 0.0)),
        ("oval", regular(48, (90.0, 50.0), 0.0)),
        ("lobed oval", lobed),
        (
            "rectangle",
            vec![
                (100.0, 100.0),
                (260.0, 100.0),
                (260.0, 180.0),
                (100.0, 180.0),
            ],
        ),
        ("diamond", regular(4, (70.0, 70.0), 0.0)),
        (
            "kite",
            vec![
                (200.0, 100.0),
                (260.0, 170.0),
                (200.0, 300.0),
                (140.0, 170.0),
            ],
        ),
        (
            "pentagon",
            regular(5, (70.0, 70.0), -std::f64::consts::FRAC_PI_2),
        ),
        ("hexagon", regular(6, (70.0, 70.0), 0.0)),
    ];

    for (name, outline) in outlines {
        for wobble in [0.0, 2.0] {
            let path = trace(&outline, 0.1, 3.0, wobble);
            for sensitivity in 0..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY {
                assert_eq!(
                    triangle_points(recognize(&path, sensitivity)),
                    None,
                    "{name} with wobble {wobble} at sensitivity {sensitivity}"
                );
            }
        }
    }
}
