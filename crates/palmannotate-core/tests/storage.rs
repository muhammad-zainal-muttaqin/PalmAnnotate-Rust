use palmannotate_core::{
    AppError, AppSettings, AppStore, Session, Side, Tree, TreeMetadata, TreeStatus, SCHEMA_VERSION,
};
use std::fs;

fn session(id: &str) -> Session {
    Session {
        id: id.into(),
        name: "Field session".into(),
        variety: "DAMIMAS".into(),
        block: "A21B".into(),
        group_key: String::new(),
        side_count: 4,
        auto_id: true,
        next_id: 1,
        operator: "Operator".into(),
        export_uri: "content://field-export".into(),
        created_at: "2026-06-07T00:00:00Z".into(),
        updated_at: "2026-06-07T00:00:00Z".into(),
        trees: vec![],
    }
}

fn tree(id: &str, name: &str) -> Tree {
    Tree {
        version: SCHEMA_VERSION,
        id: id.into(),
        session_id: "session-1".into(),
        tree_name: name.into(),
        split: "field".into(),
        side_count: 4,
        metadata: TreeMetadata::default(),
        sides: (0..4)
            .map(|side_index| Side {
                side_index,
                label: format!("Side {}", side_index + 1),
                image_path: format!("{name}_{}.jpg", side_index + 1),
                image_width: 100,
                image_height: 100,
                depth_path: None,
                depth: None,
                original_bboxes: Vec::new(),
                cache_bust: None,
                bboxes: vec![],
            })
            .collect(),
        confirmed_links: vec![],
        status: TreeStatus::Captured,
    }
}

#[test]
fn save_tree_updates_owning_session_index() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store
        .save_settings(&AppSettings {
            export_uri: "content://field-export".into(),
            export_name: "Field export".into(),
            ..AppSettings::default()
        })
        .unwrap();
    store.save_session(&session("session-1")).unwrap();

    store.save_tree(&tree("tree-1", "TREE_0001")).unwrap();

    let sessions = store.list_sessions().unwrap();
    assert_eq!(sessions[0].trees.len(), 1);
    assert_eq!(sessions[0].trees[0].id, "tree-1");
    assert_eq!(sessions[0].trees[0].tree_name, "TREE_0001");
    assert_eq!(sessions[0].trees[0].tree_id, 1);
    assert_eq!(sessions[0].trees[0].status, TreeStatus::Captured);
    assert_eq!(sessions[0].next_id, 2);
    assert!(store
        .root()
        .join("Output TXT/field/TREE_0001_1.txt")
        .is_file());
}

#[test]
fn settings_persist_and_become_the_single_session_export_folder() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    assert_eq!(store.load_settings().unwrap(), AppSettings::default());

    let saved = store
        .save_settings(&AppSettings {
            export_uri: " content://chosen ".into(),
            export_name: " Documents ".into(),
            ..AppSettings::default()
        })
        .unwrap();
    assert_eq!(saved.export_uri, "content://chosen");
    assert_eq!(saved.export_name, "Documents");

    let mut first = session("session-1");
    first.export_uri = "content://stale".into();
    store.save_session(&first).unwrap();
    assert_eq!(
        store.list_sessions().unwrap()[0].export_uri,
        "content://chosen"
    );

    store
        .save_settings(&AppSettings {
            export_uri: "content://replacement".into(),
            export_name: "USB".into(),
            ..AppSettings::default()
        })
        .unwrap();
    assert_eq!(
        store.list_sessions().unwrap()[0].export_uri,
        "content://replacement"
    );
}

#[test]
fn session_creation_is_blocked_until_global_export_folder_exists() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    let mut pending = session("session-1");
    pending.export_uri.clear();
    assert!(matches!(
        store.save_session(&pending),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn save_tree_rejects_path_traversal_and_missing_session() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();

    let traversal = store.save_tree(&tree("tree-1", "../outside"));
    assert!(matches!(traversal, Err(AppError::Validation(_))));

    let mut orphan = tree("tree-2", "TREE_0002");
    orphan.session_id = "missing".into();
    assert!(matches!(
        store.save_tree(&orphan),
        Err(AppError::NotFound(_))
    ));
}

#[test]
fn delete_tree_updates_session_without_prefix_collision() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();
    store.save_tree(&tree("tree-1", "TREE_1")).unwrap();
    store.save_tree(&tree("tree-10", "TREE_10")).unwrap();

    let dataset = store.root().join("dataset");
    fs::write(dataset.join("TREE_1_1.jpg"), b"one").unwrap();
    fs::write(dataset.join("TREE_10_1.jpg"), b"ten").unwrap();

    store.delete_tree("tree-1").unwrap();

    assert!(!dataset.join("TREE_1_1.jpg").exists());
    assert!(dataset.join("TREE_10_1.jpg").exists());
    assert!(store.load_tree("tree-10").is_ok());
    let sessions = store.list_sessions().unwrap();
    assert_eq!(sessions[0].trees.len(), 1);
    assert_eq!(sessions[0].trees[0].id, "tree-10");
    assert_eq!(sessions[0].next_id, 11);
}

#[test]
fn delete_tree_removes_nested_dataset_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();
    store.save_tree(&tree("tree-1", "TREE_1")).unwrap();
    store.save_tree(&tree("tree-10", "TREE_10")).unwrap();

    let nested = store.root().join("dataset").join("images").join("field");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("TREE_1_1.jpg"), b"one").unwrap();
    fs::write(nested.join("TREE_10_1.jpg"), b"ten").unwrap();

    store.delete_tree("tree-1").unwrap();

    assert!(!nested.join("TREE_1_1.jpg").exists());
    assert!(nested.join("TREE_10_1.jpg").exists());
}

#[test]
fn delete_session_removes_all_owned_trees_and_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();
    store.save_tree(&tree("tree-1", "TREE_1")).unwrap();
    store.save_tree(&tree("tree-2", "TREE_2")).unwrap();
    let nested = store.root().join("dataset/images/field");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("TREE_1_1.jpg"), b"one").unwrap();
    fs::write(nested.join("TREE_2_1.jpg"), b"two").unwrap();

    let sessions = store.delete_session("session-1").unwrap();

    assert!(sessions.is_empty());
    assert!(store.load_tree("tree-1").is_err());
    assert!(store.load_tree("tree-2").is_err());
    assert!(!nested.join("TREE_1_1.jpg").exists());
    assert!(!nested.join("TREE_2_1.jpg").exists());
}

#[test]
fn save_tree_rejects_side_count_mismatch() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();
    let mut invalid = tree("tree-1", "TREE_0001");
    invalid.side_count = 8;

    assert!(matches!(
        store.save_tree(&invalid),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn save_tree_rejects_unsupported_split() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();
    let mut invalid = tree("tree-1", "TREE_0001");
    invalid.split = "../outside".into();

    assert!(matches!(
        store.save_tree(&invalid),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn session_locks_group_and_accepts_legacy_blok_key() {
    let value = serde_json::json!({
        "id": "legacy",
        "name": "Legacy field session",
        "variety": "damimas",
        "blok": "A 21b",
        "sideCount": 4,
        "exportUri": "content://field-export"
    });
    let session: Session = serde_json::from_value(value).unwrap();
    assert_eq!(session.block, "A 21b");

    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session).unwrap();
    let saved = &store.list_sessions().unwrap()[0];
    assert_eq!(saved.group_key, "DAMIMAS__A21B");
    assert_eq!(saved.next_id, 1);
}

#[test]
fn field_session_rejects_unsupported_side_count() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    let mut invalid = session("session-1");
    invalid.side_count = 2;
    assert!(matches!(
        store.save_session(&invalid),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn save_tree_rejects_duplicate_name_from_another_id() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();
    store.save_tree(&tree("tree-1", "TREE_0001")).unwrap();

    assert!(matches!(
        store.save_tree(&tree("tree-2", "TREE_0001")),
        Err(AppError::Conflict(_))
    ));
    assert_eq!(store.load_tree("tree-1").unwrap().tree_name, "TREE_0001");
    assert!(store.load_tree("tree-2").is_err());
}

#[test]
fn tree_metadata_accepts_legacy_blok_key() {
    let metadata: palmannotate_core::TreeMetadata =
        serde_json::from_value(serde_json::json!({"variety": "DAMIMAS", "blok": "A21B"})).unwrap();
    assert_eq!(metadata.block, "A21B");
}

#[test]
fn import_sessions_keeps_existing_and_adds_only_new_ids() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();

    let mut replacement = session("session-1");
    replacement.name = "Must not overwrite".into();
    let incoming = vec![replacement, session("session-2"), session("session-2")];
    let merged = store.import_sessions(incoming).unwrap();

    assert_eq!(merged.len(), 2);
    assert_eq!(
        merged
            .iter()
            .find(|session| session.id == "session-1")
            .unwrap()
            .name,
        "Field session"
    );
    assert!(merged.iter().any(|session| session.id == "session-2"));
}

#[test]
fn sessions_and_trees_are_returned_in_js_display_order() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    let mut older = session("session-1");
    older.updated_at = "2026-06-01T00:00:00Z".into();
    let mut newer = session("session-2");
    newer.updated_at = "2026-06-02T00:00:00Z".into();
    store.save_session(&older).unwrap();
    store.save_session(&newer).unwrap();

    store.save_tree(&tree("tree-10", "TREE_0010")).unwrap();
    store.save_tree(&tree("tree-2", "TREE_0002")).unwrap();

    let sessions = store.list_sessions().unwrap();
    assert_eq!(sessions[0].id, "session-1");
    assert_eq!(
        sessions[0]
            .trees
            .iter()
            .map(|tree| tree.tree_id)
            .collect::<Vec<_>>(),
        [2, 10]
    );
}

#[test]
fn sessions_remember_recent_varieties_and_blocks_most_recent_first() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store
        .save_settings(&AppSettings {
            export_uri: "content://field-export".into(),
            export_name: "Field export".into(),
            ..AppSettings::default()
        })
        .unwrap();

    let mut first = session("session-1");
    first.variety = "Tenera".into();
    first.block = "A01".into();
    store.save_session(&first).unwrap();

    let mut second = session("session-2");
    second.variety = "DAMIMAS".into();
    second.block = "B02".into();
    store.save_session(&second).unwrap();

    let settings = store.load_settings().unwrap();
    assert_eq!(settings.recent_varieties, ["DAMIMAS", "Tenera"]);
    assert_eq!(settings.recent_blocks, ["B02", "A01"]);
}

#[test]
fn delete_tree_removes_logs_exports_and_id_named_snapshots() {
    let temp = tempfile::tempdir().unwrap();
    let store = AppStore::new(temp.path()).unwrap();
    store.save_session(&session("session-1")).unwrap();
    store.save_tree(&tree("tree-1", "TREE_1")).unwrap();

    let export = store.root().join("exports/TREE_1_result.csv");
    let snapshot = store.root().join("snapshots/tree-1-side-1-depth.png");
    let log = store.root().join("dataset/annotlog/field/TREE_1_1.json");
    fs::create_dir_all(export.parent().unwrap()).unwrap();
    fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    fs::create_dir_all(log.parent().unwrap()).unwrap();
    fs::write(&export, b"result").unwrap();
    fs::write(&snapshot, b"preview").unwrap();
    fs::write(&log, b"{}").unwrap();

    store.delete_tree("tree-1").unwrap();

    assert!(!export.exists());
    assert!(!snapshot.exists());
    assert!(!log.exists());
}
