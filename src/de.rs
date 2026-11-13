use serde::de::{self, Deserialize, Deserializer};
use serde_json::{Number, Value};

fn parse_i64(value: &Value) -> Result<i64, String> {
    match value {
        Value::Number(number) => number_to_i64(number),
        Value::String(text) => text
            .trim()
            .parse()
            .map_err(|_| format!("expected an integer, got {text:?}")),
        Value::Null => Ok(0),
        other => Err(format!("expected an integer, got {other}")),
    }
}

fn number_to_i64(number: &Number) -> Result<i64, String> {
    number
        .as_i64()
        .or_else(|| number.as_f64().map(|float| float as i64))
        .ok_or_else(|| format!("expected an integer, got {number}"))
}

fn parse_f64(value: &Value) -> Result<f64, String> {
    match value {
        Value::Number(number) => number
            .as_f64()
            .ok_or_else(|| format!("expected a number, got {number}")),
        Value::String(text) => text
            .trim()
            .parse()
            .map_err(|_| format!("expected a number, got {text:?}")),
        Value::Null => Ok(0.0),
        other => Err(format!("expected a number, got {other}")),
    }
}

pub(crate) fn i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    parse_i64(&Value::deserialize(deserializer)?).map_err(de::Error::custom)
}

pub(crate) fn f64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    parse_f64(&Value::deserialize(deserializer)?).map_err(de::Error::custom)
}

pub(crate) fn opt_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        value => parse_i64(&value).map(Some).map_err(de::Error::custom),
    }
}

pub(crate) fn opt_f64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        value => parse_f64(&value).map(Some).map_err(de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Numbers {
        #[serde(default, deserialize_with = "super::i64")]
        price: i64,
        #[serde(default, deserialize_with = "super::opt_f64")]
        saldo: Option<f64>,
    }

    #[test]
    fn accepts_numbers_and_numeric_strings() {
        let from_numbers: Numbers = serde_json::from_str(r#"{"price":98000,"saldo":1.5}"#).unwrap();
        assert_eq!(from_numbers.price, 98000);
        assert_eq!(from_numbers.saldo, Some(1.5));

        let from_strings: Numbers =
            serde_json::from_str(r#"{"price":"98000","saldo":"1.5"}"#).unwrap();
        assert_eq!(from_strings.price, 98000);
        assert_eq!(from_strings.saldo, Some(1.5));
    }

    #[test]
    fn missing_and_null_values_use_defaults() {
        let missing: Numbers = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.price, 0);
        assert_eq!(missing.saldo, None);

        let null: Numbers = serde_json::from_str(r#"{"price":null,"saldo":null}"#).unwrap();
        assert_eq!(null.price, 0);
        assert_eq!(null.saldo, None);
    }

    #[test]
    fn rejects_non_numeric_text() {
        assert!(serde_json::from_str::<Numbers>(r#"{"price":"murah"}"#).is_err());
    }
}
