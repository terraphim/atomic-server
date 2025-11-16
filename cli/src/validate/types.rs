//! Type definitions for the Atomic Server validation service.
//!
//! These types provide comprehensive support for:
//! - All 12 Atomic Data datatypes (including Uri and JSON)
//! - ResourceResponse handling (single and referenced resources)
//! - Complete validation reporting at all levels
//! - Snapshot synchronization and diff generation

use atomic_lib::{Resource, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a complete snapshot of an Atomic Server instance.
/// Supports ResourceResponse with nested/referenced resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSnapshot {
    pub server_url: String,
    pub schema_version: SchemaVersion,
    pub resources: Vec<Resource>,
    /// Resources that were fetched as references alongside main resources.
    /// This supports the ResourceResponse::ResourceWithReferenced pattern.
    pub referenced_resources: Vec<Resource>,
    pub extracted_at: i64,
    pub metadata: SnapshotMetadata,
}

/// Schema version detection - supports both fork and upstream patterns
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SchemaVersion {
    /// Version 1: 10 datatypes (no Uri, no JSON, has Value::Resource)
    V1,
    /// Version 2: 12 datatypes (Uri, JSON, no Value::Resource, ResourceResponse)
    V2,
}

impl Default for SchemaVersion {
    fn default() -> Self {
        SchemaVersion::V2
    }
}

/// Metadata about the extracted snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub total_resources: usize,
    pub referenced_resources: usize,
    pub ontology_count: usize,
    pub class_count: usize,
    pub property_count: usize,
    pub agent_count: usize,
    pub commit_count: usize,
    /// Map of datatype usage frequency
    pub datatype_usage: HashMap<String, usize>,
}

impl Default for SnapshotMetadata {
    fn default() -> Self {
        Self {
            total_resources: 0,
            referenced_resources: 0,
            ontology_count: 0,
            class_count: 0,
            property_count: 0,
            agent_count: 0,
            commit_count: 0,
            datatype_usage: HashMap::new(),
        }
    }
}

/// Comprehensive validation report with all levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub level: ValidationLevel,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
    pub info: Vec<ValidationError>,
    pub summary: ValidationSummary,
    /// Resources that passed validation at the requested level
    pub valid_subjects: Vec<String>,
    /// Resources that failed validation
    pub invalid_subjects: Vec<String>,
}

impl ValidationReport {
    pub fn new(level: ValidationLevel) -> Self {
        Self {
            valid: true,
            level,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: Vec::new(),
            summary: ValidationSummary::default(),
            valid_subjects: Vec::new(),
            invalid_subjects: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.valid = false;
        self.invalid_subjects.push(error.subject.clone());
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: ValidationError) {
        self.warnings.push(warning);
    }

    pub fn add_info(&mut self, info: ValidationError) {
        self.info.push(info);
    }

    pub fn merge(&mut self, other: ValidationReport) {
        if !other.valid {
            self.valid = false;
        }
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self.info.extend(other.info);
        self.valid_subjects.extend(other.valid_subjects);
        self.invalid_subjects.extend(other.invalid_subjects);
    }
}

/// Individual validation error with rich context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub severity: ErrorSeverity,
    pub code: ValidationErrorCode,
    pub subject: String,
    pub property: Option<String>,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub context: Option<serde_json::Value>,
}

/// Error severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    Error,
    Warning,
    Info,
}

/// Comprehensive validation error codes covering all scenarios
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationErrorCode {
    // Level 0: Structural
    InvalidJsonAd,
    MalformedAtom,
    InvalidSubjectUrl,

    // Level 1: Datatype
    DatatypeMismatch,
    InvalidString,
    InvalidInteger,
    InvalidFloat,
    InvalidBoolean,
    InvalidDate,
    InvalidTimestamp,
    InvalidSlug,
    InvalidMarkdown,
    InvalidAtomicUrl,
    InvalidResourceArray,
    InvalidUri, // NEW: For Uri datatype
    InvalidJson, // NEW: For JSON datatype
    InvalidNestedResource,

    // Level 2: Schema
    MissingIsA,
    InvalidIsA,
    ClassNotFound,
    MissingRequiredProperty,
    PropertyNotFound,
    ValueNotAllowed,
    InvalidClassType,

    // Level 3: Referential Integrity
    BrokenReference,
    BrokenArrayReference,
    CircularParent,
    OrphanedResource,
    UnresolvableUrl,
    MissingReferencedResource,

    // Level 4: Cryptographic
    InvalidCommitSubject,
    InvalidCommitTimestamp,
    InvalidCommitSigner,
    MissingSignature,
    InvalidSignature,
    AgentNotFound,
    AgentNoPublicKey,
    TimestampOutOfWindow,
    BrokenCommitChain,
    InvalidPreviousCommit,

    // Level 5: Authorization
    UnauthorizedCommit,
    InsufficientReadRights,
    InsufficientWriteRights,
    InsufficientAppendRights,
    RightsCheckFailed,

    // Synchronization
    ConflictDetected,
    MergeFailure,
    SyncError,
}

impl std::fmt::Display for ValidationErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Summary statistics for validation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_resources: usize,
    pub valid_resources: usize,
    pub invalid_resources: usize,
    pub total_properties_checked: usize,
    pub missing_references: usize,
    pub schema_violations: usize,
    pub signature_failures: usize,
    pub authorization_failures: usize,
    /// Breakdown by datatype validation
    pub datatype_errors: HashMap<String, usize>,
}

/// Validation levels from structural to authorization
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationLevel {
    /// Level 0: JSON-AD parsing, basic structure
    Structural = 0,
    /// Level 1: Datatype validation (all 12 types including Uri and JSON)
    Datatype = 1,
    /// Level 2: Schema validation (class requirements, property constraints)
    Schema = 2,
    /// Level 3: Referential integrity (broken links, circular dependencies)
    Referential = 3,
    /// Level 4: Cryptographic (signatures, timestamps, commit chains)
    Cryptographic = 4,
    /// Level 5: Authorization (permissions, rights hierarchy)
    Authorization = 5,
}

impl From<u8> for ValidationLevel {
    fn from(level: u8) -> Self {
        match level {
            0 => Self::Structural,
            1 => Self::Datatype,
            2 => Self::Schema,
            3 => Self::Referential,
            4 => Self::Cryptographic,
            5 => Self::Authorization,
            _ => Self::Schema, // Default
        }
    }
}

impl std::fmt::Display for ValidationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Structural => write!(f, "Level 0 (Structural)"),
            Self::Datatype => write!(f, "Level 1 (Datatype)"),
            Self::Schema => write!(f, "Level 2 (Schema)"),
            Self::Referential => write!(f, "Level 3 (Referential)"),
            Self::Cryptographic => write!(f, "Level 4 (Cryptographic)"),
            Self::Authorization => write!(f, "Level 5 (Authorization)"),
        }
    }
}

/// Represents an extracted resource with its references.
/// Maps to atomic_lib::storelike::ResourceResponse
#[derive(Debug, Clone)]
pub struct ExtractedResource {
    pub main: Resource,
    pub referenced: Vec<Resource>,
}

impl ExtractedResource {
    pub fn single(resource: Resource) -> Self {
        Self {
            main: resource,
            referenced: Vec::new(),
        }
    }

    pub fn with_references(resource: Resource, referenced: Vec<Resource>) -> Self {
        Self {
            main: resource,
            referenced,
        }
    }

    /// Total count of all resources (main + referenced)
    pub fn total_count(&self) -> usize {
        1 + self.referenced.len()
    }

    /// Get all resources as a flat list
    pub fn all_resources(&self) -> Vec<&Resource> {
        let mut all = vec![&self.main];
        all.extend(self.referenced.iter());
        all
    }
}

/// Diff report comparing two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffReport {
    pub source_url: String,
    pub target_url: String,
    pub source_only: Vec<String>,
    pub target_only: Vec<String>,
    pub modified: Vec<ResourceDiff>,
    pub schema_changes: SchemaDiff,
    pub summary: DiffSummary,
}

/// Detailed diff for a single resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDiff {
    pub subject: String,
    pub added_properties: HashMap<String, serde_json::Value>,
    pub removed_properties: Vec<String>,
    pub modified_properties: HashMap<String, PropertyDiff>,
}

/// Diff for a single property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDiff {
    pub source_value: serde_json::Value,
    pub target_value: serde_json::Value,
    pub datatype: String,
}

/// Schema-level differences
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaDiff {
    pub new_ontologies: Vec<String>,
    pub removed_ontologies: Vec<String>,
    pub new_classes: Vec<String>,
    pub removed_classes: Vec<String>,
    pub modified_classes: Vec<String>,
    pub new_properties: Vec<String>,
    pub removed_properties: Vec<String>,
    pub modified_properties: Vec<String>,
}

/// Summary of differences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub total_differences: usize,
    pub resources_added: usize,
    pub resources_removed: usize,
    pub resources_modified: usize,
    pub schema_changes: usize,
}

/// Synchronization report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub success: bool,
    pub source_url: String,
    pub target_url: String,
    pub resources_created: usize,
    pub resources_updated: usize,
    pub resources_deleted: usize,
    pub referenced_resources_synced: usize,
    pub errors: Vec<SyncError>,
    pub conflicts: Vec<Conflict>,
    pub warnings: Vec<String>,
}

impl SyncReport {
    pub fn new(source_url: String, target_url: String) -> Self {
        Self {
            success: true,
            source_url,
            target_url,
            resources_created: 0,
            resources_updated: 0,
            resources_deleted: 0,
            referenced_resources_synced: 0,
            errors: Vec::new(),
            conflicts: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// Synchronization error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncError {
    pub subject: String,
    pub error: String,
    pub retry_count: usize,
    pub recoverable: bool,
}

/// Conflict between source and target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub subject: String,
    pub property: String,
    pub source_value: serde_json::Value,
    pub target_value: serde_json::Value,
    pub source_timestamp: Option<i64>,
    pub target_timestamp: Option<i64>,
    pub resolution: Option<ConflictResolution>,
}

/// How a conflict was resolved
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolution {
    UseSource,
    UseTarget,
    UsedLatest,
    Merged,
    Skipped,
    Manual,
}

/// Strategy for resolving conflicts
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConflictStrategy {
    Source,
    Target,
    Latest,
    Manual,
    Skip,
}

impl std::str::FromStr for ConflictStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "source" => Ok(ConflictStrategy::Source),
            "target" => Ok(ConflictStrategy::Target),
            "latest" => Ok(ConflictStrategy::Latest),
            "manual" => Ok(ConflictStrategy::Manual),
            "skip" => Ok(ConflictStrategy::Skip),
            _ => Err(format!("Unknown conflict strategy: {}", s)),
        }
    }
}

/// Synchronization mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncMode {
    Push,
    Pull,
    Bidirectional,
}

impl std::str::FromStr for SyncMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "push" => Ok(SyncMode::Push),
            "pull" => Ok(SyncMode::Pull),
            "bidirectional" | "bidi" => Ok(SyncMode::Bidirectional),
            _ => Err(format!("Unknown sync mode: {}", s)),
        }
    }
}

/// Options for synchronization
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub mode: SyncMode,
    pub conflict_strategy: ConflictStrategy,
    pub include_ontologies: bool,
    pub include_commit_history: bool,
    pub include_referenced_resources: bool,
    pub dry_run: bool,
    pub filter_classes: Option<Vec<String>>,
    pub filter_subjects: Option<Vec<String>>,
    pub batch_size: usize,
    pub retry_count: usize,
    pub timeout_ms: u64,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            mode: SyncMode::Push,
            conflict_strategy: ConflictStrategy::Source,
            include_ontologies: true,
            include_commit_history: false,
            include_referenced_resources: true,
            dry_run: false,
            filter_classes: None,
            filter_subjects: None,
            batch_size: 100,
            retry_count: 3,
            timeout_ms: 30000,
        }
    }
}

/// Options for extraction
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub include_references: bool,
    pub max_depth: usize,
    pub follow_external_urls: bool,
    pub include_ontologies: bool,
    pub include_system_resources: bool,
    pub batch_size: usize,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            include_references: true,
            max_depth: 10,
            follow_external_urls: false,
            include_ontologies: true,
            include_system_resources: false,
            batch_size: 100,
        }
    }
}

/// Utility to convert Value to JSON for serialization
pub fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Integer(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => serde_json::json!(f),
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Date(d) => serde_json::Value::String(d.clone()),
        Value::Timestamp(t) => serde_json::Value::Number((*t).into()),
        Value::Slug(s) => serde_json::Value::String(s.clone()),
        Value::Markdown(m) => serde_json::Value::String(m.clone()),
        Value::AtomicUrl(u) => serde_json::Value::String(u.clone()),
        Value::ResourceArray(arr) => {
            let items: Vec<serde_json::Value> = arr
                .iter()
                .map(|sub| match sub {
                    atomic_lib::values::SubResource::Subject(s) => {
                        serde_json::Value::String(s.clone())
                    }
                    atomic_lib::values::SubResource::Nested(n) => {
                        serde_json::json!({"nested": format!("{:?}", n)})
                    }
                })
                .collect();
            serde_json::Value::Array(items)
        }
        Value::NestedResource(n) => serde_json::json!({"nested": format!("{:?}", n)}),
        Value::Uri(u) => serde_json::Value::String(u.clone()),
        Value::JSON(j) => j.clone(),
        Value::Unsupported(u) => serde_json::json!({
            "unsupported": true,
            "datatype": u.datatype,
            "value": u.value,
        }),
    }
}

/// Get the datatype name for a Value
pub fn value_datatype_name(value: &Value) -> String {
    match value {
        Value::String(_) => "String".to_string(),
        Value::Integer(_) => "Integer".to_string(),
        Value::Float(_) => "Float".to_string(),
        Value::Boolean(_) => "Boolean".to_string(),
        Value::Date(_) => "Date".to_string(),
        Value::Timestamp(_) => "Timestamp".to_string(),
        Value::Slug(_) => "Slug".to_string(),
        Value::Markdown(_) => "Markdown".to_string(),
        Value::AtomicUrl(_) => "AtomicUrl".to_string(),
        Value::ResourceArray(_) => "ResourceArray".to_string(),
        Value::NestedResource(_) => "NestedResource".to_string(),
        Value::Uri(_) => "Uri".to_string(),
        Value::JSON(_) => "JSON".to_string(),
        Value::Unsupported(u) => format!("Unsupported({})", u.datatype),
    }
}
