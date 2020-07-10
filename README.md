# rosm_mvt

A Rust library for reading and writing v2.1 [Mapbox Vector Tiles](https://docs.mapbox.com/vector-tiles/reference/).

## Writing

The `write` module contains everything needed to serialize vector tiles. The API is designed so that invalid vector tiles (according to the [specification](https://github.com/mapbox/vector-tile-spec)) cannot be created. The only thing not checked currently is that exterior polygon rings enclose interior rings and that interior rings don't intersect each other.

## Reading

Vector tile reading is not available yet.

## Dependencies

- [quick-protobuf](https://github.com/tafia/quick-protobuf) for protobuf parsing

## Similar projects

- [mvt](https://github.com/DougLau/mvt)
