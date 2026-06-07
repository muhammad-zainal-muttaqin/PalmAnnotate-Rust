use crate::{build_output_v4, serialize_yolo, AppError, AppResult, BBox, Session, Tree, TreeSummary};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppStore {
    root: PathBuf,
}

impl AppStore {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> AppResult<Self> {
        let root = app_data_dir.into().join("PalmAnnotate");
        let store = Self { root };
        store.ensure_layout()?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn save_export(&self, filename: &str, data: &[u8]) -> AppResult<PathBuf> {
        validate_segment("export filename", filename)?;
        let path = self.root.join("exports").join(filename);
        self.atomic_write(&path, data)?;
        Ok(path)
    }

    pub fn ensure_layout(&self) -> AppResult<()> {
        for directory in [
            "dataset",
            "Output JSON",
            "Output TXT",
            "exports",
            "snapshots",
            "trees",
        ] {
            fs::create_dir_all(self.root.join(directory))?;
        }
        if !self.sessions_path().exists() {
            self.atomic_json(&self.sessions_path(), &Vec::<Session>::new())?;
        }
        Ok(())
    }

    pub fn list_sessions(&self) -> AppResult<Vec<Session>> {
        Ok(serde_json::from_slice(&fs::read(self.sessions_path())?)?)
    }

    pub fn save_session(&self, session: &Session) -> AppResult<Session> {
        if session.export_uri.trim().is_empty() {
            return Err(AppError::Validation(
                "An SAF export folder is required before creating a session.".into(),
            ));
        }
        if !matches!(session.side_count, 4 | 8) {
            return Err(AppError::Validation(
                "Field sessions must use exactly 4 or 8 sides.".into(),
            ));
        }
        let mut session = session.clone();
        session.group_key = crate::group_key_for(&session.variety, &session.block);
        session.next_id = session
            .trees
            .iter()
            .map(|tree| tree.tree_id)
            .max()
            .unwrap_or(0)
            + 1;
        let mut sessions = self.list_sessions()?;
        if let Some(existing) = sessions.iter_mut().find(|item| item.id == session.id) {
            *existing = session.clone();
        } else {
            sessions.push(session.clone());
        }
        self.atomic_json(&self.sessions_path(), &sessions)?;
        Ok(session)
    }

    pub fn save_tree(&self, tree: &Tree) -> AppResult<()> {
        validate_segment("tree id", &tree.id)?;
        validate_segment("tree name", &tree.tree_name)?;
        validate_tree(tree)?;

        let mut sessions = self.list_sessions()?;
        if sessions
            .iter()
            .flat_map(|session| &session.trees)
            .any(|item| item.tree_name == tree.tree_name && item.id != tree.id)
        {
            return Err(AppError::Conflict(format!(
                "Tree name {} already belongs to a different tree; files were not overwritten.",
                tree.tree_name
            )));
        }
        let session = sessions
            .iter_mut()
            .find(|session| session.id == tree.session_id)
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Session {} for tree {} was not found.",
                    tree.session_id, tree.tree_name
                ))
            })?;
        if tree.side_count != session.side_count {
            return Err(AppError::Validation(format!(
                "Tree {} has {} sides but session {} requires {}.",
                tree.tree_name, tree.side_count, session.name, session.side_count
            )));
        }

        self.atomic_json(&self.tree_path(&tree.id), tree)?;
        let output = build_output_v4(tree);
        self.atomic_json(
            &self
                .root
                .join("Output JSON")
                .join(format!("{}.json", tree.tree_name)),
            &output,
        )?;
        for side in &tree.sides {
            let text = serialize_yolo(&side.bboxes, side.image_width, side.image_height);
            self.atomic_write(
                &self.root.join("Output TXT").join(format!(
                    "{}_{}.txt",
                    tree.tree_name,
                    side.side_index + 1
                )),
                text.as_bytes(),
            )?;
        }

        // Annotation behavior log (detector suggestions vs expert final), one file
        // per side. Best-effort — never blocks the save. Mirrors the JS sidecar
        // dataset/annotlog/{split}/{TREE}_{side}.json.
        for side in &tree.sides {
            let log = serde_json::json!({
                "tree_name": tree.tree_name,
                "side_index": side.side_index,
                "side_label": side.label,
                "width": side.image_width,
                "height": side.image_height,
                "generated_at": chrono::Utc::now().to_rfc3339(),
                "suggestion_count": side.original_bboxes.len(),
                "final_count": side.bboxes.len(),
                "suggestions": side.original_bboxes.iter().map(annotlog_box).collect::<Vec<_>>(),
                "final": side.bboxes.iter().map(annotlog_box).collect::<Vec<_>>(),
            });
            let path = self
                .root
                .join("dataset")
                .join("annotlog")
                .join(&tree.split)
                .join(format!("{}_{}.json", tree.tree_name, side.side_index + 1));
            let _ = self.atomic_json(&path, &log);
        }

        let summary = TreeSummary {
            id: tree.id.clone(),
            tree_name: tree.tree_name.clone(),
            tree_id: tree_number(&tree.tree_name),
            side_count: tree.side_count,
            status: tree.status,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Some(existing) = session.trees.iter_mut().find(|item| item.id == tree.id) {
            *existing = summary;
        } else {
            session.trees.push(summary);
        }
        session.next_id = session
            .trees
            .iter()
            .map(|tree| tree.tree_id)
            .max()
            .unwrap_or(0)
            + 1;
        session.updated_at = chrono::Utc::now().to_rfc3339();
        self.atomic_json(&self.sessions_path(), &sessions)
    }

    pub fn load_tree(&self, id: &str) -> AppResult<Tree> {
        validate_segment("tree id", id)?;
        let path = self.tree_path(id);
        if !path.exists() {
            return Err(AppError::NotFound(format!("Tree {id} was not found.")));
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    pub fn delete_tree(&self, id: &str) -> AppResult<()> {
        validate_segment("tree id", id)?;
        let tree = self.load_tree(id)?;
        let tree_name = tree.tree_name.as_str();
        validate_segment("tree name", tree_name)?;
        for path in [
            self.tree_path(id),
            self.root
                .join("Output JSON")
                .join(format!("{tree_name}.json")),
        ] {
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
        for directory in ["dataset", "Output TXT", "snapshots"] {
            let path = self.root.join(directory);
            if path.exists() {
                remove_tree_artifacts(&path, tree_name)?;
            }
        }

        let mut sessions = self.list_sessions()?;
        for session in &mut sessions {
            session.trees.retain(|summary| summary.id != id);
            session.next_id = session
                .trees
                .iter()
                .map(|tree| tree.tree_id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        self.atomic_json(&self.sessions_path(), &sessions)
    }

    pub fn import_sessions(&self, incoming: Vec<Session>) -> AppResult<Vec<Session>> {
        let mut sessions = self.list_sessions()?;
        for session in incoming {
            if sessions.iter().any(|current| current.id == session.id) {
                return Err(AppError::Conflict(format!(
                    "Session id {} already exists; import does not overwrite.",
                    session.id
                )));
            }
            sessions.push(session);
        }
        self.atomic_json(&self.sessions_path(), &sessions)?;
        Ok(sessions)
    }

    pub fn delete_session(&self, id: &str) -> AppResult<Vec<Session>> {
        validate_segment("session id", id)?;
        let sessions = self.list_sessions()?;
        let session = sessions
            .iter()
            .find(|session| session.id == id)
            .ok_or_else(|| AppError::NotFound(format!("Session {id} was not found.")))?
            .clone();
        for tree in session.trees {
            self.delete_tree(&tree.id)?;
        }
        let mut sessions = self.list_sessions()?;
        sessions.retain(|session| session.id != id);
        self.atomic_json(&self.sessions_path(), &sessions)?;
        Ok(sessions)
    }

    fn sessions_path(&self) -> PathBuf {
        self.root.join("sessions.json")
    }

    fn tree_path(&self, id: &str) -> PathBuf {
        self.root.join("trees").join(format!("{id}.json"))
    }

    fn atomic_json<T: serde::Serialize>(&self, path: &Path, value: &T) -> AppResult<()> {
        self.atomic_write(path, &serde_json::to_vec_pretty(value)?)
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, data)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(temporary, path)?;
        Ok(())
    }
}

/// Shape one bbox for the annotation behavior log, matching the JS `_annotLogShape`:
/// `{ id, classId, className, bbox_pixel:[x1,y1,x2,y2 rounded], score? }`.
fn annotlog_box(bbox: &BBox) -> serde_json::Value {
    let mut value = serde_json::json!({
        "id": bbox.id,
        "classId": bbox.class_id,
        "className": bbox.class_name,
        "bbox_pixel": [
            bbox.x1.round() as i64,
            bbox.y1.round() as i64,
            bbox.x2.round() as i64,
            bbox.y2.round() as i64,
        ],
    });
    if let Some(score) = bbox.confidence {
        value["score"] = serde_json::json!((f64::from(score) * 10000.0).round() / 10000.0);
    }
    value
}

fn validate_segment(label: &str, value: &str) -> AppResult<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ' '))
        })
    {
        return Err(AppError::Validation(format!(
            "Invalid {label}: {value:?}. Use letters, numbers, spaces, dash, underscore, or dot."
        )));
    }
    Ok(())
}

fn validate_tree(tree: &Tree) -> AppResult<()> {
    if tree.version != crate::SCHEMA_VERSION {
        return Err(AppError::Validation(format!(
            "Tree {} uses unsupported schema version {}.",
            tree.tree_name, tree.version
        )));
    }
    if tree.side_count != tree.sides.len() || !matches!(tree.side_count, 4 | 8) {
        return Err(AppError::Validation(format!(
            "Tree {} must contain exactly its declared 4 or 8 sides.",
            tree.tree_name
        )));
    }
    let mut endpoints = HashSet::new();
    for (expected_index, side) in tree.sides.iter().enumerate() {
        if side.side_index != expected_index {
            return Err(AppError::Validation(format!(
                "Tree {} side indices must be sequential from zero.",
                tree.tree_name
            )));
        }
        if side.image_width == 0 || side.image_height == 0 {
            return Err(AppError::Validation(format!(
                "Tree {} side {} has invalid image dimensions.",
                tree.tree_name,
                side.side_index + 1
            )));
        }
        validate_relative(&side.image_path)?;
        if let Some(path) = &side.depth_path {
            validate_relative(path)?;
        }
        let mut ids = HashSet::new();
        for bbox in &side.bboxes {
            if !ids.insert(bbox.id.as_str()) {
                return Err(AppError::Validation(format!(
                    "Tree {} side {} contains duplicate bbox id {}.",
                    tree.tree_name,
                    side.side_index + 1,
                    bbox.id
                )));
            }
            if !(-1..=3).contains(&bbox.class_id)
                || ![bbox.x1, bbox.y1, bbox.x2, bbox.y2]
                    .into_iter()
                    .all(f64::is_finite)
                || bbox.x1 < 0.0
                || bbox.y1 < 0.0
                || bbox.x2 <= bbox.x1
                || bbox.y2 <= bbox.y1
                || bbox.x2 > f64::from(side.image_width)
                || bbox.y2 > f64::from(side.image_height)
            {
                return Err(AppError::Validation(format!(
                    "Tree {} bbox {} has invalid class or geometry.",
                    tree.tree_name, bbox.id
                )));
            }
            endpoints.insert((side.side_index, bbox.id.as_str()));
        }
    }
    let mut links = HashSet::new();
    for link in &tree.confirmed_links {
        let distance = link.side_a.abs_diff(link.side_b);
        let adjacent = tree.side_count == 2
            || distance == 1
            || (tree.side_count > 2 && distance == tree.side_count - 1);
        if !adjacent
            || !endpoints.contains(&(link.side_a, link.bbox_id_a.as_str()))
            || !endpoints.contains(&(link.side_b, link.bbox_id_b.as_str()))
        {
            return Err(AppError::Validation(format!(
                "Tree {} contains a stale or non-adjacent confirmed link.",
                tree.tree_name
            )));
        }
        let left = (link.side_a, link.bbox_id_a.as_str());
        let right = (link.side_b, link.bbox_id_b.as_str());
        let key = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        if !links.insert(key) {
            return Err(AppError::Validation(format!(
                "Tree {} contains a duplicate confirmed link.",
                tree.tree_name
            )));
        }
    }
    Ok(())
}

fn validate_relative(value: &str) -> AppResult<()> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(AppError::Validation(format!(
            "Tree file path must stay relative to the dataset: {value:?}."
        )));
    }
    Ok(())
}

fn belongs_to_tree(file_name: &str, tree_name: &str) -> bool {
    file_name == tree_name
        || file_name.strip_prefix(tree_name).is_some_and(|suffix| {
            suffix.starts_with('_') || suffix.starts_with('.') || suffix.starts_with('-')
        })
}

fn remove_tree_artifacts(directory: &Path, tree_name: &str) -> AppResult<()> {
    for item in fs::read_dir(directory)? {
        let item = item?;
        let path = item.path();
        if item.file_type()?.is_dir() {
            remove_tree_artifacts(&path, tree_name)?;
            if fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(path)?;
            }
        } else if belongs_to_tree(&item.file_name().to_string_lossy(), tree_name) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn tree_number(tree_name: &str) -> usize {
    tree_name
        .rsplit('_')
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}
