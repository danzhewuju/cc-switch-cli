use crate::config::{atomic_write, home_dir};
use crate::error::AppError;
use crate::settings::get_hermes_override_dir;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;

fn default_config() -> Value {
    json!({
        "custom_providers": []
    })
}

pub fn get_hermes_dir() -> PathBuf {
    if let Some(override_dir) = get_hermes_override_dir() {
        return override_dir;
    }

    home_dir()
        .map(|home| home.join(".hermes"))
        .unwrap_or_else(|| PathBuf::from(".hermes"))
}

pub fn get_hermes_config_path() -> PathBuf {
    get_hermes_dir().join("config.yaml")
}

pub fn read_hermes_config_source() -> Result<Option<String>, AppError> {
    let path = get_hermes_config_path();
    if !path.exists() {
        return Ok(None);
    }

    fs::read_to_string(&path)
        .map(Some)
        .map_err(|e| AppError::io(&path, e))
}

pub fn write_hermes_config_source(source: &str) -> Result<(), AppError> {
    let path = get_hermes_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    atomic_write(&path, source.as_bytes())
}

pub fn read_hermes_config() -> Result<Value, AppError> {
    let path = get_hermes_config_path();
    if !path.exists() {
        return Ok(default_config());
    }

    let source = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&source)
        .map_err(|e| AppError::Config(format!("Failed to parse Hermes config as YAML: {e}")))?;
    serde_json::to_value(yaml_value).map_err(|e| AppError::JsonSerialize { source: e })
}

fn write_hermes_config(config: &Value) -> Result<(), AppError> {
    let yaml_value = serde_yaml::to_value(config)
        .map_err(|e| AppError::Config(format!("Failed to convert Hermes config to YAML: {e}")))?;
    let yaml = serde_yaml::to_string(&yaml_value)
        .map_err(|e| AppError::Config(format!("Failed to serialize Hermes config as YAML: {e}")))?;
    write_hermes_config_source(&yaml)
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .expect("value should be object after normalization")
}

fn provider_id_from_value(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    for key in ["name", "id", "provider"] {
        let candidate = object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(id) = candidate {
            return Some(id.to_string());
        }
    }
    None
}

fn primary_model_id_from_value(value: &Value) -> Option<String> {
    value
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("models")
                .and_then(Value::as_object)
                .and_then(|models| models.keys().next().cloned())
        })
        .or_else(|| {
            value
                .get("models")
                .and_then(Value::as_array)
                .and_then(|models| models.first())
                .and_then(|model| model.get("id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

fn provider_matches_model(provider: &Value, model_id: &str) -> bool {
    let model_id = model_id.trim();
    if model_id.is_empty() {
        return false;
    }

    provider
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value == model_id)
        || provider
            .get("models")
            .and_then(Value::as_object)
            .is_some_and(|models| models.contains_key(model_id))
        || provider
            .get("models")
            .and_then(Value::as_array)
            .is_some_and(|models| {
                models.iter().any(|model| {
                    model
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .is_some_and(|value| value == model_id)
                })
            })
}

fn normalize_provider_value(id: &str, provider: Value) -> Result<Value, AppError> {
    let mut provider = provider;
    let object = provider.as_object_mut().ok_or_else(|| {
        AppError::localized(
            "provider.hermes.settings.not_object",
            "Hermes 配置必须是 JSON 对象",
            "Hermes configuration must be a JSON object",
        )
    })?;

    let has_identifier = ["name", "id", "provider"].iter().any(|key| {
        object
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    });
    if !has_identifier {
        object.insert("name".to_string(), Value::String(id.to_string()));
    }

    Ok(provider)
}

pub fn get_current_provider_id() -> Result<Option<String>, AppError> {
    let config = read_hermes_config()?;
    let Some(model) = config.get("model").and_then(Value::as_object) else {
        return Ok(None);
    };

    let provider_ref = model
        .get("provider")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();

    if let Some(custom_id) = provider_ref.strip_prefix("custom:") {
        let custom_id = custom_id.trim();
        if !custom_id.is_empty() {
            return Ok(Some(custom_id.to_string()));
        }
    }

    if provider_ref == "custom" {
        let default_model = model
            .get("default")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(default_model) = default_model {
            for (id, provider) in get_providers()? {
                if provider_matches_model(&provider, default_model) {
                    return Ok(Some(id));
                }
            }
        }
    }

    Ok(None)
}

pub fn get_providers() -> Result<IndexMap<String, Value>, AppError> {
    let config = read_hermes_config()?;
    let custom_providers = config
        .get("custom_providers")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));

    let mut providers = IndexMap::new();
    match custom_providers {
        Value::Array(entries) => {
            for entry in entries {
                if let Some(id) = provider_id_from_value(&entry) {
                    providers.insert(id, entry);
                }
            }
        }
        Value::Object(entries) => {
            for (id, entry) in entries {
                providers.insert(id, entry);
            }
        }
        _ => {}
    }

    Ok(providers)
}

pub fn set_current_provider(id: &str, provider: &Value) -> Result<(), AppError> {
    let mut config = read_hermes_config()?;
    let root = ensure_object(&mut config);
    let model = root
        .entry("model".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let model = ensure_object(model);

    model.insert(
        "provider".to_string(),
        Value::String(format!("custom:{id}")),
    );
    if let Some(model_id) = primary_model_id_from_value(provider) {
        model.insert("default".to_string(), Value::String(model_id));
    }

    write_hermes_config(&config)
}

pub fn set_provider(id: &str, provider: Value) -> Result<(), AppError> {
    let mut config = read_hermes_config()?;
    let root = ensure_object(&mut config);
    let normalized = normalize_provider_value(id, provider)?;

    match root
        .entry("custom_providers".to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
    {
        Value::Array(entries) => {
            let mut replaced = false;
            for entry in entries.iter_mut() {
                if provider_id_from_value(entry).as_deref() == Some(id) {
                    *entry = normalized.clone();
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                entries.push(normalized);
            }
        }
        Value::Object(entries) => {
            entries.insert(id.to_string(), normalized);
        }
        slot => {
            *slot = Value::Array(vec![normalized]);
        }
    }

    write_hermes_config(&config)
}

pub fn remove_provider(id: &str) -> Result<(), AppError> {
    let mut config = read_hermes_config()?;
    let root = ensure_object(&mut config);

    if let Some(custom_providers) = root.get_mut("custom_providers") {
        match custom_providers {
            Value::Array(entries) => {
                entries.retain(|entry| provider_id_from_value(entry).as_deref() != Some(id));
            }
            Value::Object(entries) => {
                entries.remove(id);
            }
            _ => {
                *custom_providers = Value::Array(Vec::new());
            }
        }
    }

    write_hermes_config(&config)
}
