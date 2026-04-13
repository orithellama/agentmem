use tempfile::tempdir;

use agent_hashmap::config::Config;
use agent_hashmap::store::locking;
use agent_hashmap::store::Store;
use agent_hashmap::types::{Key, Namespace, ProjectName, Value};

fn open_test_store() -> (tempfile::TempDir, Config, Store) {
    let dir = tempdir().expect("failed to create temporary directory");

    let config = Config::for_project_root(
        ProjectName::new("demo-project").expect("valid project name"),
        dir.path(),
    )
    .expect("failed to build config");

    let store = Store::open(config.clone()).expect("failed to open store");

    (dir, config, store)
}

#[test]
fn store_starts_empty() {
    let (_dir, _config, store) = open_test_store();

    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
    assert!(store.entries().is_empty());
}

#[test]
fn set_then_get_returns_value() {
    let (_dir, _config, mut store) = open_test_store();

    let key = Key::new("agent/claude/current_task").expect("valid key");
    let value = Value::new("Review auth middleware").expect("valid value");

    let previous = store
        .set(key.clone(), value.clone())
        .expect("set should succeed");

    assert!(previous.is_none());

    let retrieved = store.get(&key).expect("value should exist");
    assert_eq!(retrieved, &value);
}

#[test]
fn set_overwrites_existing_value_and_returns_previous() {
    let (_dir, _config, mut store) = open_test_store();

    let key = Key::new("agent/claude/current_task").expect("valid key");
    let first = Value::new("Review PR #41").expect("valid value");
    let second = Value::new("Review PR #42").expect("valid value");

    let previous = store
        .set(key.clone(), first.clone())
        .expect("first set should succeed");
    assert!(previous.is_none());

    let previous = store
        .set(key.clone(), second.clone())
        .expect("second set should succeed");

    assert_eq!(previous.as_ref(), Some(&first));
    assert_eq!(store.get(&key), Some(&second));
}

#[test]
fn delete_existing_key_removes_value() {
    let (_dir, _config, mut store) = open_test_store();

    let key = Key::new("agent/codex/current_task").expect("valid key");
    let value = Value::new("Refactor persistence layer").expect("valid value");

    store
        .set(key.clone(), value.clone())
        .expect("set should succeed");

    let removed = store.delete(&key).expect("value should be removed");
    assert_eq!(removed, value);
    assert!(store.get(&key).is_none());
    assert!(!store.contains(&key));
}

#[test]
fn delete_missing_key_returns_none() {
    let (_dir, _config, mut store) = open_test_store();

    let key = Key::new("agent/gemini/current_task").expect("valid key");

    assert!(store.delete(&key).is_none());
}

#[test]
fn namespace_listing_returns_only_matching_entries() {
    let (_dir, _config, mut store) = open_test_store();

    store
        .set(
            Key::new("agent/claude/current_task").expect("valid key"),
            Value::new("Audit auth flow").expect("valid value"),
        )
        .expect("set should succeed");

    store
        .set(
            Key::new("agent/claude/context/summary").expect("valid key"),
            Value::new("Need safer token rotation").expect("valid value"),
        )
        .expect("set should succeed");

    store
        .set(
            Key::new("agent/codex/current_task").expect("valid key"),
            Value::new("Write tests").expect("valid value"),
        )
        .expect("set should succeed");

    let namespace = Namespace::new("agent/claude").expect("valid namespace");
    let entries = store.list_namespace(&namespace);

    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .all(|entry| entry.key.as_str().starts_with("agent/claude/")));
}

#[test]
fn delete_namespace_removes_only_matching_entries() {
    let (_dir, _config, mut store) = open_test_store();

    store
        .set(
            Key::new("agent/claude/current_task").expect("valid key"),
            Value::new("A").expect("valid value"),
        )
        .expect("set should succeed");

    store
        .set(
            Key::new("agent/claude/context/summary").expect("valid key"),
            Value::new("B").expect("valid value"),
        )
        .expect("set should succeed");

    store
        .set(
            Key::new("agent/codex/current_task").expect("valid key"),
            Value::new("C").expect("valid value"),
        )
        .expect("set should succeed");

    let removed = store.delete_namespace(&Namespace::new("agent/claude").expect("valid namespace"));

    assert_eq!(removed, 2);
    assert_eq!(store.len(), 1);
    assert!(store
        .get(&Key::new("agent/codex/current_task").expect("valid key"))
        .is_some());
}

#[test]
fn entries_are_returned_in_stable_sorted_order() {
    let (_dir, _config, mut store) = open_test_store();

    for (key, value) in [
        ("agent/codex/current_task", "c"),
        ("agent/claude/current_task", "a"),
        ("agent/claude/context", "b"),
    ] {
        store
            .set(
                Key::new(key).expect("valid key"),
                Value::new(value).expect("valid value"),
            )
            .expect("set should succeed");
    }

    let entries = store.entries();
    let keys: Vec<String> = entries
        .iter()
        .map(|entry| entry.key.as_str().to_owned())
        .collect();

    assert_eq!(
        keys,
        vec![
            "agent/claude/context",
            "agent/claude/current_task",
            "agent/codex/current_task",
        ]
    );
}

#[test]
fn flush_then_reopen_roundtrips_data() {
    let (_dir, config, mut store) = open_test_store();

    let key = Key::new("project/demo/root").expect("valid key");
    let value = Value::new("/workspace/demo").expect("valid value");

    store
        .set(key.clone(), value.clone())
        .expect("set should succeed");

    store.flush().expect("flush should succeed");

    let reopened = Store::open(config).expect("reopen should succeed");

    assert_eq!(reopened.get(&key), Some(&value));
}

#[test]
fn reload_discards_unsaved_in_memory_changes() {
    let (_dir, config, mut store) = open_test_store();

    let key = Key::new("agent/claude/current_task").expect("valid key");

    store
        .set(
            key.clone(),
            Value::new("Initial value").expect("valid value"),
        )
        .expect("set should succeed");

    store.flush().expect("flush should succeed");

    store
        .set(
            key.clone(),
            Value::new("Unsaved change").expect("valid value"),
        )
        .expect("set should succeed");

    store.reload().expect("reload should succeed");

    assert_eq!(
        store.get(&key).expect("key should exist").as_str(),
        "Initial value"
    );

    let reopened = Store::open(config).expect("reopen should succeed");
    assert_eq!(
        reopened.get(&key).expect("key should exist").as_str(),
        "Initial value"
    );
}

#[test]
fn open_locked_acquires_lock_and_release_lock_clears_it() {
    let (dir, config, _store) = open_test_store();

    let mut locked = Store::open_locked(config).expect("open locked store");

    assert!(locked.is_locked());
    assert!(locking::is_locked(locked.path()));

    locked.release_lock().expect("release lock");
    assert!(!locking::is_locked(
        &dir.path().join(".agentmem/store.json")
    ));
}

#[test]
fn clear_then_flush_persists_empty_store() {
    let (_dir, config, mut store) = open_test_store();

    store
        .set(
            Key::new("agent/claude/current_task").expect("valid key"),
            Value::new("task").expect("valid value"),
        )
        .expect("set should succeed");
    store.flush().expect("flush should succeed");

    store.clear();
    store.flush().expect("flush after clear");

    let reopened = Store::open(config).expect("reopen");
    assert!(reopened.is_empty());
}
