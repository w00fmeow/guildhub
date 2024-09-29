use std::fmt::Debug;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

pub fn omit_values<Input>(input: Input, to_omit: Value) -> Result<Value>
where
    Input: Serialize + Debug,
{
    let mut original_value = serde_json::to_value(input)?;

    if let Some(object) = original_value.as_object_mut() {
        let keys_to_remove: Vec<String> = object
            .iter()
            .filter_map(|(key, value)| {
                if value == &to_omit {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            object.remove(&key);
        }
    }

    return Ok(original_value);
}
