use std::{collections::BTreeMap, fmt::Display};

pub type KV = BTreeMap<String, Value>;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    // formatによってはデータが詰められてしまう場合がある
    // IntについてはDeserialize後の扱いやすさのためにまずは64bitのみで実装
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Text(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::I64(x) => write!(f, "{}", x),
            Value::U64(x) => write!(f, "{}", x),
            Value::F32(x) => write!(f, "{:.6}", x),
            Value::F64(x) => write!(f, "{:.6}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::Text(x) => write!(f, "\"{}\"", x),
            Value::Bytes(x) => write!(f, "bytes({})", x.len()),
            Value::Array(x) => write!(f, "vec({}, len={})", x[0], x.len()),
        }
    }
}

impl serde::Serialize for Value {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::I64(v) => serializer.serialize_i64(*v),
            Value::U64(v) => serializer.serialize_u64(*v),
            Value::F32(v) => serializer.serialize_f32(*v),
            Value::F64(v) => serializer.serialize_f64(*v),
            Value::Text(v) => serializer.serialize_str(v),
            Value::Bool(v) => serializer.serialize_bool(*v),
            Value::Bytes(v) => serializer.serialize_bytes(v),
            Value::Array(v) => v.serialize(serializer),
            Value::Null => serializer.serialize_unit(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;
        use std::fmt;
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = crate::kv::Value;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("any valid CBOR value")
            }

            #[inline]
            fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::F32(v))
            }

            #[inline]
            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::F64(v))
            }

            #[inline]
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::U64(v))
            }

            #[inline]
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::I64(v))
            }

            #[inline]
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_string(String::from(value))
            }

            #[inline]
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::Text(value))
            }

            #[inline]
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_byte_buf(v.to_owned())
            }

            #[inline]
            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::Bytes(v))
            }

            #[inline]
            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::Bool(v))
            }

            #[inline]
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_unit()
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Value::Null)
            }

            #[inline]
            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let mut vec = Vec::new();

                while let Some(elem) = visitor.next_element()? {
                    vec.push(elem);
                }

                Ok(Value::Array(vec))
            }
        }
        deserializer.deserialize_any(ValueVisitor)
    }
}

// Primitive type from
macro_rules! impl_from {
    ($for_type:ty) => {
        impl From<$for_type> for Value {
            fn from(_: $for_type) -> Self {
                Self::Null
            }
        }
    };
    ($variant:path, $for_type:ty) => {
        impl From<$for_type> for Value {
            fn from(v: $for_type) -> Self {
                $variant(v.into())
            }
        }
    };
}

impl_from!(Self::I64, i8);
impl_from!(Self::I64, i16);
impl_from!(Self::I64, i32);
impl_from!(Self::I64, i64);
impl_from!(Self::U64, u8);
impl_from!(Self::U64, u16);
impl_from!(Self::U64, u32);
impl_from!(Self::U64, u64);
impl_from!(Self::F32, f32);
impl_from!(Self::F64, f64);
impl_from!(Self::Bool, bool);
impl_from!(Self::Text, &str);
impl_from!(Self::Bytes, &[u8]);
impl_from!(Self::Text, String);
impl_from!(Self::Bytes, Vec<u8>);
impl_from!(());

// [u8]以外はArrayとして解釈する
macro_rules! vec_owned_from {
    ($for_type:ty) => {
        impl From<Vec<$for_type>> for Value {
            fn from(v: Vec<$for_type>) -> Self {
                Self::Array(v.into_iter().map(|x| x.into()).collect())
            }
        }
    };
}

vec_owned_from!(bool);
vec_owned_from!(i8);
vec_owned_from!(i16);
vec_owned_from!(i32);
vec_owned_from!(i64);
vec_owned_from!(u16);
vec_owned_from!(u32);
vec_owned_from!(u64);
vec_owned_from!(f32);
vec_owned_from!(f64);
vec_owned_from!(String);
vec_owned_from!(&str);

#[cfg(test)]
mod tests {
    use crate::kv::{Value, KV};
    use float_cmp::approx_eq;
    use itertools::izip;

    #[test]
    fn test_integer() {
        let kv = kv_zip!(
            "i8",
            1_i8,
            "i16",
            42_i16,
            "i32",
            72_i32,
            "i64",
            i64::MIN,
            "u8",
            0_u8,
            "u16",
            138_u16,
            "u32",
            2568_u32,
            "u64",
            3313_u64
        );

        // serialize
        let buf = serde_cbor::to_vec(&kv).unwrap();
        assert_eq!(buf[0], 0xa8); // 8 length map
        assert_eq!(buf.len(), 54); // 順不同なので長さのみ見る
                                   // deserialize
        let data: KV = serde_cbor::from_slice(buf.as_ref()).unwrap();
        if let Some(Value::I64(x)) = data.get("i64") {
            assert_eq!(*x, i64::MIN);
        } else {
            unreachable!();
        }
        if let Some(Value::U64(x)) = data.get("u64") {
            assert_eq!(*x, 3313_u64);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_float() {
        let testdata_f32 = -1.558_751_7_f32;
        let kv = kv_zip!("f32", testdata_f32, "f64", f64::MAX);

        // serialize
        let buf = serde_cbor::to_vec(&kv).unwrap();
        assert_eq!(buf[0], 0xa2);
        assert_eq!(buf.len(), 23);

        // deserialize
        let data: KV = serde_cbor::from_slice(buf.as_ref()).unwrap();
        if let Some(Value::F32(x)) = data.get("f32") {
            assert!(approx_eq!(f32, *x, testdata_f32));
        } else {
            unreachable!();
        }
        if let Some(Value::F64(x)) = data.get("f64") {
            assert!(approx_eq!(f64, *x, f64::MAX));
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_bool_null() {
        let kv = kv_zip!("t", true, "f", false, "unit", ());

        // serialize
        let buf = serde_cbor::to_vec(&kv).unwrap();
        assert_eq!(buf[0], 0xa3);
        assert_eq!(buf.len(), 13);

        // deserialize
        let data: KV = serde_cbor::from_slice(buf.as_ref()).unwrap();
        if let Some(Value::Bool(x)) = data.get("t") {
            assert_eq!(x, &true);
        } else {
            unreachable!();
        }
        if let Some(Value::Bool(x)) = data.get("f") {
            assert_eq!(x, &false);
        } else {
            unreachable!();
        }
        if let Some(x) = data.get("unit") {
            match x {
                Value::Null => {}
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_string() {
        let testdata_str = "static ligetime str";
        let testdata_string = format!("owned String {}", 123);
        let kv = kv_zip!("str", testdata_str, "String", testdata_string.clone());

        // serialize
        let buf = serde_cbor::to_vec(&kv).unwrap();
        assert_eq!(buf[0], 0xa2);
        assert_eq!(buf.len(), 49);

        // deserialize
        let data: KV = serde_cbor::from_slice(buf.as_ref()).unwrap();
        if let Some(Value::Text(x)) = data.get("str") {
            assert_eq!(x, testdata_str);
        } else {
            unreachable!();
        }
        if let Some(Value::Text(x)) = data.get("String") {
            assert_eq!(x, &testdata_string);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_bytes() {
        let testdata = vec![64_u8; 512];
        let kv = kv_zip!(
            "byte_slice",
            &testdata[0..32],
            "byte_array",
            testdata.clone(),
            "byte_owned",
            vec![16_u8; 1024]
        );

        // serialize
        let buf = serde_cbor::to_vec(&kv).unwrap();
        assert_eq!(buf[0], 0xa3);

        // deserialize
        let data: KV = serde_cbor::from_slice(buf.as_ref()).unwrap();
        if let Some(Value::Bytes(x)) = data.get("byte_slice") {
            assert_eq!(x.len(), 32);
            assert_eq!(x[0..32], testdata[0..32]);
        } else {
            unreachable!();
        }
        if let Some(Value::Bytes(x)) = data.get("byte_array") {
            assert_eq!(x.len(), 512);
            assert_eq!(x, &testdata);
        } else {
            unreachable!();
        }
        if let Some(Value::Bytes(x)) = data.get("byte_owned") {
            assert_eq!(x.len(), 1024);
            assert_eq!(x[0], 16);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_array() {
        let data_string: Vec<String> = vec!["hello", "world"]
            .into_iter()
            .map(|x| x.to_string())
            .collect();
        let kv = kv_zip!(
            "str_vec",
            vec!["hello", "world"],
            "String_vec",
            data_string,
            "float_vec",
            vec![1.0_f32, 8.6, 778.68],
            "uint_vec",
            vec![0_u16, 5, 8]
        );

        // serialize
        let buf = serde_cbor::to_vec(&kv).unwrap();
        assert_eq!(buf[0], 0xa4);
        assert_eq!(buf.len(), 83);

        // deserialize
        let data: KV = serde_cbor::from_slice(buf.as_ref()).unwrap();
        if let Some(Value::Array(x)) = data.get("float_vec") {
            assert_eq!(x.len(), 3);
            if let Some(Value::Array(v)) = kv.get("float_vec") {
                izip!(x, v).into_iter().for_each(|(actual, expect)| {
                    if let Value::F32(actual) = actual {
                        if let Value::F32(expect) = expect {
                            assert!(approx_eq!(f32, *actual, *expect));
                        } else {
                            unreachable!();
                        }
                    } else {
                        unreachable!();
                    }
                });
            }
        } else {
            unreachable!();
        }
        if let Some(Value::Array(x)) = data.get("uint_vec") {
            assert_eq!(x.len(), 3);
            // u16 -> u64になっているので解釈に注意
            if let Some(Value::Array(v)) = kv.get("uint_vec") {
                izip!(x, v).into_iter().for_each(|(actual, expect)| {
                    if let Value::U64(actual) = actual {
                        if let Value::U64(expect) = expect {
                            assert_eq!(actual, expect);
                        } else {
                            unreachable!();
                        }
                    } else {
                        unreachable!();
                    }
                });
            }
        } else {
            unreachable!();
        }
    }
}
