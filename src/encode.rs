use super::Value;
use Marker;

/// Write the MessagePack value out to a Vector
pub fn write_value(buf: &mut Vec<u8>, val: &Value) {
    match *val {
        Value::Null => {

        Value::Boolean(b),
        Value::Integer(i),
        Value::String(s),
        Value::F32(f),
        Value::F64(f),
        Value::Binary(bin),
        Value::Array(array),
        Value::Object(obj),
        Value::Hash(hash),
        Value::Identity(id),
        Value::Lockbox(lock),
        Value::Timestamp(time),
    }
}
