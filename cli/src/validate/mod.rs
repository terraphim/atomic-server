//! Validation service module for Atomic Server.
//!
//! This module provides comprehensive validation and synchronization capabilities:
//! - Full support for all 12 Atomic Data datatypes (including Uri and JSON)
//! - ResourceResponse handling for nested/referenced resources
//! - Validation levels 0-5 (Structural to Authorization)
//! - Snapshot extraction and comparison
//! - Synchronization with conflict resolution

pub mod crypto;
pub mod extractor;
pub mod types;
pub mod validator;

use atomic_lib::Storelike;

// Re-export commonly used types
pub use crypto::{AuthorizationValidator, CryptoValidator};
pub use extractor::Extractor;
pub use types::{
    ConflictStrategy, DiffReport, ExtractOptions, ExtractedResource, SchemaVersion, SnapshotMetadata,
    StoreSnapshot, SyncMode, SyncOptions, SyncReport, ValidationError, ValidationErrorCode,
    ValidationLevel, ValidationReport,
};
pub use validator::Validator;

/// Validate a server instance at the specified level
pub fn validate_server(
    server_url: &str,
    agent_secret: Option<String>,
    level: ValidationLevel,
) -> Result<ValidationReport, String> {
    let agent = if let Some(secret) = agent_secret {
        Some(
            atomic_lib::agents::Agent::from_secret(&secret)
                .map_err(|e| format!("Invalid agent secret: {}", e))?,
        )
    } else {
        None
    };

    let extractor = Extractor::new(server_url, agent.clone())
        .map_err(|e| format!("Failed to create extractor: {}", e))?;

    // Create snapshot
    let snapshot = extractor
        .create_snapshot(None)
        .map_err(|e| format!("Failed to create snapshot: {}", e))?;

    // Validate all resources
    let mut validator = Validator::new(extractor.store(), level);
    let mut report = validator.validate_resources(&snapshot.resources);

    // If cryptographic validation is requested, also validate commits
    if level >= ValidationLevel::Cryptographic {
        let mut crypto_validator = CryptoValidator::new(extractor.store());

        // Find and validate all commit resources
        for resource in &snapshot.resources {
            let crypto_errors = crypto_validator.validate_commit(resource);
            for error in crypto_errors {
                match error.severity {
                    types::ErrorSeverity::Error => {
                        report.add_error(error);
                        report.summary.signature_failures += 1;
                    }
                    types::ErrorSeverity::Warning => report.add_warning(error),
                    types::ErrorSeverity::Info => report.add_info(error),
                }
            }
        }

        // If authorization validation is requested, check rights
        if level >= ValidationLevel::Authorization {
            let mut auth_validator = AuthorizationValidator::new(extractor.store());

            // Validate that commits have proper authorization
            for resource in &snapshot.resources {
                if is_commit_resource(resource) {
                    if let Ok(commit) = atomic_lib::commit::Commit::from_resource(resource.clone())
                    {
                        // Check if target resource exists
                        let target = extractor.store().get_resource(&commit.subject).ok();
                        let auth_errors =
                            auth_validator.validate_commit_authorization(&commit, target.as_ref());
                        for error in auth_errors {
                            match error.severity {
                                types::ErrorSeverity::Error => {
                                    report.add_error(error);
                                    report.summary.authorization_failures += 1;
                                }
                                types::ErrorSeverity::Warning => report.add_warning(error),
                                types::ErrorSeverity::Info => report.add_info(error),
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(report)
}

/// Check if a resource is a Commit
fn is_commit_resource(resource: &atomic_lib::Resource) -> bool {
    use atomic_lib::{urls, Value};
    if let Ok(Value::ResourceArray(classes)) = resource.get(urls::IS_A) {
        classes.iter().any(|item| match item {
            atomic_lib::values::SubResource::Subject(s) => s == urls::COMMIT,
            _ => false,
        })
    } else {
        false
    }
}

/// Extract an ontology from a server
pub fn extract_ontology(
    ontology_url: &str,
    agent_secret: Option<String>,
) -> Result<Vec<ExtractedResource>, String> {
    let agent = if let Some(secret) = agent_secret {
        Some(
            atomic_lib::agents::Agent::from_secret(&secret)
                .map_err(|e| format!("Invalid agent secret: {}", e))?,
        )
    } else {
        None
    };

    let server_url = url::Url::parse(ontology_url)
        .map_err(|e| format!("Invalid ontology URL: {}", e))?
        .origin()
        .unicode_serialization();

    let extractor = Extractor::new(&server_url, agent)
        .map_err(|e| format!("Failed to create extractor: {}", e))?;

    let resources = extractor
        .extract_ontology(ontology_url)
        .map_err(|e| format!("Failed to extract ontology: {}", e))?;

    Ok(resources)
}

/// Quick validation check for a single resource
pub fn validate_resource_quick(
    resource: &atomic_lib::Resource,
    store: &impl atomic_lib::storelike::Storelike,
) -> ValidationReport {
    let mut validator = Validator::new(store, ValidationLevel::Schema);
    validator.validate_resource(resource)
}

/// Detect the schema version of a server
pub fn detect_server_version(
    server_url: &str,
    agent_secret: Option<String>,
) -> Result<SchemaVersion, String> {
    let agent = if let Some(secret) = agent_secret {
        Some(
            atomic_lib::agents::Agent::from_secret(&secret)
                .map_err(|e| format!("Invalid agent secret: {}", e))?,
        )
    } else {
        None
    };

    let extractor = Extractor::new(server_url, agent)
        .map_err(|e| format!("Failed to create extractor: {}", e))?;

    Ok(extractor.detect_schema_version())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_level_from_u8() {
        assert_eq!(ValidationLevel::from(0u8), ValidationLevel::Structural);
        assert_eq!(ValidationLevel::from(1u8), ValidationLevel::Datatype);
        assert_eq!(ValidationLevel::from(2u8), ValidationLevel::Schema);
        assert_eq!(ValidationLevel::from(3u8), ValidationLevel::Referential);
        assert_eq!(ValidationLevel::from(4u8), ValidationLevel::Cryptographic);
        assert_eq!(ValidationLevel::from(5u8), ValidationLevel::Authorization);
        // Default to Schema for unknown levels
        assert_eq!(ValidationLevel::from(99u8), ValidationLevel::Schema);
    }

    #[test]
    fn test_conflict_strategy_from_str() {
        assert_eq!(
            "source".parse::<ConflictStrategy>().unwrap(),
            ConflictStrategy::Source
        );
        assert_eq!(
            "target".parse::<ConflictStrategy>().unwrap(),
            ConflictStrategy::Target
        );
        assert_eq!(
            "latest".parse::<ConflictStrategy>().unwrap(),
            ConflictStrategy::Latest
        );
        assert_eq!(
            "manual".parse::<ConflictStrategy>().unwrap(),
            ConflictStrategy::Manual
        );
        assert_eq!(
            "skip".parse::<ConflictStrategy>().unwrap(),
            ConflictStrategy::Skip
        );
        assert!("invalid".parse::<ConflictStrategy>().is_err());
    }

    #[test]
    fn test_sync_mode_from_str() {
        assert_eq!("push".parse::<SyncMode>().unwrap(), SyncMode::Push);
        assert_eq!("pull".parse::<SyncMode>().unwrap(), SyncMode::Pull);
        assert_eq!(
            "bidirectional".parse::<SyncMode>().unwrap(),
            SyncMode::Bidirectional
        );
        assert_eq!("bidi".parse::<SyncMode>().unwrap(), SyncMode::Bidirectional);
        assert!("invalid".parse::<SyncMode>().is_err());
    }
}
