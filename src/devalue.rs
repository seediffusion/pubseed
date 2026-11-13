//! Decoder for the "devalue" flat-array encoding SvelteKit uses in
//! `__data.json` responses: element 0 is the root, and every object value and
//! array element is an index into the same array.

use serde_json::{Map, Value};

/// Merges the data of every node (layout first, then page) into one object.
pub fn page_data(body: &Value) -> Map<String, Value> {
    let mut merged = Map::new();
    let nodes = body.get("nodes").and_then(Value::as_array);
    for node in nodes.into_iter().flatten() {
        if let Some(data) = node.get("data").and_then(Value::as_array)
            && let Value::Object(map) = hydrate(data, 0, 0)
        {
            merged.extend(map);
        }
    }
    merged
}

fn hydrate(data: &[Value], index: i64, depth: u32) -> Value {
    // Negative indices are devalue's undefined/NaN/hole markers.
    if index < 0 || depth > 64 {
        return Value::Null;
    }
    match data.get(index as usize) {
        Some(Value::Object(map)) => Value::Object(
            map.iter()
                .map(|(key, v)| {
                    (
                        key.clone(),
                        hydrate(data, v.as_i64().unwrap_or(-1), depth + 1),
                    )
                })
                .collect(),
        ),
        Some(Value::Array(items)) => match items.first() {
            // A leading string is a type tag: ["Date", "2026-..."] and friends.
            Some(Value::String(tag)) if tag == "Date" => {
                items.get(1).cloned().unwrap_or(Value::Null)
            }
            Some(Value::String(_)) => Value::Null,
            _ => Value::Array(
                items
                    .iter()
                    .map(|v| hydrate(data, v.as_i64().unwrap_or(-1), depth + 1))
                    .collect(),
            ),
        },
        Some(other) => other.clone(),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_layout_and_page_nodes() {
        let body: Value = serde_json::from_str(
            r#"{"type":"data","nodes":[
                {"type":"data","data":[{"user":1,"isAdmin":2},null,false],"uses":{}},
                {"type":"data","data":[{"audios":1,"page":4},[2],{"id":3,"gone":-1},"abc",1],"uses":{}}
            ]}"#,
        )
        .unwrap();
        let data = page_data(&body);
        assert_eq!(data["user"], Value::Null);
        assert_eq!(data["isAdmin"], Value::Bool(false));
        assert_eq!(data["page"], 1);
        assert_eq!(data["audios"][0]["id"], "abc");
        assert_eq!(data["audios"][0]["gone"], Value::Null);
    }
}
