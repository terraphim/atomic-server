# Atomic Server Validation Service: Implementation Plan

**Version:** 1.0.0
**Date:** 2025-11-06
**Estimated Duration:** 12 weeks
**Recommended Language:** Rust (Primary), TypeScript (Secondary)

## Quick Reference

- **Primary Spec Document**: [`VALIDATION_SPECIFICATION.md`](./VALIDATION_SPECIFICATION.md)
- **Target Directory**: `cli/src/validate/`
- **Binary Name**: `atomic-validate`
- **Dependencies**: `atomic-lib`, `tokio`, `reqwest`, `serde`, `clap`

---

## Project Structure

```
atomic-server/
├── VALIDATION_SPECIFICATION.md       # Formal specification (THIS DOCUMENT'S COMPANION)
├── VALIDATION_IMPLEMENTATION_PLAN.md # This document
├── cli/
│   ├── Cargo.toml                    # Add atomic-validate binary
│   └── src/
│       ├── validate/                  # NEW: Validation service module
│       │   ├── mod.rs                 # Module root, exports
│       │   ├── cli.rs                 # CLI interface and argument parsing
│       │   ├── types.rs               # Type definitions (StoreSnapshot, etc.)
│       │   ├── extractor.rs           # Data extraction logic
│       │   ├── validator.rs           # Validation engine (levels 0-5)
│       │   ├── synchronizer.rs        # Synchronization engine
│       │   ├── diff.rs                # Snapshot comparison
│       │   ├── conflict.rs            # Conflict resolution strategies
│       │   ├── report.rs              # Report generation (JSON, text)
│       │   └── utils.rs               # Utility functions
│       └── main.rs                    # Add validate subcommand
├── lib/                               # Existing atomic-lib (reuse extensively)
└── docs/
    ├── validation-service.md          # NEW: User guide
    ├── validation-levels.md           # NEW: Validation reference
    └── synchronization-guide.md       # NEW: Sync strategies
```

---

## Phase 1: Foundation (Weeks 1-3)

### Week 1: Project Setup

#### Day 1-2: Project Scaffolding
**Task**: Create directory structure and basic CLI

**Files to Create:**
1. `cli/src/validate/mod.rs`
2. `cli/src/validate/cli.rs`
3. `cli/src/validate/types.rs`

**`cli/Cargo.toml` - Add dependencies:**
```toml
[[bin]]
name = "atomic-validate"
path = "src/validate/cli.rs"

[dependencies]
atomic_lib = { path = "../lib" }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
```

**`cli/src/validate/cli.rs` - Skeleton:**
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "atomic-validate")]
#[command(about = "Atomic Server validation and synchronization tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate an Atomic Server instance
    Server {
        /// Server URL to validate
        url: String,
        /// Agent secret for authentication
        #[arg(long)]
        agent: Option<String>,
        /// Validation level (0-5)
        #[arg(long, default_value = "2")]
        level: u8,
        /// Output format (json, text)
        #[arg(long, default_value = "text")]
        output: String,
    },
    /// Extract and save a snapshot
    Extract {
        /// Server URL or ontology URL
        url: String,
        /// Output file path
        #[arg(long)]
        out: String,
        /// Agent secret
        #[arg(long)]
        agent: Option<String>,
        /// Extract specific ontology
        #[arg(long)]
        ontology: Option<String>,
    },
    /// Compare two server instances
    Compare {
        /// Source server URL
        source: String,
        /// Target server URL
        target: String,
        /// Source agent secret
        #[arg(long)]
        agent_source: Option<String>,
        /// Target agent secret
        #[arg(long)]
        agent_target: Option<String>,
        /// Output format
        #[arg(long, default_value = "text")]
        output: String,
    },
    /// Synchronize two server instances
    Sync {
        /// Source server URL
        source: String,
        /// Target server URL
        target: String,
        /// Sync mode: push, pull, bidirectional
        #[arg(long, default_value = "push")]
        mode: String,
        /// Conflict resolution: source, target, latest, manual
        #[arg(long, default_value = "source")]
        conflict: String,
        /// Agent secret
        #[arg(long)]
        agent: String,
        /// Dry run (don't apply changes)
        #[arg(long)]
        dry_run: bool,
        /// Include ontologies in sync
        #[arg(long)]
        include_ontologies: bool,
    },
    /// Validate a local snapshot file
    ValidateFile {
        /// Snapshot file path
        file: String,
        /// Validation level (0-5)
        #[arg(long, default_value = "3")]
        level: u8,
        /// Output format
        #[arg(long, default_value = "text")]
        output: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { url, agent, level, output } => {
            todo!("Implement server validation");
        }
        Commands::Extract { url, out, agent, ontology } => {
            todo!("Implement extraction");
        }
        Commands::Compare { source, target, agent_source, agent_target, output } => {
            todo!("Implement comparison");
        }
        Commands::Sync { source, target, mode, conflict, agent, dry_run, include_ontologies } => {
            todo!("Implement synchronization");
        }
        Commands::ValidateFile { file, level, output } => {
            todo!("Implement file validation");
        }
    }
}
```

**`cli/src/validate/types.rs` - Core types:**
```rust
use atomic_lib::{Resource, Commit};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSnapshot {
    pub server_url: String,
    pub resources: Vec<Resource>,
    pub commits: Vec<Commit>,
    pub extracted_at: i64,
    pub metadata: SnapshotMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub total_resources: usize,
    pub ontology_count: usize,
    pub class_count: usize,
    pub property_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
    pub summary: ValidationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub severity: ErrorSeverity,
    pub code: String,
    pub subject: String,
    pub property: Option<String>,
    pub message: String,
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_resources: usize,
    pub valid_resources: usize,
    pub invalid_resources: usize,
    pub missing_references: usize,
    pub schema_violations: usize,
    pub signature_failures: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum ValidationLevel {
    Level0, // Structural (JSON-AD parsing)
    Level1, // Datatype validation
    Level2, // Schema validation
    Level3, // Referential integrity
    Level4, // Cryptographic validation
    Level5, // Authorization validation
}

impl From<u8> for ValidationLevel {
    fn from(level: u8) -> Self {
        match level {
            0 => Self::Level0,
            1 => Self::Level1,
            2 => Self::Level2,
            3 => Self::Level3,
            4 => Self::Level4,
            5 => Self::Level5,
            _ => Self::Level2, // Default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffReport {
    pub source_only: Vec<String>,         // Subject URLs
    pub target_only: Vec<String>,
    pub modified: Vec<ResourceDiff>,
    pub schema_changes: SchemaDiff,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDiff {
    pub subject: String,
    pub added_properties: HashMap<String, serde_json::Value>,
    pub removed_properties: Vec<String>,
    pub modified_properties: HashMap<String, (serde_json::Value, serde_json::Value)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDiff {
    pub new_ontologies: Vec<String>,
    pub removed_ontologies: Vec<String>,
    pub new_classes: Vec<String>,
    pub removed_classes: Vec<String>,
    pub new_properties: Vec<String>,
    pub removed_properties: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub total_differences: usize,
    pub resources_added: usize,
    pub resources_removed: usize,
    pub resources_modified: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub success: bool,
    pub resources_created: usize,
    pub resources_updated: usize,
    pub resources_deleted: usize,
    pub errors: Vec<SyncError>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncError {
    pub subject: String,
    pub error: String,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub subject: String,
    pub property: String,
    pub source_value: serde_json::Value,
    pub target_value: serde_json::Value,
    pub source_timestamp: Option<i64>,
    pub target_timestamp: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum ConflictStrategy {
    Source,
    Target,
    Latest,
    Manual,
}
```

**Deliverables:**
- [ ] Project structure created
- [ ] CLI skeleton compiles
- [ ] Type definitions complete

#### Day 3-4: Basic Extractor

**File to Create:** `cli/src/validate/extractor.rs`

**Implementation:**
```rust
use atomic_lib::{Storelike, Store, Resource, errors::AtomicResult};
use std::collections::HashSet;
use crate::validate::types::StoreSnapshot;

pub struct Extractor {
    store: Store,
}

impl Extractor {
    pub fn new(server_url: &str, agent: Option<crate::agents::Agent>) -> AtomicResult<Self> {
        let store = Store::init()?;
        store.set_server_url(server_url);
        if let Some(agent) = agent {
            store.set_default_agent(agent);
        }
        Ok(Self { store })
    }

    /// Fetch a single resource with all its properties
    pub async fn fetch_resource(&self, subject: &str) -> AtomicResult<Resource> {
        self.store.get_resource(subject).await
    }

    /// Recursively fetch a resource and its children up to a depth limit
    pub async fn fetch_resource_recursive(
        &self,
        subject: &str,
        depth: usize,
        visited: &mut HashSet<String>,
    ) -> AtomicResult<Vec<Resource>> {
        if depth == 0 || visited.contains(subject) {
            return Ok(vec![]);
        }

        visited.insert(subject.to_string());
        let resource = self.fetch_resource(subject).await?;
        let mut resources = vec![resource.clone()];

        // Fetch nested resources referenced by AtomicUrl properties
        for (prop, value) in resource.get_propvals() {
            match value {
                atomic_lib::Value::AtomicUrl(url) => {
                    if !visited.contains(url.as_str()) {
                        let nested = self.fetch_resource_recursive(
                            url.as_str(),
                            depth - 1,
                            visited,
                        ).await?;
                        resources.extend(nested);
                    }
                }
                atomic_lib::Value::ResourceArray(arr) => {
                    for item in arr {
                        if let Some(url) = item.get_id() {
                            if !visited.contains(url) {
                                let nested = self.fetch_resource_recursive(
                                    url,
                                    depth - 1,
                                    visited,
                                ).await?;
                                resources.extend(nested);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(resources)
    }

    /// Extract an entire ontology with all classes, properties, and instances
    pub async fn extract_ontology(&self, ontology_url: &str) -> AtomicResult<Vec<Resource>> {
        use atomic_lib::urls;

        let ontology = self.fetch_resource(ontology_url).await?;
        let mut resources = vec![ontology.clone()];

        // Extract classes
        if let Ok(classes) = ontology.get(urls::CLASSES) {
            if let atomic_lib::Value::ResourceArray(class_urls) = classes {
                for class_item in class_urls {
                    if let Some(class_url) = class_item.get_id() {
                        let class_resource = self.fetch_resource(class_url).await?;
                        resources.push(class_resource);
                    }
                }
            }
        }

        // Extract properties
        if let Ok(properties) = ontology.get(urls::PROPERTIES) {
            if let atomic_lib::Value::ResourceArray(prop_urls) = properties {
                for prop_item in prop_urls {
                    if let Some(prop_url) = prop_item.get_id() {
                        let prop_resource = self.fetch_resource(prop_url).await?;
                        resources.push(prop_resource);
                    }
                }
            }
        }

        // Extract instances (optional)
        if let Ok(instances) = ontology.get(urls::INSTANCES) {
            if let atomic_lib::Value::ResourceArray(instance_urls) = instances {
                for instance_item in instance_urls {
                    if let Some(instance_url) = instance_item.get_id() {
                        let instance_resource = self.fetch_resource(instance_url).await?;
                        resources.push(instance_resource);
                    }
                }
            }
        }

        Ok(resources)
    }

    /// Create a snapshot of the entire store
    pub async fn create_snapshot(&self) -> AtomicResult<StoreSnapshot> {
        // Implementation will iterate all resources
        // For now, placeholder
        todo!("Implement full store extraction")
    }
}
```

**Deliverables:**
- [ ] Basic resource fetching works
- [ ] Recursive fetching with depth limit
- [ ] Ontology extraction complete
- [ ] Handles authentication

#### Day 5: Level 0-1 Validation

**File to Create:** `cli/src/validate/validator.rs`

**Implementation:**
```rust
use atomic_lib::{Resource, Storelike, Value};
use crate::validate::types::{
    ValidationReport, ValidationError, ValidationSummary,
    ErrorSeverity, ValidationLevel,
};

pub struct Validator<'a, S: Storelike> {
    store: &'a S,
    level: ValidationLevel,
}

impl<'a, S: Storelike> Validator<'a, S> {
    pub fn new(store: &'a S, level: ValidationLevel) -> Self {
        Self { store, level }
    }

    pub fn validate_resource(&self, resource: &Resource) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Level 0: Structural validation (already done by parsing)
        // Level 1: Datatype validation (Value::new() already validates)

        if matches!(self.level, ValidationLevel::Level2 | ValidationLevel::Level3 |
                    ValidationLevel::Level4 | ValidationLevel::Level5) {
            // Level 2: Schema validation
            if let Err(e) = resource.check_required_props(self.store) {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: "SCHEMA_VIOLATION".to_string(),
                    subject: resource.get_subject().to_string(),
                    property: None,
                    message: format!("Schema validation failed: {}", e),
                    context: None,
                });
            }
        }

        // Additional levels implemented in Phase 2

        errors
    }

    pub fn validate_snapshot(&self, resources: &[Resource]) -> ValidationReport {
        let mut all_errors = Vec::new();
        let mut all_warnings = Vec::new();
        let total_resources = resources.len();
        let mut invalid_count = 0;

        for resource in resources {
            let errors = self.validate_resource(resource);
            if !errors.is_empty() {
                invalid_count += 1;
                for error in errors {
                    match error.severity {
                        ErrorSeverity::Error => all_errors.push(error),
                        ErrorSeverity::Warning => all_warnings.push(error),
                        ErrorSeverity::Info => all_warnings.push(error),
                    }
                }
            }
        }

        ValidationReport {
            valid: all_errors.is_empty(),
            errors: all_errors,
            warnings: all_warnings,
            summary: ValidationSummary {
                total_resources,
                valid_resources: total_resources - invalid_count,
                invalid_resources: invalid_count,
                missing_references: 0, // Phase 2
                schema_violations: invalid_count,
                signature_failures: 0, // Phase 2
            },
        }
    }
}
```

**Deliverables:**
- [ ] Level 0-1 validation works
- [ ] Error reporting structured
- [ ] JSON output format supported

### Week 2: Level 2 Validation

#### Day 6-7: Schema Validation Enhancement

**Task**: Improve schema validation beyond basic required properties check

**Enhancement in `validator.rs`:**
```rust
fn validate_schema(&self, resource: &Resource) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check if resource has isA property
    match resource.get(atomic_lib::urls::IS_A) {
        Ok(atomic_lib::Value::ResourceArray(classes)) => {
            for class_item in classes {
                if let Some(class_url) = class_item.get_id() {
                    // Fetch class and validate against it
                    match self.store.get_resource(class_url) {
                        Ok(class_resource) => {
                            errors.extend(self.validate_against_class(resource, &class_resource));
                        }
                        Err(e) => {
                            errors.push(ValidationError {
                                severity: ErrorSeverity::Error,
                                code: "CLASS_NOT_FOUND".to_string(),
                                subject: resource.get_subject().to_string(),
                                property: Some(atomic_lib::urls::IS_A.to_string()),
                                message: format!("Class {} not found: {}", class_url, e),
                                context: None,
                            });
                        }
                    }
                }
            }
        }
        Ok(_) => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "INVALID_IS_A".to_string(),
                subject: resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::IS_A.to_string()),
                message: "isA property must be a ResourceArray".to_string(),
                context: None,
            });
        }
        Err(_) => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Warning,
                code: "MISSING_IS_A".to_string(),
                subject: resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::IS_A.to_string()),
                message: "Resource has no isA property (no class membership)".to_string(),
                context: None,
            });
        }
    }

    errors
}

fn validate_against_class(&self, resource: &Resource, class: &Resource) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check required properties
    if let Ok(atomic_lib::Value::ResourceArray(required)) = class.get(atomic_lib::urls::REQUIRES) {
        for prop_item in required {
            if let Some(prop_url) = prop_item.get_id() {
                if resource.get(prop_url).is_err() {
                    errors.push(ValidationError {
                        severity: ErrorSeverity::Error,
                        code: "MISSING_REQUIRED_PROPERTY".to_string(),
                        subject: resource.get_subject().to_string(),
                        property: Some(prop_url.to_string()),
                        message: format!("Required property {} is missing", prop_url),
                        context: None,
                    });
                }
            }
        }
    }

    // Check datatype constraints
    for (prop_url, value) in resource.get_propvals() {
        if let Ok(prop_resource) = self.store.get_resource(prop_url) {
            if let Err(e) = self.validate_property_value(&prop_resource, value) {
                errors.push(e);
            }
        }
    }

    errors
}

fn validate_property_value(&self, property: &Resource, value: &Value) -> Result<(), ValidationError> {
    // Check datatype match
    // This is already done by Value::new(), but we can add extra checks here

    // Check allows_only constraint (enum values)
    if let Ok(atomic_lib::Value::ResourceArray(allowed)) = property.get(atomic_lib::urls::ALLOWS_ONLY) {
        let allowed_strings: Vec<String> = allowed.iter()
            .filter_map(|item| item.get_id().map(|s| s.to_string()))
            .collect();

        if !allowed_strings.is_empty() {
            // Value must be one of the allowed values
            match value {
                Value::AtomicUrl(url) if !allowed_strings.contains(url) => {
                    return Err(ValidationError {
                        severity: ErrorSeverity::Error,
                        code: "VALUE_NOT_ALLOWED".to_string(),
                        subject: property.get_subject().to_string(),
                        property: Some(property.get_subject().to_string()),
                        message: format!("Value {} not in allowed list", url),
                        context: Some(serde_json::json!({"allowed": allowed_strings})),
                    });
                }
                _ => {}
            }
        }
    }

    Ok(())
}
```

**Deliverables:**
- [ ] Class-based validation complete
- [ ] Datatype constraints verified
- [ ] Enum value constraints checked

#### Day 8-10: Report Generation

**File to Create:** `cli/src/validate/report.rs`

**Implementation:**
```rust
use crate::validate::types::{ValidationReport, DiffReport, SyncReport};
use colored::*;

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn format_validation_text(report: &ValidationReport) -> String {
        let mut output = String::new();

        output.push_str(&format!("\n{}\n", "Validation Report".bold().underline()));
        output.push_str(&format!("Status: {}\n", if report.valid {
            "PASS".green().bold()
        } else {
            "FAIL".red().bold()
        }));

        output.push_str(&format!("\n{}\n", "Summary:".bold()));
        output.push_str(&format!("  Total Resources: {}\n", report.summary.total_resources));
        output.push_str(&format!("  Valid: {}\n", report.summary.valid_resources));
        output.push_str(&format!("  Invalid: {}\n", report.summary.invalid_resources));
        output.push_str(&format!("  Schema Violations: {}\n", report.summary.schema_violations));
        output.push_str(&format!("  Missing References: {}\n", report.summary.missing_references));
        output.push_str(&format!("  Signature Failures: {}\n", report.summary.signature_failures));

        if !report.errors.is_empty() {
            output.push_str(&format!("\n{}\n", format!("Errors ({}):", report.errors.len()).red().bold()));
            for (i, error) in report.errors.iter().take(50).enumerate() {
                output.push_str(&format!("  {}. [{}] {}\n", i + 1, error.code, error.message));
                output.push_str(&format!("     Subject: {}\n", error.subject));
                if let Some(prop) = &error.property {
                    output.push_str(&format!("     Property: {}\n", prop));
                }
            }
            if report.errors.len() > 50 {
                output.push_str(&format!("  ... and {} more errors\n", report.errors.len() - 50));
            }
        }

        if !report.warnings.is_empty() {
            output.push_str(&format!("\n{}\n", format!("Warnings ({}):", report.warnings.len()).yellow().bold()));
            for (i, warning) in report.warnings.iter().take(20).enumerate() {
                output.push_str(&format!("  {}. [{}] {}\n", i + 1, warning.code, warning.message));
            }
            if report.warnings.len() > 20 {
                output.push_str(&format!("  ... and {} more warnings\n", report.warnings.len() - 20));
            }
        }

        output
    }

    pub fn format_validation_json(report: &ValidationReport) -> String {
        serde_json::to_string_pretty(report).unwrap()
    }

    pub fn format_diff_text(report: &DiffReport) -> String {
        let mut output = String::new();

        output.push_str(&format!("\n{}\n", "Diff Report".bold().underline()));
        output.push_str(&format!("Total Differences: {}\n", report.summary.total_differences));
        output.push_str(&format!("  Added: {}\n", report.summary.resources_added));
        output.push_str(&format!("  Removed: {}\n", report.summary.resources_removed));
        output.push_str(&format!("  Modified: {}\n", report.summary.resources_modified));

        if !report.source_only.is_empty() {
            output.push_str(&format!("\n{}\n", "Resources in Source Only:".green()));
            for subject in report.source_only.iter().take(20) {
                output.push_str(&format!("  + {}\n", subject));
            }
            if report.source_only.len() > 20 {
                output.push_str(&format!("  ... and {} more\n", report.source_only.len() - 20));
            }
        }

        if !report.target_only.is_empty() {
            output.push_str(&format!("\n{}\n", "Resources in Target Only:".red()));
            for subject in report.target_only.iter().take(20) {
                output.push_str(&format!("  - {}\n", subject));
            }
            if report.target_only.len() > 20 {
                output.push_str(&format!("  ... and {} more\n", report.target_only.len() - 20));
            }
        }

        if !report.modified.is_empty() {
            output.push_str(&format!("\n{}\n", "Modified Resources:".yellow()));
            for diff in report.modified.iter().take(10) {
                output.push_str(&format!("  ~ {}\n", diff.subject));
                output.push_str(&format!("    Added properties: {}\n", diff.added_properties.len()));
                output.push_str(&format!("    Removed properties: {}\n", diff.removed_properties.len()));
                output.push_str(&format!("    Modified properties: {}\n", diff.modified_properties.len()));
            }
            if report.modified.len() > 10 {
                output.push_str(&format!("  ... and {} more\n", report.modified.len() - 10));
            }
        }

        output
    }

    pub fn format_sync_text(report: &SyncReport) -> String {
        let mut output = String::new();

        output.push_str(&format!("\n{}\n", "Sync Report".bold().underline()));
        output.push_str(&format!("Status: {}\n", if report.success {
            "SUCCESS".green().bold()
        } else {
            "FAILED".red().bold()
        }));

        output.push_str(&format!("\n{}\n", "Summary:".bold()));
        output.push_str(&format!("  Created: {}\n", report.resources_created));
        output.push_str(&format!("  Updated: {}\n", report.resources_updated));
        output.push_str(&format!("  Deleted: {}\n", report.resources_deleted));
        output.push_str(&format!("  Errors: {}\n", report.errors.len()));
        output.push_str(&format!("  Conflicts: {}\n", report.conflicts.len()));

        if !report.errors.is_empty() {
            output.push_str(&format!("\n{}\n", "Errors:".red().bold()));
            for (i, error) in report.errors.iter().take(10).enumerate() {
                output.push_str(&format!("  {}. {}: {}\n", i + 1, error.subject, error.error));
            }
        }

        if !report.conflicts.is_empty() {
            output.push_str(&format!("\n{}\n", "Conflicts:".yellow().bold()));
            for (i, conflict) in report.conflicts.iter().take(10).enumerate() {
                output.push_str(&format!("  {}. {} - {}\n", i + 1, conflict.subject, conflict.property));
            }
        }

        output
    }
}
```

**Add dependency to `cli/Cargo.toml`:**
```toml
colored = "2"
```

**Deliverables:**
- [ ] Text reports with color formatting
- [ ] JSON report output
- [ ] Human-readable diff display

---

## Phase 2: Advanced Validation (Weeks 4-6)

### Week 3-4: Referential Integrity (Level 3)

**Enhancement in `validator.rs`:**
```rust
async fn validate_referential_integrity(&self, resource: &Resource) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for (prop_url, value) in resource.get_propvals() {
        match value {
            Value::AtomicUrl(url) => {
                // Check if URL is resolvable
                if !self.is_local_id(url) {
                    match self.store.get_resource(url).await {
                        Err(e) => {
                            errors.push(ValidationError {
                                severity: ErrorSeverity::Error,
                                code: "BROKEN_REFERENCE".to_string(),
                                subject: resource.get_subject().to_string(),
                                property: Some(prop_url.to_string()),
                                message: format!("Referenced resource {} not found: {}", url, e),
                                context: None,
                            });
                        }
                        Ok(_) => {}
                    }
                }
            }
            Value::ResourceArray(arr) => {
                for item in arr {
                    if let Some(url) = item.get_id() {
                        if !self.is_local_id(url) {
                            match self.store.get_resource(url).await {
                                Err(e) => {
                                    errors.push(ValidationError {
                                        severity: ErrorSeverity::Error,
                                        code: "BROKEN_ARRAY_REFERENCE".to_string(),
                                        subject: resource.get_subject().to_string(),
                                        property: Some(prop_url.to_string()),
                                        message: format!("Array item {} not found: {}", url, e),
                                        context: None,
                                    });
                                }
                                Ok(_) => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Check for circular parent references
    if let Ok(Value::AtomicUrl(parent_url)) = resource.get(atomic_lib::urls::PARENT) {
        if self.has_circular_parent(resource.get_subject(), parent_url, &mut HashSet::new()).await {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "CIRCULAR_PARENT".to_string(),
                subject: resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::PARENT.to_string()),
                message: "Circular parent relationship detected".to_string(),
                context: None,
            });
        }
    }

    errors
}

async fn has_circular_parent(
    &self,
    original: &str,
    current: &str,
    visited: &mut HashSet<String>,
) -> bool {
    if current == original {
        return true;
    }
    if visited.contains(current) {
        return false;
    }
    visited.insert(current.to_string());

    if let Ok(resource) = self.store.get_resource(current).await {
        if let Ok(Value::AtomicUrl(parent)) = resource.get(atomic_lib::urls::PARENT) {
            return self.has_circular_parent(original, parent, visited).await;
        }
    }

    false
}

fn is_local_id(&self, url: &str) -> bool {
    !url.starts_with("http://") && !url.starts_with("https://")
}
```

**Deliverables:**
- [ ] Broken reference detection
- [ ] Circular dependency detection
- [ ] Local ID vs absolute URL handling

### Week 5: Cryptographic Validation (Level 4)

**Enhancement in `validator.rs`:**
```rust
use atomic_lib::authentication::verify_signature;

async fn validate_cryptographic(&self, resource: &Resource) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check if this is a Commit resource
    if let Ok(Value::ResourceArray(classes)) = resource.get(atomic_lib::urls::IS_A) {
        let is_commit = classes.iter().any(|item| {
            item.get_id() == Some(atomic_lib::urls::COMMIT)
        });

        if is_commit {
            errors.extend(self.validate_commit_signature(resource).await);
        }
    }

    errors
}

async fn validate_commit_signature(&self, commit_resource: &Resource) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Extract commit fields
    let subject = match commit_resource.get(atomic_lib::urls::SUBJECT) {
        Ok(Value::AtomicUrl(s)) => s,
        _ => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "INVALID_COMMIT_SUBJECT".to_string(),
                subject: commit_resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::SUBJECT.to_string()),
                message: "Commit must have a subject property".to_string(),
                context: None,
            });
            return errors;
        }
    };

    let created_at = match commit_resource.get(atomic_lib::urls::CREATED_AT) {
        Ok(Value::Timestamp(ts)) => ts,
        _ => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "INVALID_COMMIT_TIMESTAMP".to_string(),
                subject: commit_resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::CREATED_AT.to_string()),
                message: "Commit must have a valid timestamp".to_string(),
                context: None,
            });
            return errors;
        }
    };

    let signer = match commit_resource.get(atomic_lib::urls::SIGNER) {
        Ok(Value::AtomicUrl(s)) => s,
        _ => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "INVALID_COMMIT_SIGNER".to_string(),
                subject: commit_resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::SIGNER.to_string()),
                message: "Commit must have a signer property".to_string(),
                context: None,
            });
            return errors;
        }
    };

    let signature = match commit_resource.get(atomic_lib::urls::SIGNATURE) {
        Ok(Value::String(sig)) => sig,
        _ => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "MISSING_SIGNATURE".to_string(),
                subject: commit_resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::SIGNATURE.to_string()),
                message: "Commit must have a signature".to_string(),
                context: None,
            });
            return errors;
        }
    };

    // Fetch agent resource to get public key
    let agent_resource = match self.store.get_resource(signer).await {
        Ok(r) => r,
        Err(e) => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "AGENT_NOT_FOUND".to_string(),
                subject: commit_resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::SIGNER.to_string()),
                message: format!("Agent {} not found: {}", signer, e),
                context: None,
            });
            return errors;
        }
    };

    let public_key = match agent_resource.get(atomic_lib::urls::PUBLIC_KEY) {
        Ok(Value::String(key)) => key,
        _ => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "AGENT_NO_PUBLIC_KEY".to_string(),
                subject: commit_resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::SIGNER.to_string()),
                message: format!("Agent {} has no public key", signer),
                context: None,
            });
            return errors;
        }
    };

    // Verify signature
    let message = format!("{} {}", subject, created_at);
    match verify_signature(&message, signature, public_key) {
        Ok(true) => {}, // Valid signature
        Ok(false) | Err(_) => {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: "INVALID_SIGNATURE".to_string(),
                subject: commit_resource.get_subject().to_string(),
                property: Some(atomic_lib::urls::SIGNATURE.to_string()),
                message: "Signature verification failed".to_string(),
                context: Some(serde_json::json!({
                    "message": message,
                    "signature": signature,
                    "public_key": public_key,
                })),
            });
        }
    }

    // Check timestamp window (default: ±10 seconds)
    let now = chrono::Utc::now().timestamp_millis();
    let window = 10_000; // 10 seconds in ms
    if (now - created_at).abs() > window {
        errors.push(ValidationError {
            severity: ErrorSeverity::Warning,
            code: "TIMESTAMP_OUT_OF_WINDOW".to_string(),
            subject: commit_resource.get_subject().to_string(),
            property: Some(atomic_lib::urls::CREATED_AT.to_string()),
            message: format!("Timestamp {} is outside acceptable window", created_at),
            context: Some(serde_json::json!({
                "created_at": created_at,
                "now": now,
                "diff_ms": (now - created_at).abs(),
            })),
        });
    }

    errors
}
```

**Add dependency:**
```toml
chrono = "0.4"
```

**Deliverables:**
- [ ] Signature verification works
- [ ] Timestamp window checking
- [ ] Agent public key validation

### Week 6: Authorization Validation (Level 5)

**Enhancement in `validator.rs`:**
```rust
use atomic_lib::hierarchy::check_rights;

async fn validate_authorization(&self, resource: &Resource, agent: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // For commit resources, check if signer has write rights
    if let Ok(Value::ResourceArray(classes)) = resource.get(atomic_lib::urls::IS_A) {
        let is_commit = classes.iter().any(|item| {
            item.get_id() == Some(atomic_lib::urls::COMMIT)
        });

        if is_commit {
            if let Ok(Value::AtomicUrl(subject)) = resource.get(atomic_lib::urls::SUBJECT) {
                if let Ok(Value::AtomicUrl(signer)) = resource.get(atomic_lib::urls::SIGNER) {
                    match check_rights(self.store, subject, signer, atomic_lib::hierarchy::Right::Write).await {
                        Ok(true) => {}, // Has rights
                        Ok(false) => {
                            errors.push(ValidationError {
                                severity: ErrorSeverity::Error,
                                code: "UNAUTHORIZED_COMMIT".to_string(),
                                subject: resource.get_subject().to_string(),
                                property: Some(atomic_lib::urls::SIGNER.to_string()),
                                message: format!("Agent {} does not have write rights for {}", signer, subject),
                                context: None,
                            });
                        }
                        Err(e) => {
                            errors.push(ValidationError {
                                severity: ErrorSeverity::Warning,
                                code: "RIGHTS_CHECK_FAILED".to_string(),
                                subject: resource.get_subject().to_string(),
                                property: None,
                                message: format!("Could not check rights: {}", e),
                                context: None,
                            });
                        }
                    }
                }
            }
        }
    }

    errors
}
```

**Deliverables:**
- [ ] Authorization checking for commits
- [ ] Hierarchical permission traversal
- [ ] Write/Read/Append rights validation

---

## Phase 3: Synchronization Engine (Weeks 7-10)

### Week 7-8: Comparison and Diff

**File to Create:** `cli/src/validate/diff.rs`

**Implementation:**
```rust
use crate::validate::types::{DiffReport, ResourceDiff, SchemaDiff, DiffSummary};
use atomic_lib::Resource;
use std::collections::{HashMap, HashSet};

pub struct DiffEngine;

impl DiffEngine {
    pub fn compare_resources(source: &Resource, target: &Resource) -> ResourceDiff {
        let source_props: HashMap<_, _> = source.get_propvals().collect();
        let target_props: HashMap<_, _> = target.get_propvals().collect();

        let mut added = HashMap::new();
        let mut removed = Vec::new();
        let mut modified = HashMap::new();

        // Find added and modified properties
        for (prop, source_val) in &source_props {
            match target_props.get(prop) {
                None => {
                    // Property exists in source but not target
                    added.insert(prop.to_string(), serde_json::to_value(source_val).unwrap());
                }
                Some(target_val) => {
                    // Property exists in both, check if different
                    if !values_equal(source_val, target_val) {
                        modified.insert(
                            prop.to_string(),
                            (
                                serde_json::to_value(source_val).unwrap(),
                                serde_json::to_value(target_val).unwrap(),
                            ),
                        );
                    }
                }
            }
        }

        // Find removed properties
        for prop in target_props.keys() {
            if !source_props.contains_key(prop) {
                removed.push(prop.to_string());
            }
        }

        ResourceDiff {
            subject: source.get_subject().to_string(),
            added_properties: added,
            removed_properties: removed,
            modified_properties: modified,
        }
    }

    pub fn compare_snapshots(
        source_resources: &[Resource],
        target_resources: &[Resource],
    ) -> DiffReport {
        let source_map: HashMap<_, _> = source_resources.iter()
            .map(|r| (r.get_subject(), r))
            .collect();
        let target_map: HashMap<_, _> = target_resources.iter()
            .map(|r| (r.get_subject(), r))
            .collect();

        let source_subjects: HashSet<_> = source_map.keys().cloned().collect();
        let target_subjects: HashSet<_> = target_map.keys().cloned().collect();

        let source_only: Vec<_> = source_subjects.difference(&target_subjects)
            .map(|s| s.to_string())
            .collect();
        let target_only: Vec<_> = target_subjects.difference(&source_subjects)
            .map(|s| s.to_string())
            .collect();

        let mut modified = Vec::new();
        for subject in source_subjects.intersection(&target_subjects) {
            let source_res = source_map.get(subject).unwrap();
            let target_res = target_map.get(subject).unwrap();
            let diff = Self::compare_resources(source_res, target_res);

            if !diff.added_properties.is_empty() ||
               !diff.removed_properties.is_empty() ||
               !diff.modified_properties.is_empty() {
                modified.push(diff);
            }
        }

        // TODO: Implement schema diff detection
        let schema_changes = SchemaDiff {
            new_ontologies: Vec::new(),
            removed_ontologies: Vec::new(),
            new_classes: Vec::new(),
            removed_classes: Vec::new(),
            new_properties: Vec::new(),
            removed_properties: Vec::new(),
        };

        DiffReport {
            source_only: source_only.clone(),
            target_only: target_only.clone(),
            modified: modified.clone(),
            schema_changes,
            summary: DiffSummary {
                total_differences: source_only.len() + target_only.len() + modified.len(),
                resources_added: source_only.len(),
                resources_removed: target_only.len(),
                resources_modified: modified.len(),
            },
        }
    }
}

fn values_equal(a: &atomic_lib::Value, b: &atomic_lib::Value) -> bool {
    // Implement value comparison
    // For now, simple string comparison
    format!("{:?}", a) == format!("{:?}", b)
}
```

**Deliverables:**
- [ ] Resource-level diff generation
- [ ] Snapshot-level comparison
- [ ] Schema change detection

### Week 9: Synchronization Logic

**File to Create:** `cli/src/validate/synchronizer.rs`

**Implementation:**
```rust
use crate::validate::types::{SyncReport, SyncError, Conflict, ConflictStrategy};
use crate::validate::diff::DiffEngine;
use atomic_lib::{Resource, Storelike, agents::Agent, Commit};

pub struct Synchronizer<'a> {
    source_store: &'a dyn Storelike,
    target_url: String,
    agent: Agent,
}

impl<'a> Synchronizer<'a> {
    pub fn new(source_store: &'a dyn Storelike, target_url: String, agent: Agent) -> Self {
        Self {
            source_store,
            target_url,
            agent,
        }
    }

    pub async fn sync_push(
        &self,
        source_resources: Vec<Resource>,
        conflict_strategy: ConflictStrategy,
        dry_run: bool,
    ) -> anyhow::Result<SyncReport> {
        let mut report = SyncReport {
            success: true,
            resources_created: 0,
            resources_updated: 0,
            resources_deleted: 0,
            errors: Vec::new(),
            conflicts: Vec::new(),
        };

        // Sort resources by dependency (parents before children)
        let sorted = self.topological_sort(source_resources)?;

        for resource in sorted {
            if dry_run {
                tracing::info!("DRY RUN: Would sync {}", resource.get_subject());
                continue;
            }

            // Check if resource exists in target
            match self.fetch_target_resource(resource.get_subject()).await {
                Ok(Some(target_resource)) => {
                    // Resource exists, check for conflicts
                    let diff = DiffEngine::compare_resources(&resource, &target_resource);
                    if !diff.modified_properties.is_empty() {
                        // Handle conflict
                        match self.resolve_conflict(&resource, &target_resource, conflict_strategy).await {
                            Ok(resolved) => {
                                match self.push_resource(&resolved).await {
                                    Ok(_) => report.resources_updated += 1,
                                    Err(e) => {
                                        report.success = false;
                                        report.errors.push(SyncError {
                                            subject: resource.get_subject().to_string(),
                                            error: e.to_string(),
                                            retry_count: 0,
                                        });
                                    }
                                }
                            }
                            Err(conflict) => {
                                report.conflicts.push(conflict);
                            }
                        }
                    }
                }
                Ok(None) => {
                    // Resource doesn't exist, create it
                    match self.push_resource(&resource).await {
                        Ok(_) => report.resources_created += 1,
                        Err(e) => {
                            report.success = false;
                            report.errors.push(SyncError {
                                subject: resource.get_subject().to_string(),
                                error: e.to_string(),
                                retry_count: 0,
                            });
                        }
                    }
                }
                Err(e) => {
                    report.errors.push(SyncError {
                        subject: resource.get_subject().to_string(),
                        error: format!("Failed to fetch from target: {}", e),
                        retry_count: 0,
                    });
                }
            }
        }

        Ok(report)
    }

    async fn fetch_target_resource(&self, subject: &str) -> anyhow::Result<Option<Resource>> {
        // Use reqwest to fetch from target server
        let client = reqwest::Client::new();
        let response = client.get(subject)
            .header("Accept", "application/ad+json")
            .send()
            .await?;

        if response.status() == 404 {
            return Ok(None);
        }

        let json = response.text().await?;
        let resource = atomic_lib::parse::parse_json_ad_resource(
            &json,
            self.source_store,
            &Default::default(),
        )?;

        Ok(Some(resource))
    }

    async fn push_resource(&self, resource: &Resource) -> anyhow::Result<()> {
        // Create commit for this resource
        let commit = Commit::new_from_resource(resource, &self.agent)?;

        // Serialize commit to JSON-AD
        let commit_json = atomic_lib::serialize::resource_to_json_ad(&commit.into_resource()?)?;

        // POST to target server
        let client = reqwest::Client::new();
        let response = client.post(&format!("{}/commit", self.target_url))
            .header("Content-Type", "application/ad+json")
            .body(commit_json)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to push commit: {}", response.status());
        }

        Ok(())
    }

    async fn resolve_conflict(
        &self,
        source: &Resource,
        target: &Resource,
        strategy: ConflictStrategy,
    ) -> Result<Resource, Conflict> {
        match strategy {
            ConflictStrategy::Source => Ok(source.clone()),
            ConflictStrategy::Target => Ok(target.clone()),
            ConflictStrategy::Latest => {
                // Compare timestamps
                let source_ts = self.get_last_commit_timestamp(source).unwrap_or(0);
                let target_ts = self.get_last_commit_timestamp(target).unwrap_or(0);

                if source_ts >= target_ts {
                    Ok(source.clone())
                } else {
                    Ok(target.clone())
                }
            }
            ConflictStrategy::Manual => {
                // Return conflict for manual resolution
                Err(Conflict {
                    subject: source.get_subject().to_string(),
                    property: "multiple".to_string(),
                    source_value: serde_json::to_value(source).unwrap(),
                    target_value: serde_json::to_value(target).unwrap(),
                    source_timestamp: self.get_last_commit_timestamp(source),
                    target_timestamp: self.get_last_commit_timestamp(target),
                })
            }
        }
    }

    fn get_last_commit_timestamp(&self, resource: &Resource) -> Option<i64> {
        // Extract last commit timestamp if available
        resource.get(atomic_lib::urls::LAST_COMMIT)
            .ok()
            .and_then(|commit_url| {
                if let atomic_lib::Value::AtomicUrl(url) = commit_url {
                    self.source_store.get_resource(url).ok()
                        .and_then(|commit| {
                            commit.get(atomic_lib::urls::CREATED_AT).ok()
                                .and_then(|ts| {
                                    if let atomic_lib::Value::Timestamp(t) = ts {
                                        Some(t)
                                    } else {
                                        None
                                    }
                                })
                        })
                } else {
                    None
                }
            })
    }

    fn topological_sort(&self, resources: Vec<Resource>) -> anyhow::Result<Vec<Resource>> {
        // Implement topological sort based on parent relationships
        // For now, simple implementation
        let mut sorted = resources;
        sorted.sort_by_key(|r| {
            // Resources with no parent come first
            if r.get(atomic_lib::urls::PARENT).is_err() {
                0
            } else {
                1
            }
        });
        Ok(sorted)
    }
}
```

**Deliverables:**
- [ ] Push synchronization works
- [ ] Conflict detection and resolution
- [ ] Topological sorting for dependencies

### Week 10: Conflict Resolution

**File to Create:** `cli/src/validate/conflict.rs`

**Implementation:**
```rust
use crate::validate::types::{Conflict, ConflictStrategy};
use atomic_lib::Resource;
use std::io::{self, Write};

pub struct ConflictResolver;

impl ConflictResolver {
    pub fn resolve_interactive(conflict: &Conflict) -> anyhow::Result<ConflictStrategy> {
        println!("\n{}", "Conflict Detected".bold().yellow());
        println!("Subject: {}", conflict.subject);
        println!("Property: {}", conflict.property);
        println!("\nSource value:");
        println!("{}", serde_json::to_string_pretty(&conflict.source_value)?);
        println!("\nTarget value:");
        println!("{}", serde_json::to_string_pretty(&conflict.target_value)?);

        if let (Some(source_ts), Some(target_ts)) = (conflict.source_timestamp, conflict.target_timestamp) {
            println!("\nTimestamps:");
            println!("  Source: {} ({})", source_ts, Self::format_timestamp(source_ts));
            println!("  Target: {} ({})", target_ts, Self::format_timestamp(target_ts));
        }

        println!("\nResolution options:");
        println!("  [s] Use source value");
        println!("  [t] Use target value");
        println!("  [l] Use latest (by timestamp)");
        println!("  [k] Skip this resource");

        print!("\nChoice: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "s" => Ok(ConflictStrategy::Source),
            "t" => Ok(ConflictStrategy::Target),
            "l" => Ok(ConflictStrategy::Latest),
            "k" => anyhow::bail!("Skipped by user"),
            _ => {
                println!("Invalid choice, using source");
                Ok(ConflictStrategy::Source)
            }
        }
    }

    fn format_timestamp(ts: i64) -> String {
        use chrono::{DateTime, Utc};
        let dt: DateTime<Utc> = DateTime::from_timestamp_millis(ts).unwrap();
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }
}
```

**Deliverables:**
- [ ] Interactive conflict resolution
- [ ] Automated strategies (source, target, latest)
- [ ] Conflict logging for batch processing

---

## Phase 4: Testing and Documentation (Weeks 11-12)

### Week 11: Testing

#### Integration Tests

**File to Create:** `cli/tests/validation_tests.rs`

**Test Scenarios:**
1. Extract ontology from live server
2. Validate snapshot at all levels
3. Compare two identical instances (no diff)
4. Compare instances with differences
5. Sync from source to empty target
6. Sync with conflicts
7. Handle network failures gracefully

**Example Test:**
```rust
#[tokio::test]
async fn test_extract_core_ontology() {
    let extractor = Extractor::new("https://atomicdata.dev", None).unwrap();
    let resources = extractor.extract_ontology("https://atomicdata.dev/ontology/core").await.unwrap();

    assert!(!resources.is_empty());
    assert!(resources.iter().any(|r| r.get_subject().contains("/ontology/core")));
}

#[tokio::test]
async fn test_validation_levels() {
    let store = Store::init().unwrap();
    // Add test resources

    let validator = Validator::new(&store, ValidationLevel::Level2);
    let report = validator.validate_snapshot(&resources);

    assert!(report.valid);
    assert_eq!(report.errors.len(), 0);
}

#[tokio::test]
async fn test_sync_empty_target() {
    // Set up source and target
    let source_resources = vec![/* ... */];
    let synchronizer = Synchronizer::new(&source_store, target_url, agent);

    let report = synchronizer.sync_push(
        source_resources,
        ConflictStrategy::Source,
        false,
    ).await.unwrap();

    assert!(report.success);
    assert_eq!(report.resources_created, expected_count);
}
```

**Deliverables:**
- [ ] 90%+ code coverage
- [ ] All test scenarios pass
- [ ] Performance benchmarks documented

### Week 12: Documentation

#### User Documentation

**File to Create:** `docs/validation-service.md`

**Contents:**
- Overview and purpose
- Installation instructions
- Basic usage examples
- CLI command reference
- Configuration options

**File to Create:** `docs/validation-levels.md`

**Contents:**
- Detailed explanation of each level
- What gets validated at each level
- Performance implications
- When to use each level

**File to Create:** `docs/synchronization-guide.md`

**Contents:**
- Synchronization concepts
- Conflict resolution strategies
- Best practices
- Troubleshooting common issues
- Advanced scenarios (bidirectional, incremental)

#### API Documentation

Generate rustdoc:
```bash
cargo doc --no-deps --document-private-items -p atomic-validate
```

**Deliverables:**
- [ ] Complete user documentation
- [ ] API documentation (rustdoc)
- [ ] Example scripts and use cases
- [ ] Troubleshooting guide

---

## Testing Strategy

### Unit Tests
- Each module has corresponding unit tests
- Mock stores for testing validators
- Test all error paths

### Integration Tests
- Test against live atomic servers
- Test with extract-ontology workflow
- Test all CLI commands end-to-end

### Performance Tests
- Benchmark extraction speed (resources/sec)
- Benchmark validation speed per level
- Benchmark sync throughput (commits/sec)
- Memory profiling for large datasets

### Test Data
- Use public atomicdata.dev ontologies
- Create test fixtures with known violations
- Generate synthetic datasets for scale testing

---

## Success Criteria

### Phase 1
- [ ] CLI extracts 1,000+ resource ontology successfully
- [ ] Validator detects 95%+ of schema violations
- [ ] Validation completes in <10s for 1,000 resources

### Phase 2
- [ ] Cryptographic validator detects 100% of invalid signatures
- [ ] Referential integrity finds all broken links
- [ ] Authorization validator matches server behavior 100%

### Phase 3
- [ ] Sync replicates 10,000+ resource database
- [ ] Conflict resolution handles 95%+ automatically
- [ ] Throughput: >100 commits/second

### Phase 4
- [ ] 90%+ code coverage
- [ ] Zero critical bugs
- [ ] Complete documentation

---

## Dependencies

### Rust Crates
```toml
[dependencies]
atomic_lib = { path = "../lib" }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
colored = "2"
chrono = "0.4"

[dev-dependencies]
tempfile = "3"
```

---

## Risk Mitigation

### Technical Risks
1. **Large dataset memory consumption**
   - Mitigation: Implement streaming, batch processing
2. **Network failures during sync**
   - Mitigation: Retry with exponential backoff, resume capability
3. **Complex conflict scenarios**
   - Mitigation: Start with simple strategies, iterate

### Project Risks
1. **Scope creep**
   - Mitigation: Strict phase boundaries, defer advanced features
2. **Dependency on atomic-lib changes**
   - Mitigation: Pin versions, coordinate with core team
3. **Performance bottlenecks**
   - Mitigation: Profile early, optimize critical paths

---

## Future Enhancements (Post-Phase 4)

1. **Incremental Sync**: Only sync changes since last sync
2. **Merkle Tree Optimization**: Fast snapshot comparison
3. **Web Interface**: Browser-based validation dashboard
4. **Distributed Validation**: Peer-to-peer validation network
5. **Machine Learning**: Anomaly detection, predictive validation
6. **CRDT Support**: Conflict-free replicated data types

---

## Contributing

Once the foundation is complete, external contributors can:
- Add new validation rules
- Improve conflict resolution strategies
- Add new output formats
- Improve performance
- Write additional documentation

**Contribution Guide:** See `CONTRIBUTING.md` in the repository root.

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-11-06 | Claude | Initial implementation plan |
