//! Configuration as datoms (ADR-INTERFACE-005, WP2).
//!
//! Config lives in the store as `:config/*` attributes, not in config files.
//! The store IS the configuration.
//!
//! ```text
//! Config = π_{:config/*}(Store)
//! get(key) = resolve(Store, :config/{key})
//! set(key, val) = transact(Store, [[:config/{key}, val]])
//! ```

use crate::datom::{latest_assert, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;
use std::collections::HashMap;

/// Get a config value from the store.
///
/// Resolves `:config/value` for the entity matching `:config/key` = `key`.
pub fn get_config(store: &Store, key: &str) -> Option<String> {
    let key_attr = Attribute::from_keyword(":config/key");
    let val_attr = Attribute::from_keyword(":config/value");

    // Find the entity with :config/key = key
    let entity = store
        .attribute_datoms(&key_attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .find(|d| matches!(&d.value, Value::String(k) if k == key))
        .map(|d| d.entity)?;

    // Get :config/value for that entity (most recent assertion wins via LWW)
    let datoms = store.entity_datoms(entity);
    latest_assert(&datoms, &val_attr).and_then(|d| match &d.value {
        Value::String(v) => Some(v.clone()),
        _ => None,
    })
}

/// Get a config value, or return the default if not set.
pub fn get_config_or(store: &Store, key: &str, default: &str) -> String {
    get_config(store, key).unwrap_or_else(|| default.to_string())
}

/// Get all config key-value pairs from the store.
pub fn all_config(store: &Store) -> Vec<(String, String, String)> {
    let key_attr = Attribute::from_keyword(":config/key");
    let val_attr = Attribute::from_keyword(":config/value");
    let scope_attr = Attribute::from_keyword(":config/scope");

    let mut results = Vec::new();

    for datom in store.attribute_datoms(&key_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let key = match &datom.value {
            Value::String(k) => k.clone(),
            _ => continue,
        };
        let entity = datom.entity;

        let ent_datoms = store.entity_datoms(entity);

        let value = latest_assert(&ent_datoms, &val_attr)
            .and_then(|d| match &d.value {
                Value::String(v) => Some(v.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let scope = latest_assert(&ent_datoms, &scope_attr)
            .and_then(|d| match &d.value {
                Value::Keyword(k) => Some(k.clone()),
                _ => None,
            })
            .unwrap_or_else(|| ":config.scope/project".to_string());

        results.push((key, value, scope));
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

/// Build datoms for setting a config value.
///
/// Creates an entity with `:config/key`, `:config/value`, and `:config/scope`.
pub fn set_config_datoms(key: &str, value: &str, scope: &str, tx: TxId) -> Vec<Datom> {
    let ident = format!(":config/{}", key.replace('.', "-"));
    let entity = EntityId::from_ident(&ident);

    vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":config/key"),
            Value::String(key.to_string()),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":config/value"),
            Value::String(value.to_string()),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":config/scope"),
            Value::Keyword(format!(":config.scope/{scope}")),
            tx,
            Op::Assert,
        ),
    ]
}

/// Default config values. Returned for keys not set in the store.
pub fn defaults() -> HashMap<String, (&'static str, &'static str)> {
    let mut m = HashMap::new();
    m.insert(
        "output.default-mode".into(),
        ("agent", "Default output mode when no flag/env/TTY"),
    );
    m.insert(
        "output.token-budget".into(),
        ("300", "Max tokens for agent-mode output"),
    );
    m.insert(
        "harvest.auto-commit".into(),
        ("false", "Auto-commit harvest results"),
    );
    m.insert(
        "harvest.confidence-floor".into(),
        ("0.3", "Min confidence for harvest candidates"),
    );
    m.insert(
        "session.auto-start".into(),
        ("true", "Auto-start session on first command"),
    );
    m.insert(
        "trace.source-dirs".into(),
        ("crates/", "Directories to scan for spec refs"),
    );
    m.insert(
        "git.enabled".into(),
        ("auto", "Git integration: auto/always/never"),
    );
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;
    use crate::schema;
    use crate::store::Store;

    fn make_store_with_config() -> Store {
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);
        let mut all_datoms = schema::full_schema_datoms(tx);

        let tx2 = TxId::new(2, 0, agent);
        all_datoms.extend(set_config_datoms(
            "output.default-mode",
            "json",
            "project",
            tx2,
        ));
        all_datoms.extend(set_config_datoms("git.enabled", "auto", "global", tx2));

        Store::from_datoms(all_datoms.into_iter().collect())
    }

    #[test]
    fn get_config_returns_value() {
        let store = make_store_with_config();
        assert_eq!(
            get_config(&store, "output.default-mode"),
            Some("json".into())
        );
        assert_eq!(get_config(&store, "git.enabled"), Some("auto".into()));
    }

    #[test]
    fn get_config_returns_none_for_missing() {
        let store = make_store_with_config();
        assert_eq!(get_config(&store, "nonexistent"), None);
    }

    #[test]
    fn get_config_or_returns_default() {
        let store = make_store_with_config();
        assert_eq!(get_config_or(&store, "nonexistent", "fallback"), "fallback");
        assert_eq!(get_config_or(&store, "git.enabled", "never"), "auto");
    }

    #[test]
    fn all_config_returns_sorted() {
        let store = make_store_with_config();
        let all = all_config(&store);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "git.enabled");
        assert_eq!(all[1].0, "output.default-mode");
    }

    #[test]
    fn defaults_has_all_keys() {
        let d = defaults();
        assert!(d.contains_key("output.default-mode"));
        assert!(d.contains_key("git.enabled"));
        assert!(d.contains_key("harvest.confidence-floor"));
    }
}
