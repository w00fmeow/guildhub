use std::collections::HashMap;

use validator::ValidationErrors;

pub fn validator_errors_to_hashmap(errors: Option<ValidationErrors>) -> HashMap<String, String> {
    let mut error_map = HashMap::new();

    if errors.is_none() {
        return error_map;
    };

    for (field, errors) in errors.unwrap().field_errors().drain() {
        if let Some(error) = errors.first() {
            if let Some(error_message) = error.message.as_deref() {
                error_map.insert(field.to_string(), error_message.to_string());
            }
        }
    }

    error_map
}
