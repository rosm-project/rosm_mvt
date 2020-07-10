use std::error;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum SpecViolation {
    EmptyTile,
    IdenticalLayerNames(String),
    EmptyLayer,
    IdenticalFeatureIds(u64),
    IdenticalAttributeKeys(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum InvalidGeometry {
    EmptyPointGeometry,
    EmptyLineGeometry,
    InvalidLineGeometry,
    EmptyPolygonGeometry,
    InvalidPolygonGeometry,
}

impl fmt::Display for InvalidGeometry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let description = match self {
            InvalidGeometry::EmptyPointGeometry => "Empty point geometry",
            InvalidGeometry::EmptyLineGeometry => "Empty line geometry",
            InvalidGeometry::InvalidLineGeometry => "A line should contain a least two points",
            InvalidGeometry::EmptyPolygonGeometry => "Empty polygon geometry",
            InvalidGeometry::InvalidPolygonGeometry => "A polygon should contain a least three points",
        };
        write!(f, "{}", description)
    }
}

impl error::Error for InvalidGeometry {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl fmt::Display for SpecViolation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut offending_value = None;
        let description = match self {
            SpecViolation::EmptyTile => ("4.1. Layers", "A Vector Tile SHOULD contain at least one layer.", ),
            SpecViolation::IdenticalLayerNames(name) => {
                offending_value = Some(name.clone());
                ("4.1. Layers", "A Vector Tile MUST NOT contain two or more layers whose name values are byte-for-byte identical.")
            },
            SpecViolation::EmptyLayer => ("4.1. Layers", "A layer SHOULD contain at least one feature."),
            SpecViolation::IdenticalFeatureIds(id) => {
                offending_value = Some(format!("{}", id));
                ("4.2. Features", "If a feature has an id field, the value of the id SHOULD be unique among the features of the parent layer.")
            },
            SpecViolation::IdenticalAttributeKeys(key) => {
                offending_value = Some(key.clone());
                ("4.4. Feature Attributes", "Every key index MUST be unique within that feature such that no other attribute pair within that feature has the same key index.")
            }
        };
        let (section, rule) = description;
        write!(f, "{}: {}{}", section, rule, if let Some(v) = offending_value { format!(" Offending value: {}", v) } else { String::new() })
    }
}

impl error::Error for SpecViolation {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}
