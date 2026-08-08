use std::collections::HashSet;

use crate::{CommandError, CommandResult};

pub const MAX_BATCH_PATHS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchRefreshTarget {
    Wallpapers,
    Stats,
    Collections,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct BatchMutationResult {
    pub affected: usize,
    pub refresh: Vec<BatchRefreshTarget>,
}

pub fn validate_batch_rating(rating: i32) -> CommandResult<i32> {
    if matches!(rating, -1..=1) {
        Ok(rating)
    } else {
        Err(CommandError::new(
            "invalid_rating",
            "Rating must be -1, 0, or 1.",
        ))
    }
}

pub fn validate_relation_id(id: i64, relation: &str) -> CommandResult<i64> {
    if id > 0 {
        return Ok(id);
    }

    let (code, label) = match relation {
        "tag" => ("invalid_tag", "Tag"),
        "collection" => ("invalid_collection", "Collection"),
        _ => ("invalid_relation", "Relation"),
    };
    Err(CommandError::new(
        code,
        format!("{label} ID must be positive."),
    ))
}

pub fn normalize_batch_paths(paths: &[String]) -> CommandResult<Vec<String>> {
    if paths.is_empty() {
        return Err(CommandError::new(
            "empty_batch",
            "Select at least one wallpaper.",
        ));
    }
    if paths.len() > MAX_BATCH_PATHS {
        return Err(CommandError::new(
            "batch_limit_exceeded",
            format!("A batch may contain at most {MAX_BATCH_PATHS} wallpaper paths."),
        ));
    }

    let mut seen = HashSet::with_capacity(paths.len());
    let mut normalized = Vec::with_capacity(paths.len());
    for path in paths {
        let path = path.trim();
        if path.is_empty() {
            return Err(CommandError::new(
                "invalid_batch_path",
                "Wallpaper paths cannot be blank.",
            ));
        }
        if seen.insert(path.to_string()) {
            normalized.push(path.to_string());
        }
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_batch_paths, validate_batch_rating, validate_relation_id, BatchMutationResult,
        BatchRefreshTarget, MAX_BATCH_PATHS,
    };

    #[test]
    fn batch_operations_deduplicate_paths_without_reordering() {
        let paths = vec![
            "  C:\\Walls\\first.jpg  ".to_string(),
            "C:\\Walls\\second.jpg".to_string(),
            "C:\\Walls\\first.jpg".to_string(),
        ];

        let normalized = normalize_batch_paths(&paths).expect("batch should normalize");

        assert_eq!(
            normalized,
            vec![
                "C:\\Walls\\first.jpg".to_string(),
                "C:\\Walls\\second.jpg".to_string()
            ]
        );
    }

    #[test]
    fn batch_operations_reject_an_empty_batch() {
        let error = normalize_batch_paths(&[]).expect_err("empty batch should fail");

        assert_eq!(error.code, "empty_batch");
    }

    #[test]
    fn batch_operations_reject_more_than_the_fixed_upper_bound() {
        assert_eq!(MAX_BATCH_PATHS, 500);
        let paths = (0..=MAX_BATCH_PATHS)
            .map(|index| format!("C:\\Walls\\{index}.jpg"))
            .collect::<Vec<_>>();

        let error = normalize_batch_paths(&paths).expect_err("oversized batch should fail");

        assert_eq!(error.code, "batch_limit_exceeded");
        assert!(error.message.contains("500"));
    }

    #[test]
    fn batch_operations_reject_blank_paths() {
        let error =
            normalize_batch_paths(&["  ".to_string()]).expect_err("blank paths should fail");

        assert_eq!(error.code, "invalid_batch_path");
    }

    #[test]
    fn batch_operations_serialize_affected_count_and_refresh_targets() {
        let result = BatchMutationResult {
            affected: 2,
            refresh: vec![
                BatchRefreshTarget::Wallpapers,
                BatchRefreshTarget::Stats,
                BatchRefreshTarget::Collections,
            ],
        };

        let value = serde_json::to_value(result).expect("result should serialize");

        assert_eq!(
            value,
            serde_json::json!({
                "affected": 2,
                "refresh": ["wallpapers", "stats", "collections"]
            })
        );
    }

    #[test]
    fn batch_operations_accept_only_supported_ratings() {
        for rating in [-1, 0, 1] {
            assert_eq!(
                validate_batch_rating(rating).expect("rating should be valid"),
                rating
            );
        }

        let error = validate_batch_rating(2).expect_err("rating should be rejected");
        assert_eq!(error.code, "invalid_rating");
    }

    #[test]
    fn batch_operations_require_positive_relation_ids() {
        assert_eq!(
            validate_relation_id(7, "tag").expect("positive ID should pass"),
            7
        );
        let error = validate_relation_id(0, "collection").expect_err("zero ID should fail");
        assert_eq!(error.code, "invalid_collection");
    }
}
