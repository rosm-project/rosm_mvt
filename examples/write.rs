use rosm_mvt::common::Value;
use rosm_mvt::write::{Tile, Layer, Feature, Geometry, EncodableGeometry};

use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut point = Feature::new(Geometry::Point((2048, 2048)).encode()?);
    point.id = Some(1234);

    let mut line = Feature::new(Geometry::Line(&[
        (10, 20), 
        (30, 40),
    ]).encode()?);
    line.add_tag("length", Value::Float(4.0));

    let features = vec![point, line];

    let layer = Layer::new("example", features)?;

    let layers = vec![layer];

    let tile = Tile::new(layers)?;

    let mut file = File::create("example.mvt")?;
    tile.write(&mut file);

    Ok(())
}
