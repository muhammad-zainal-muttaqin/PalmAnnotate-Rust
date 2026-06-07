pub mod dedup;
pub mod depth;
pub mod detector;
pub mod error;
pub mod model;
pub mod output;
pub mod quality;
pub mod results;
pub mod storage;
pub mod yolo;

pub use dedup::{suggest_tree, LinkSuggestion, SuggestionSignals};
pub use depth::{
    depth_color, depth_display_range, DepthDisplayRange, DEPTH_CEILING_MM, DEPTH_FLOOR_MM,
};
pub use detector::{decode_yolo, DetectorConfig, Letterbox};
pub use error::{AppError, AppResult, ErrorPayload};
pub use model::*;
pub use output::{build_output_v4, load_output_v4};
pub use quality::{check_tree, QualityIssue, QualityLevel, QualityReport};
pub use results::{compute_results, Cluster, ComputationResult};
pub use storage::AppStore;
pub use yolo::{parse_yolo, serialize_yolo};
