// use serde::ser::{SerializeTuple, Serializer};

// pub fn serialize<S: Serializer, const N: usize>(
//     u: &String,
//     serializer: S,
// ) -> Result<S::Ok, S::Error> {
//     let bytes = u.as_bytes();
//     let mut serializer = serializer.serialize_tuple(N)?;
//     for i in 0..N {
//         let v = if i < bytes.len() { bytes[i] } else { 0 };
//         serializer.serialize_element(&v)?;
//     }
//     serializer.end()
// }

// pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<String, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let u: [u8; N] = Deserialize::deserialize(deserializer)?;
//     Ok(String::try_from(u)?)
// }
