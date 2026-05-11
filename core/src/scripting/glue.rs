//! Glue code for Rune integration.

use rune::Any;

#[derive(Any, Debug, Clone)]
pub struct ScraperConfig {
    #[rune(get, set)]
    pub user_agent: String,
}

impl Default for ScraperConfig {
    fn default() -> Self {
        Self {
            user_agent: "Soromantic/1.0".to_string(),
        }
    }
}
use anyhow::{Result, anyhow};
use rune::runtime::Object;
use rune::{Value, from_value};

/// # Errors
/// Returns error if conversion fails.
pub fn into_object(value: Value) -> Result<Object> {
    from_value(value).map_err(|e| anyhow!(e))
}

/// # Errors
/// Returns error if field is missing or conversion fails.
pub fn extract_string(obj: &Object, key: &str) -> Result<String> {
    let val = obj
        .get_value::<_, Value>(key)
        .into_result()?
        .ok_or_else(|| anyhow!("Missing field {key}"))?;
    from_value(val).map_err(|e| anyhow!("Field {key} error: {e}"))
}

/// # Errors
/// Returns error if field access fails.
pub fn extract_opt_string(obj: &Object, key: &str) -> Result<Option<String>> {
    let Some(val) = obj.get_value::<_, Value>(key).into_result()? else {
        return Ok(None);
    };

    // Try Option<String>
    from_value(val.clone()).map_or_else(
        |_| {
            // Try String directly
            from_value(val).map_or_else(|_| Ok(None), |s| Ok(Some(s)))
        },
        Ok,
    )
}

/// # Errors
/// Returns error if field access fails.
pub fn extract_opt_int(obj: &Object, key: &str) -> Result<Option<i64>> {
    let Some(val) = obj.get_value::<_, Value>(key).into_result()? else {
        return Ok(None);
    };

    from_value(val.clone()).map_or_else(
        |_| from_value(val).map_or_else(|_| Ok(None), |i| Ok(Some(i))),
        Ok,
    )
}

/// # Errors
/// Returns error if field is missing or conversion fails.
// Return Vec<Value> to avoid Shared issues
pub fn extract_list(obj: &Object, key: &str) -> Result<Vec<Value>> {
    let val = obj
        .get_value::<_, Value>(key)
        .into_result()?
        .ok_or_else(|| anyhow!("Missing field {key}"))?;
    from_value(val).map_err(|e| anyhow!("Field {key} error: {e}"))
}
