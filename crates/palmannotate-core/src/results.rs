use crate::{BBox, Tree};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cluster {
    pub members: Vec<(usize, usize)>,
    pub dominant_class: String,
    pub class_mismatch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputationResult {
    pub unique_count: usize,
    pub raw_count: usize,
    pub linked_count: usize,
    pub unassigned_count: usize,
    pub class_counts: BTreeMap<String, usize>,
    pub side_counts: BTreeMap<String, usize>,
    pub clusters: Vec<Cluster>,
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parent[value] != value {
            self.parent[value] = self.find(self.parent[value]);
        }
        self.parent[value]
    }

    fn union(&mut self, a: usize, b: usize) -> bool {
        let a = self.find(a);
        let b = self.find(b);
        if a == b {
            false
        } else {
            self.parent[b] = a;
            true
        }
    }
}

pub fn compute_results(tree: &Tree) -> ComputationResult {
    let mut lookup = HashMap::new();
    let mut flat: Vec<(usize, usize, &BBox)> = Vec::new();
    let mut side_counts = BTreeMap::new();
    for side in &tree.sides {
        side_counts.insert(side.label.clone(), side.bboxes.len());
        for (box_index, bbox) in side.bboxes.iter().enumerate() {
            lookup.insert((side.side_index, bbox.id.as_str()), flat.len());
            flat.push((side.side_index, box_index, bbox));
        }
    }

    let mut uf = UnionFind::new(flat.len());
    let mut linked_count = 0;
    for link in &tree.confirmed_links {
        let Some(&a) = lookup.get(&(link.side_a, link.bbox_id_a.as_str())) else {
            continue;
        };
        let Some(&b) = lookup.get(&(link.side_b, link.bbox_id_b.as_str())) else {
            continue;
        };
        if uf.union(a, b) {
            linked_count += 1;
        }
    }

    let mut groups: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for (index, &(side_index, box_index, _)) in flat.iter().enumerate() {
        groups
            .entry(uf.find(index))
            .or_default()
            .push((side_index, box_index));
    }

    let mut class_counts = ["B1", "B2", "B3", "B4", "other"]
        .into_iter()
        .map(|name| (name.to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    let mut clusters = Vec::new();
    for members in groups.into_values() {
        let mut votes: HashMap<&str, usize> = HashMap::new();
        for &(side_index, box_index) in &members {
            let bbox = &tree.sides[side_index].bboxes[box_index];
            *votes.entry(&bbox.class_name).or_default() += 1;
        }
        let dominant_class = votes
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(name, _)| (*name).to_string())
            .unwrap_or_else(|| "U".into());
        *class_counts
            .entry(if class_counts.contains_key(&dominant_class) {
                dominant_class.clone()
            } else {
                "other".into()
            })
            .or_default() += 1;
        clusters.push(Cluster {
            class_mismatch: votes.len() > 1,
            dominant_class,
            members,
        });
    }

    ComputationResult {
        unique_count: clusters.len(),
        raw_count: flat.len(),
        linked_count,
        unassigned_count: flat
            .iter()
            .filter(|(_, _, bbox)| !bbox.is_assigned())
            .count(),
        class_counts,
        side_counts,
        clusters,
    }
}
