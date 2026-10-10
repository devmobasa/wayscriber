//! Floating stroke coordinates accept legacy JSON integers and validate both directions.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn valid(x: f64, y: f64) -> bool {
    [x, y]
        .into_iter()
        .all(|v| v.is_finite() && v >= f64::from(i32::MIN) && v <= f64::from(i32::MAX))
}

pub(super) fn serialize<S: Serializer>(
    points: &[(f64, f64)],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if points.iter().any(|&(x, y)| !valid(x, y)) {
        return Err(serde::ser::Error::custom("invalid stroke coordinate"));
    }
    points.serialize(serializer)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<(f64, f64)>, D::Error> {
    let points = Vec::<(f64, f64)>::deserialize(deserializer)?;
    if points.iter().any(|&(x, y)| !valid(x, y)) {
        return Err(serde::de::Error::custom("invalid stroke coordinate"));
    }
    Ok(points)
}

pub(super) fn serialize_pressure<S: Serializer>(
    points: &[(f64, f64, f32)],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if points
        .iter()
        .any(|&(x, y, width)| !valid(x, y) || !width.is_finite())
    {
        return Err(serde::ser::Error::custom("invalid pressure stroke sample"));
    }
    points.serialize(serializer)
}

pub(super) fn deserialize_pressure<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<(f64, f64, f32)>, D::Error> {
    let points = Vec::<(f64, f64, f32)>::deserialize(deserializer)?;
    if points
        .iter()
        .any(|&(x, y, width)| !valid(x, y) || !width.is_finite())
    {
        return Err(serde::de::Error::custom("invalid pressure stroke sample"));
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use crate::draw::{Shape, color::BLACK};

    #[test]
    fn fractional_ink_survives_selection_transforms_and_serialization() {
        for shape in [
            Shape::Freehand {
                points: vec![(10.25, 20.5), (13.75, 22.125)],
                color: BLACK,
                thick: 2.0,
            },
            Shape::FreehandPressure {
                points: vec![(10.25, 20.5, 2.0), (13.75, 22.125, 4.0)],
                color: BLACK,
            },
            Shape::MarkerStroke {
                points: vec![(10.25, 20.5), (13.75, 22.125)],
                color: BLACK,
                thick: 2.0,
            },
        ] {
            let mut transformed = shape.scaled(1.5, 0.5, 0.0, 0.0);
            transformed.translate(3, -2);
            let expected = [(18.375, 8.25), (23.625, 9.0625)];
            match &transformed {
                Shape::Freehand { points, .. } | Shape::MarkerStroke { points, .. } => {
                    assert_eq!(points, &expected)
                }
                Shape::FreehandPressure { points, .. } => {
                    assert_eq!(points, &[(18.375, 8.25, 2.0), (23.625, 9.0625, 4.0)])
                }
                _ => unreachable!(),
            }
            let encoded = serde_json::to_vec(&transformed).unwrap();
            assert_eq!(
                serde_json::from_slice::<Shape>(&encoded).unwrap(),
                transformed
            );
            let bounds = transformed.bounding_box().unwrap();
            assert!(bounds.contains(18, 8));
            assert!(bounds.contains(24, 10));
        }
    }

    #[test]
    fn invalid_stroke_coordinates_and_pressure_cannot_be_saved_or_loaded() {
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e30] {
            for shape in [
                Shape::Freehand {
                    points: vec![(x, 1.0)],
                    color: BLACK,
                    thick: 2.0,
                },
                Shape::MarkerStroke {
                    points: vec![(x, 1.0)],
                    color: BLACK,
                    thick: 2.0,
                },
                Shape::FreehandPressure {
                    points: vec![(x, 1.0, 2.0)],
                    color: BLACK,
                },
            ] {
                assert!(serde_json::to_vec(&shape).is_err());
            }
        }
        let shape = Shape::FreehandPressure {
            points: vec![(1.0, 2.0, f32::NAN)],
            color: BLACK,
        };
        assert!(serde_json::to_vec(&shape).is_err());
        let shape = Shape::Freehand {
            points: vec![(1.0, 2.0)],
            color: BLACK,
            thick: 2.0,
        };
        let mut value = serde_json::to_value(shape).unwrap();
        value["Freehand"]["points"][0][0] = serde_json::json!(1e30);
        assert!(serde_json::from_value::<Shape>(value).is_err());
    }
}
