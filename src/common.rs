#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Float(f32),
    Double(f64),
    Int(i64),
    UInt(u64),
    SInt(i64),
    Bool(bool),
}

impl Eq for Value {
    // TODO: binary equality for f32/f64
}
