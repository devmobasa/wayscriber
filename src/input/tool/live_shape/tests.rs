use crate::domain::{BoardGrid, BoardGridKind};
use crate::draw::{BLACK, PolygonKind, Shape};

fn recognize(points: &[(i32, i32)], sensitivity: u8) -> Option<Shape> {
    recognize_on(points, BoardGrid::default(), sensitivity)
}

fn recognize_on(points: &[(i32, i32)], grid: BoardGrid, sensitivity: u8) -> Option<Shape> {
    super::recognize(points, BLACK, 3.0, false, grid, sensitivity)
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

#[test]
fn open_arcs_spirals_repeated_loops_and_departing_tails_stay_ink() {
    let arc = |turns: f64, growth: f64| -> Vec<(i32, i32)> {
        (0..=(turns * 96.0).round() as usize)
            .map(|step| {
                let angle = std::f64::consts::TAU * step as f64 / 96.0;
                let radius = 60.0 + growth * step as f64 / 96.0;
                (
                    (200.0 + radius * angle.cos()).round() as i32,
                    (200.0 + radius * angle.sin()).round() as i32,
                )
            })
            .collect()
    };
    let mut tail = arc(1.1, 0.0);
    tail.extend([(280, 260), (310, 280), (340, 300)]);
    let mut retraced = arc(1.0, 0.0);
    retraced.extend(arc(1.0, 0.0).into_iter().rev());

    for (name, path) in [
        ("open arc", arc(0.7, 0.0)),
        ("spiral", arc(1.4, 60.0)),
        ("two loops", arc(2.0, 0.0)),
        ("retraced loop", retraced),
        ("departing tail", tail),
    ] {
        for level in 0..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY {
            assert!(recognize(&path, level).is_none(), "{name} at level {level}");
        }
    }
}

#[test]
fn overlapping_rectangles_do_not_become_ellipses() {
    let sharp = vec![
        (100.0, 100.0),
        (300.0, 100.0),
        (300.0, 220.0),
        (100.0, 220.0),
    ];
    let mut rounded = Vec::new();
    for (cx, cy, start) in [
        (280.0, 120.0, -90.0_f64),
        (280.0, 200.0, 0.0),
        (120.0, 200.0, 90.0),
        (120.0, 120.0, 180.0),
    ] {
        for step in 0..=12 {
            let angle = (start + 90.0 * step as f64 / 12.0).to_radians();
            rounded.push((cx + 20.0 * angle.cos(), cy + 20.0 * angle.sin()));
        }
    }

    for outline in [&sharp, &rounded] {
        for step in [1.0, 4.0] {
            for extra in [0.15, 0.3, 0.45] {
                for reversed in [false, true] {
                    let mut path = trace(outline, 0.1, step, 0.0);
                    if reversed {
                        path.reverse();
                    }
                    let lap = path.len() - 1;
                    path.extend_from_within(1..=(lap as f64 * extra).round() as usize);
                    for level in [3, 4] {
                        assert!(
                            !matches!(recognize(&path, level), Some(Shape::Ellipse { .. })),
                            "rectangle, step {step}, overlap {extra}, reversed {reversed}, level {level}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn overlap_allows_small_radius_drift_but_rejects_spirals() {
    for turns in [1.2, 1.4, 1.5] {
        for (growth, expected_ellipse) in [(3.0, true), (10.0, false), (15.0, false), (20.0, false)]
        {
            let path: Vec<_> = (0..=(turns * 360.0) as usize)
                .map(|step| {
                    let fraction = step as f64 / 360.0;
                    let angle = std::f64::consts::TAU * fraction;
                    let radius = 60.0 + growth * fraction / turns;
                    (
                        (200.0 + radius * angle.cos()).round() as i32,
                        (200.0 + radius * angle.sin()).round() as i32,
                    )
                })
                .collect();
            for level in 0..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY {
                let shape = recognize(&path, level);
                if expected_ellipse {
                    assert!(
                        matches!(shape, Some(Shape::Ellipse { .. })),
                        "small drift +{growth}px, {turns} turns, level {level}"
                    );
                } else {
                    assert!(
                        shape.is_none(),
                        "spiral +{growth}px, {turns} turns, level {level}"
                    );
                }
            }
        }
    }
}

#[test]
fn overlap_rejects_inward_curls() {
    for inward in [0.2, 0.3] {
        let path: Vec<_> = (0..=468)
            .map(|step| {
                let fraction = step as f64 / 360.0;
                let angle = std::f64::consts::TAU * fraction;
                let radius = 60.0 * (1.0 - inward * ((fraction - 1.1) / 0.2).clamp(0.0, 1.0));
                (
                    (200.0 + radius * angle.cos()).round() as i32,
                    (200.0 + radius * angle.sin()).round() as i32,
                )
            })
            .collect();
        for level in 0..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY {
            assert!(
                recognize(&path, level).is_none(),
                "inward curl {inward}, level {level}"
            );
        }
    }
}

fn circle_with_offset_overlap(
    radius: f64,
    extra: f64,
    offset: f64,
    direction: f64,
) -> Vec<(i32, i32)> {
    (0..=(360.0 * (1.0 + extra)).round() as usize)
        .map(|step| {
            let angle = direction * std::f64::consts::TAU * step as f64 / 360.0;
            // The second pass differs from the first, even at the same angle.
            let radius = radius + if step > 360 { offset } else { 0.0 };
            (
                (600.0 + radius * angle.cos()).round() as i32,
                (600.0 + radius * angle.sin()).round() as i32,
            )
        })
        .collect()
}

#[test]
fn short_offset_overshoots_preserve_closed_circle_recognition() {
    for radius in [60.0, 150.0, 300.0] {
        let offsets: &[f64] = if radius == 60.0 {
            &[-8.0, 8.0]
        } else {
            &[-15.0, -8.0, 8.0, 15.0]
        };
        for &offset in offsets {
            for direction in [-1.0, 1.0] {
                for (extra, min_level) in [(0.03, 0), (0.08, 3)] {
                    let path = circle_with_offset_overlap(radius, extra, offset, direction);
                    for level in min_level..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY {
                        assert!(
                            matches!(recognize(&path, level), Some(Shape::Ellipse { .. })),
                            "radius {radius}, offset {offset}, extra {extra}, direction {direction}, level {level}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn long_offset_overlaps_allow_large_circle_retracing_but_reject_small_circle_drift() {
    for (radius, offset, expected_ellipse) in [
        (60.0, -8.0, false),
        (60.0, 8.0, false),
        (150.0, -8.0, true),
        (150.0, 8.0, true),
        (150.0, -15.0, false),
        (150.0, 15.0, false),
        (300.0, -15.0, true),
        (300.0, 15.0, true),
    ] {
        for direction in [-1.0, 1.0] {
            let path = circle_with_offset_overlap(radius, 0.3, offset, direction);
            for level in 0..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY {
                let shape = recognize(&path, level);
                if expected_ellipse {
                    assert!(
                        matches!(shape, Some(Shape::Ellipse { .. })),
                        "radius {radius}, offset {offset}, direction {direction}, level {level}"
                    );
                } else {
                    assert!(
                        shape.is_none(),
                        "radius {radius}, offset {offset}, direction {direction}, level {level}"
                    );
                }
            }
        }
    }
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

#[test]
fn rectangles_snap_their_edges_to_nearby_cartesian_lines() {
    let grid = BoardGrid::new(BoardGridKind::Cartesian, 40);
    let near = [(42.0, 38.0), (158.0, 41.0), (157.0, 122.0), (41.0, 119.0)];
    let far = [(20.0, 20.0), (140.0, 20.0), (140.0, 100.0), (20.0, 100.0)];

    let snapped = recognize_on(&trace(&near, 0.1, 3.0, 0.0), grid, 2);
    let untouched = recognize_on(&trace(&far, 0.1, 3.0, 0.0), grid, 2);

    assert!(
        matches!(
            snapped,
            Some(Shape::Rect {
                x: 40,
                y: 40,
                w: 120,
                h: 80,
                ..
            })
        ),
        "{snapped:?}"
    );
    assert!(
        matches!(
            untouched,
            Some(Shape::Rect {
                x: 20,
                y: 20,
                w: 120,
                h: 80,
                ..
            })
        ),
        "edges 20px from every line stay put: {untouched:?}"
    );
}

#[test]
fn triangle_corners_snap_to_isometric_lattice_points() {
    // Lattice points (i, j) sit at x = i·√3/2·s and y = j·s, shifted down half
    // a spacing in odd columns. These corners are a few pixels off i = 2, 6
    // (y = 160) and i = 4 (y = 40).
    let drawn = [(72.0, 157.0), (205.0, 163.0), (140.0, 43.0)];

    for kind in [BoardGridKind::Isometric, BoardGridKind::IsometricDots] {
        let grid = BoardGrid::new(kind, 40);
        let points = triangle_points(recognize_on(&trace(&drawn, 0.1, 3.0, 0.0), grid, 2))
            .unwrap_or_else(|| panic!("triangle on {kind:?}"));

        let mut sorted = points.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![(69, 160), (139, 40), (208, 160)], "{kind:?}");
    }
}

/// Quick mouse rectangles traced from a screenshot of strokes Shape Pen left
/// as ink at the old default: one tilted, one with a rounded corner, and one
/// skewed with a side leaning about 17 degrees.
const QUICK_RECTANGLES: [&[(f64, f64)]; 3] = [
    &[(38.0, 122.0), (382.0, 150.0), (401.0, 256.0), (39.0, 240.0)],
    &[
        (567.0, 234.0),
        (845.0, 226.0),
        (850.0, 386.0),
        (622.0, 395.0),
        (597.0, 350.0),
    ],
    &[
        (206.0, 343.0),
        (457.0, 350.0),
        (492.0, 466.0),
        (252.0, 485.0),
    ],
];

#[test]
fn quick_rectangles_with_leaning_sides_are_rectangles_by_default() {
    let default = crate::config::DEFAULT_SHAPE_RECOGNITION_SENSITIVITY;

    for corners in QUICK_RECTANGLES {
        let path = trace(corners, 0.05, 2.0, 1.0);

        let shape = recognize(&path, default);

        let Some(Shape::Rect { x, y, w, h, .. }) = shape else {
            panic!("{corners:?} became {shape:?}");
        };
        // Each side lands on its average position, inside the drawn extremes.
        let (xs, ys): (Vec<f64>, Vec<f64>) = corners.iter().copied().unzip();
        let min = |values: &[f64]| values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |values: &[f64]| values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(f64::from(x) >= min(&xs) - 2.0 && f64::from(x + w) <= max(&xs) + 2.0);
        assert!(f64::from(y) >= min(&ys) - 2.0 && f64::from(y + h) <= max(&ys) + 2.0);
    }
}

#[test]
fn leaning_quadrilaterals_need_more_sensitivity_the_more_they_lean() {
    // Left and right sides lean 21 degrees: a trapezoid, not a quick
    // rectangle, until the most forgiving levels.
    let trapezoid = [
        (100.0, 250.0),
        (300.0, 250.0),
        (250.0, 120.0),
        (150.0, 120.0),
    ];
    let path = trace(&trapezoid, 0.1, 3.0, 0.0);

    let kinds: Vec<_> = (0..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY)
        .map(|level| recognize(&path, level).map(|shape| shape.kind_name()))
        .collect();

    assert_eq!(
        kinds,
        [None, None, None, Some("Rectangle"), Some("Rectangle")]
    );
}

#[test]
fn diamonds_kites_and_round_strokes_never_become_rectangles() {
    let outlines = [
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
        ("circle", regular(48, (60.0, 60.0), 0.0)),
        ("oval", regular(48, (90.0, 50.0), 0.0)),
    ];

    for (name, outline) in outlines {
        for wobble in [0.0, 2.0] {
            let path = trace(&outline, 0.1, 3.0, wobble);
            for level in 0..=crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY {
                assert!(
                    !matches!(recognize(&path, level), Some(Shape::Rect { .. })),
                    "{name} with wobble {wobble} at level {level}"
                );
            }
        }
    }
}
