use super::common::{Value};

use super::error::{InvalidGeometry, SpecViolation};

use super::proto::vector_tile as pbf;
use pbf::mod_Tile as pbf_tile;

use quick_protobuf::{Writer, MessageWrite};

use std::convert::Into;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::Write;

#[derive(Debug, PartialEq, Eq)]
pub struct Tile {
    layers: Vec<Layer>,
}

impl Tile {
    pub fn new(layers: Vec<Layer>) -> Result<Tile, SpecViolation> {
        if layers.is_empty() {
            Err(SpecViolation::EmptyTile)
        } else {
            let mut names = HashSet::with_capacity(layers.len());
            for layer in &layers {
                if !names.insert(&layer.name) {
                    return Err(SpecViolation::IdenticalLayerNames(layer.name.clone()));
                }
            }
            Ok(Tile { layers })
        }
    }

    pub fn write<W: Write>(self, writer: &mut W) {
        let mut pbf_writer = Writer::new(writer);
        let message: pbf::Tile = self.into();
        message.write_message(&mut pbf_writer).unwrap(); // FIXME: is it safe to unwrap?
    }
}

impl<'a> Into<pbf::Tile<'a>> for Tile {
    fn into(self) -> pbf::Tile<'a> {
        pbf::Tile {
            layers: self.layers.into_iter().map(|l| l.into()).collect()
        }
    }
}

impl Eq for pbf_tile::Feature {}

#[derive(Debug, PartialEq, Eq)]
pub struct Layer {
    name: String,
    features: Vec<pbf_tile::Feature>,
    keys: Vec<String>,
    values: Vec<Value>,
    pub extent: u32,
}

impl Layer {
    const VERSION: u32 = 2;

    pub fn new<Name>(name: Name, mut features: Vec<Feature>) -> Result<Layer, SpecViolation> where Name: Into<String> {
        if features.is_empty() {
            Err(SpecViolation::EmptyLayer)
        } else {
            let (keys, values) = Self::encode_features_tags(&mut features)?;
            let features = Self::encode_features(features)?;

            // FIXME: are empty names allowed? Probably not...
            Ok(Layer { name: name.into(), features, keys, values, extent: 4096 })
        }
    }

    fn encode_features_tags(features: &mut [Feature]) -> Result<(Vec<String>, Vec<Value>), SpecViolation> {
        let mut keys = Vec::new();
        let mut key_lookup = HashMap::new(); // FIXME: for a small amount of tags a simple linear search would be enough

        let mut values: Vec<Value> = Vec::new();

        for feature in features {
            let mut key_indices = HashSet::with_capacity(feature.tags.len());

            for (key, value) in &feature.tags {
                let key_idx = match key_lookup.get(key) {
                    Some(idx) => {
                        feature.encoded_tags.push(*idx);
                        *idx
                    },
                    None => {
                        let idx = keys.len() as u32;
                        keys.push(key.clone());
                        feature.encoded_tags.push(idx);
                        key_lookup.insert(key, idx);
                        idx
                    }
                };

                if !key_indices.insert(key_idx) {
                    return Err(SpecViolation::IdenticalAttributeKeys(keys.last().unwrap().clone()));
                }

                match values.iter().position(|v| v == value) {
                    Some(idx) => {
                        feature.encoded_tags.push(idx as u32);
                    },
                    None => {
                        let idx = values.len() as u32;
                        values.push(value.clone());
                        feature.encoded_tags.push(idx);
                    }
                }
            }
            assert!(feature.encoded_tags.len() % 2 == 0);
        }

        Ok((keys, values))
    }

    fn encode_features(features: Vec<Feature>) -> Result<Vec<pbf_tile::Feature>, SpecViolation> {
        let mut encoded_features = Vec::with_capacity(features.len());
        let mut ids = HashSet::with_capacity(features.len());

        for feature in features {
            if let Some(id) = feature.id {
                if !ids.insert(id) {
                    return Err(SpecViolation::IdenticalFeatureIds(id));
                }
            }

            encoded_features.push(pbf_tile::Feature {
                id: feature.id.unwrap_or(0),
                tags: feature.encoded_tags,
                type_pb: feature.geometry.r#type,
                geometry: feature.geometry.commands,
            });
        }

        Ok(encoded_features)
    }
}

impl<'a> Into<pbf_tile::Layer<'a>> for Layer {
    fn into(self) -> pbf_tile::Layer<'a> {
        pbf_tile::Layer {
            version: Layer::VERSION,
            name: Cow::Owned(self.name),
            features: self.features,
            keys: self.keys.iter().map(|s| Cow::Owned(s.clone())).collect(),
            values: self.values.into_iter().map(|v| v.into()).collect(),
            extent: self.extent,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feature {
    pub id: Option<u64>,
    pub tags: Vec<(String, Value)>,
    encoded_tags: Vec<u32>,
    geometry: EncodedGeometry,
}

impl Feature {
    pub fn new(geometry: EncodedGeometry) -> Feature {
        Feature {
            id: None,
            tags: Vec::new(),
            encoded_tags: Vec::new(),
            geometry
        }
    }

    pub fn add_tag<Key>(&mut self, key: Key, value: Value) where Key: Into<String> {
        self.tags.push((key.into(), value));
    }
}

impl<'a> Into<pbf_tile::Value<'a>> for Value {
    fn into(self) -> pbf_tile::Value<'a> {
        let mut value = pbf_tile::Value::default();
        match self {
            Value::String(v) => value.string_value = Some(Cow::Owned(v)),
            Value::Float(v) => value.float_value = Some(v),
            Value::Double(v) => value.double_value = Some(v),
            Value::Int(v) => value.int_value = Some(v),
            Value::UInt(v) => value.uint_value = Some(v),
            Value::SInt(v) => value.sint_value = Some(v),
            Value::Bool(v) => value.bool_value = Some(v),
        }
        value
    }
}

#[derive(Copy, Clone)]
enum Command {
    MoveTo(TileCoord),
    LineTo(TileCoord),
    ClosePath,
}

fn encode_command(command: &Command, count: u32) -> u32 {
    match command {
        Command::MoveTo(_) => (1 & 0x7) | (count << 3),
        Command::LineTo(_) => (2 & 0x7) | (count << 3),
        Command::ClosePath => (7 & 0x7),
    }
}

fn encode_param(param: i32) -> u32 {
    ((param << 1) ^ (param >> 31)) as u32
}

fn diff_to(from: &TileCoord, to: &TileCoord) -> TileCoord {
    (to.0 - from.0, to.1 - from.1)
}

fn encode_geometry(commands: &[Command]) -> Vec<u32> {
    let mut encoded_commands = Vec::with_capacity(commands.len() * 3);
    let mut cursor: TileCoord = (0, 0).into();
    let mut command_buffer = &commands[..];
    command_buffer = &[];

    let mut move_cursor = |to: TileCoord| -> TileCoord {
        let diff = diff_to(&cursor, &to);
        cursor = to;
        diff
    };

    let mut flush_command_buffer = |cb: &mut &[Command], ec: &mut Vec<u32>| {
        if cb.is_empty() {
            return;
        }

        ec.push(encode_command(cb.first().unwrap(), cb.len() as u32));

        for command in cb.iter() {
            match command {
                Command::MoveTo(coord) | Command::LineTo(coord) => {
                    let (x, y) = move_cursor(*coord);
                    ec.push(encode_param(x));
                    ec.push(encode_param(y));
                }
                Command::ClosePath => assert!(false)
            }
        }

        *cb = &[];
    };

    let mut start = None;

    for (idx, command) in commands.iter().enumerate() {
        match command {
            Command::MoveTo(_) => {
                match command_buffer.last() {
                    Some(Command::LineTo(_)) => {
                        flush_command_buffer(&mut command_buffer, &mut encoded_commands); 
                        start = None; 
                    },
                    _ => {}
                }

                if start.is_none() { start = Some(idx); }
                command_buffer = &commands[start.unwrap()..=idx];
            }
            Command::LineTo(_) => {
                match command_buffer.last() {
                    Some(Command::MoveTo(_)) => {
                        flush_command_buffer(&mut command_buffer, &mut encoded_commands);
                        start = None; 
                    },
                    _ => {}
                }

                if start.is_none() { start = Some(idx); }
                command_buffer = &commands[start.unwrap()..=idx];
            }
            Command::ClosePath => encoded_commands.push(encode_command(command, 0))
        }
    }

    flush_command_buffer(&mut command_buffer, &mut encoded_commands);

    encoded_commands
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedGeometry {
    r#type: pbf_tile::GeomType,
    commands: Vec<u32>,
}

pub trait EncodableGeometry {
    fn encode(&self) -> Result<EncodedGeometry, InvalidGeometry>;
}

type TileCoord = (i32, i32);
type TileCoords<'a> = &'a [TileCoord];

pub enum Geometry<'a> {
    Point(TileCoord),
    MultiPoint(TileCoords<'a>),
    Line(TileCoords<'a>),
    MultiLine(&'a [TileCoords<'a>]),
    Polygon(TileCoords<'a>, &'a [TileCoords<'a>]),
}

fn encode_line(line: &[TileCoord], commands: &mut Vec<Command>) -> Result<(), InvalidGeometry> {
    if line.is_empty() {
        return Err(InvalidGeometry::EmptyLineGeometry);
    } else if line.len() < 2 {
        return Err(InvalidGeometry::InvalidLineGeometry);
    }

    for (idx, point) in line.iter().enumerate() {
        if idx == 0 {
            commands.push(Command::MoveTo(*point));
        } else {
            commands.push(Command::LineTo(*point));
        }
    }

    Ok(())
}

fn encode_ring(ring: &[TileCoord], commands: &mut Vec<Command>) -> Result<i32, InvalidGeometry> {
    if ring.is_empty() {
        return Err(InvalidGeometry::EmptyPolygonGeometry);
    } else if ring.len() < 3 {
        return Err(InvalidGeometry::InvalidPolygonGeometry);
    }

    // TODO: maybe ensure that first vertex != last vertex?

    // Check winding of rings

    let mut area = 0;

    for i in 0..ring.len()-1 {
        area += ring[i].0 * ring[i + 1].1;
    }

    area += ring.last().unwrap().0 * ring.first().unwrap().1;

    for i in 0..ring.len()-1 {
        area -= ring[i + 1].0 * ring[i].1;
    }

    area -= ring.first().unwrap().0 * ring.last().unwrap().1;

    if area == 0 {
        return Err(InvalidGeometry::InvalidPolygonGeometry);
    }

    // Serialize geometry

    for (idx, point) in ring.iter().enumerate() {
        if idx == 0 {
            commands.push(Command::MoveTo(*point));
        } else {
            commands.push(Command::LineTo(*point));
        }
    }

    commands.push(Command::ClosePath);

    Ok(area)
}

impl<'a> EncodableGeometry for Geometry<'a> {
    fn encode(&self) -> Result<EncodedGeometry, InvalidGeometry> {
        match self {
            Geometry::Point(point) => {
                let commands = vec![Command::MoveTo(*point)];
        
                Ok(EncodedGeometry {
                    r#type: pbf_tile::GeomType::POINT,
                    commands: encode_geometry(&commands)
                })
            },
            Geometry::MultiPoint(points) => {
                if points.is_empty() {
                    return Err(InvalidGeometry::EmptyPointGeometry);
                }
                
                let mut commands = Vec::with_capacity(points.len());
        
                for point in points.iter() {
                    commands.push(Command::MoveTo(*point));
                }
        
                Ok(EncodedGeometry {
                    r#type: pbf_tile::GeomType::POINT,
                    commands: encode_geometry(&commands)
                })
            },
            Geometry::Line(line) => {
                if line.is_empty() {
                    return Err(InvalidGeometry::EmptyLineGeometry);
                }
        
                let mut commands = Vec::with_capacity(line.len());
        
                encode_line(line, &mut commands)?;
        
                Ok(EncodedGeometry {
                    r#type: pbf_tile::GeomType::LINESTRING,
                    commands: encode_geometry(&commands)
                })
            },
            Geometry::MultiLine(lines) => {
                if lines.is_empty() {
                    return Err(InvalidGeometry::EmptyLineGeometry);
                }

                let command_count: usize = lines.iter().map(|line| line.len()).sum();
        
                let mut commands = Vec::with_capacity(command_count);
        
                for line in lines.iter() {
                    encode_line(line, &mut commands)?;
                }
        
                Ok(EncodedGeometry {
                    r#type: pbf_tile::GeomType::LINESTRING,
                    commands: encode_geometry(&commands)
                })
            },
            Geometry::Polygon(exterior_ring, interior_rings) => {
                if exterior_ring.is_empty() {
                    return Err(InvalidGeometry::EmptyPolygonGeometry);
                }

                let command_count = exterior_ring.len() + interior_rings.iter().map(|ring| ring.len()).sum::<usize>();
        
                let mut commands = Vec::with_capacity(command_count);
        
                let area = encode_ring(exterior_ring, &mut commands)?;

                if area.is_negative() {
                    return Err(InvalidGeometry::InvalidPolygonGeometry);
                }

                for line in interior_rings.iter() {
                    let area = encode_ring(line, &mut commands)?;

                    if area.is_positive() {
                        return Err(InvalidGeometry::InvalidPolygonGeometry);
                    }
                }

                // TODO: check intersection/enclosement
        
                Ok(EncodedGeometry {
                    r#type: pbf_tile::GeomType::POLYGON,
                    commands: encode_geometry(&commands)
                })
            }
        }
    }
}

#[cfg(test)]
mod mvt_writer_test {
    use super::*;
    use quick_protobuf::{MessageRead, BytesReader};

    fn create_test_feature() -> Feature {
        let geometry = EncodedGeometry {
            r#type: pbf_tile::GeomType::UNKNOWN,
            commands: vec![],
        };
        Feature::new(geometry)
    }

    fn create_test_tile() -> Result<Tile, SpecViolation> {
        let geometry = Geometry::Point((2048, 2048));
        let mut poi = Feature::new(geometry.encode().unwrap());
        poi.id = Some(1234);
        poi.add_tag("key", Value::Int(123));
    
        let features = vec![
            poi
        ];
    
        let layers = vec![
            Layer::new("layer", features)?
        ];
    
        Tile::new(layers)
    }

    #[test]
    fn empty_tile() {
        let result = Tile::new(vec![]);
        assert_eq!(result, Err(SpecViolation::EmptyTile));
    }

    #[test]
    fn empty_layer() {
        let result = Layer::new("test", vec![]);
        assert_eq!(result, Err(SpecViolation::EmptyLayer));
    }

    #[test]
    fn identical_layer_names() {
        let features = vec![create_test_feature()];
        let identical_name = "test";
        let layers = vec![
            Layer::new(identical_name, features.clone()).unwrap(),
            Layer::new(identical_name, features).unwrap(),
        ];
        let result = Tile::new(layers);
        assert_eq!(result, Err(SpecViolation::IdenticalLayerNames(identical_name.into())));
    }

    #[test]
    fn identical_feature_ids() {
        let mut feature = create_test_feature();
        feature.id = Some(1);
        let features = vec![feature.clone(), feature];
        let result = Layer::new("test", features);
        assert_eq!(result, Err(SpecViolation::IdenticalFeatureIds(1)));
    }

    #[test]
    fn identical_attribute_keys() {
        let mut feature = create_test_feature();
        let identical_key = "one";
        feature.add_tag(identical_key, Value::Bool(false));
        feature.add_tag(identical_key, Value::Bool(true));

        let features = vec![feature.clone(), feature];
        let result = Layer::new("test", features);
        assert_eq!(result, Err(SpecViolation::IdenticalAttributeKeys(identical_key.into())));
    }

    #[test]
    fn invalid_line() {
        let geometry = Geometry::Line(&[]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::EmptyLineGeometry));

        let geometry = Geometry::Line(&[(0, 0)]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::InvalidLineGeometry));

        let geometry = Geometry::MultiLine(&[&[]]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::EmptyLineGeometry));

        let geometry = Geometry::MultiLine(&[&[(0, 0)]]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::InvalidLineGeometry));
    }

    #[test]
    fn invalid_polygon() {
        let geometry = Geometry::Polygon(&[], &[]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::EmptyPolygonGeometry));

        let geometry = Geometry::Polygon(&[(0, 0), (1, 1), (0, 1)], &[&[]]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::EmptyPolygonGeometry));

        let geometry = Geometry::Polygon(&[(0, 0), (1, 1)], &[&[]]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::InvalidPolygonGeometry));

        let geometry = Geometry::Polygon(&[(0, 0), (1, 1), (0, 1)], &[&[(0, 0), (1, 1)]]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::InvalidPolygonGeometry));

        let geometry = Geometry::Polygon(&[(0, 0), (0, 1), (1, 1)], &[]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::InvalidPolygonGeometry));

        let geometry = Geometry::Polygon(&[(0, 0), (1, 1), (0, 1)], &[&[(0, 0), (1, 1), (0, 1)]]);
        assert_eq!(geometry.encode(), Err(InvalidGeometry::InvalidPolygonGeometry));
    }

    #[test]
    fn read_back() {
        let tile = create_test_tile().unwrap();

        let mut out = Vec::new();
        let mut writer = Writer::new(&mut out);
        let message: pbf::Tile = tile.into();
        message.write_message(&mut writer).unwrap();

        let read_message = {
            let mut reader = BytesReader::from_bytes(&out);
            pbf::Tile::from_reader(&mut reader, &out).unwrap()
        };

        assert_eq!(message, read_message);
    }
}
