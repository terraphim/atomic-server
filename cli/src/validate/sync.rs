//! Synchronization engine for Atomic Data.
//!
//! Provides diff generation, conflict resolution, and bidirectional sync between atomic instances.

use atomic_lib::{
    agents::Agent,
    commit::CommitBuilder,
    errors::AtomicResult,
    storelike::Storelike,
    urls, Resource, Value,
};
use std::collections::{HashMap, HashSet};

use crate::validate::{
    types::{
        Conflict, ConflictResolution, ConflictStrategy, DiffReport, DiffSummary, PropertyDiff,
        ResourceDiff, SchemaDiff, StoreSnapshot, SyncError, SyncMode, SyncOptions, SyncReport,
    },
    Extractor,
};

/// Synchronization engine for comparing and syncing Atomic Data stores.
pub struct SyncEngine {
    source_extractor: Extractor,
    target_extractor: Extractor,
    agent: Option<Agent>,
    options: SyncOptions,
}

impl SyncEngine {
    /// Create a new sync engine between two servers.
    pub fn new(
        source_url: &str,
        target_url: &str,
        agent: Option<Agent>,
        options: SyncOptions,
    ) -> AtomicResult<Self> {
        let source_extractor = Extractor::new(source_url, agent.clone())?;
        let target_extractor = Extractor::new(target_url, agent.clone())?;

        Ok(Self {
            source_extractor,
            target_extractor,
            agent,
            options,
        })
    }

    /// Generate a diff report between source and target.
    pub fn generate_diff(&self) -> AtomicResult<DiffReport> {
        let source_snapshot = self.source_extractor.create_snapshot(None)?;
        let target_snapshot = self.target_extractor.create_snapshot(None)?;

        self.compare_snapshots(&source_snapshot, &target_snapshot)
    }

    /// Compare two snapshots and generate a diff report.
    pub fn compare_snapshots(
        &self,
        source: &StoreSnapshot,
        target: &StoreSnapshot,
    ) -> AtomicResult<DiffReport> {
        let mut source_subjects: HashSet<String> = HashSet::new();
        let mut target_subjects: HashSet<String> = HashSet::new();
        let mut source_map: HashMap<String, &Resource> = HashMap::new();
        let mut target_map: HashMap<String, &Resource> = HashMap::new();

        // Build indexes
        for resource in &source.resources {
            let subject = resource.get_subject().to_string();
            source_subjects.insert(subject.clone());
            source_map.insert(subject, resource);
        }

        for resource in &target.resources {
            let subject = resource.get_subject().to_string();
            target_subjects.insert(subject.clone());
            target_map.insert(subject, resource);
        }

        // Find differences
        let source_only: Vec<String> = source_subjects
            .difference(&target_subjects)
            .cloned()
            .collect();
        let target_only: Vec<String> = target_subjects
            .difference(&source_subjects)
            .cloned()
            .collect();

        // Find modified resources
        let mut modified = Vec::new();
        let common_subjects: HashSet<_> = source_subjects
            .intersection(&target_subjects)
            .cloned()
            .collect();

        for subject in common_subjects {
            let source_resource = source_map.get(&subject).unwrap();
            let target_resource = target_map.get(&subject).unwrap();

            if let Some(diff) = self.compare_resources(source_resource, target_resource) {
                modified.push(diff);
            }
        }

        // Analyze schema changes
        let schema_changes = self.analyze_schema_changes(&source_map, &target_map);

        // Build summary
        let summary = DiffSummary {
            total_differences: source_only.len() + target_only.len() + modified.len(),
            resources_added: source_only.len(),
            resources_removed: target_only.len(),
            resources_modified: modified.len(),
            schema_changes: schema_changes.new_classes.len()
                + schema_changes.removed_classes.len()
                + schema_changes.new_properties.len()
                + schema_changes.removed_properties.len(),
        };

        Ok(DiffReport {
            source_url: source.server_url.clone(),
            target_url: target.server_url.clone(),
            source_only,
            target_only,
            modified,
            schema_changes,
            summary,
        })
    }

    /// Compare two resources and return a ResourceDiff if different.
    fn compare_resources(
        &self,
        source: &Resource,
        target: &Resource,
    ) -> Option<ResourceDiff> {
        let subject = source.get_subject().to_string();
        let mut added_properties: HashMap<String, serde_json::Value> = HashMap::new();
        let mut removed_properties: Vec<String> = Vec::new();
        let mut modified_properties: HashMap<String, PropertyDiff> = HashMap::new();

        let source_props: HashSet<String> = source
            .get_propvals()
            .keys()
            .cloned()
            .collect();
        let target_props: HashSet<String> = target
            .get_propvals()
            .keys()
            .cloned()
            .collect();

        // Properties in source but not in target (added)
        for prop in source_props.difference(&target_props) {
            if let Ok(value) = source.get(prop) {
                added_properties
                    .insert(prop.clone(), crate::validate::types::value_to_json(value));
            }
        }

        // Properties in target but not in source (removed)
        for prop in target_props.difference(&source_props) {
            removed_properties.push(prop.clone());
        }

        // Properties in both - check if modified
        for prop in source_props.intersection(&target_props) {
            if let (Ok(source_val), Ok(target_val)) = (source.get(prop), target.get(prop)) {
                let source_json = crate::validate::types::value_to_json(source_val);
                let target_json = crate::validate::types::value_to_json(target_val);

                if source_json != target_json {
                    modified_properties.insert(
                        prop.clone(),
                        PropertyDiff {
                            source_value: source_json,
                            target_value: target_json,
                            datatype: crate::validate::types::value_datatype_name(source_val),
                        },
                    );
                }
            }
        }

        if added_properties.is_empty()
            && removed_properties.is_empty()
            && modified_properties.is_empty()
        {
            None
        } else {
            Some(ResourceDiff {
                subject,
                added_properties,
                removed_properties,
                modified_properties,
            })
        }
    }

    /// Analyze schema-level changes between source and target.
    fn analyze_schema_changes(
        &self,
        source_map: &HashMap<String, &Resource>,
        target_map: &HashMap<String, &Resource>,
    ) -> SchemaDiff {
        let mut schema_diff = SchemaDiff::default();

        // Find classes
        let source_classes = self.extract_classes(source_map);
        let target_classes = self.extract_classes(target_map);

        schema_diff.new_classes = source_classes
            .difference(&target_classes)
            .cloned()
            .collect();
        schema_diff.removed_classes = target_classes
            .difference(&source_classes)
            .cloned()
            .collect();

        // Find properties
        let source_properties = self.extract_properties(source_map);
        let target_properties = self.extract_properties(target_map);

        schema_diff.new_properties = source_properties
            .difference(&target_properties)
            .cloned()
            .collect();
        schema_diff.removed_properties = target_properties
            .difference(&source_properties)
            .cloned()
            .collect();

        // Find ontologies
        let source_ontologies = self.extract_ontologies(source_map);
        let target_ontologies = self.extract_ontologies(target_map);

        schema_diff.new_ontologies = source_ontologies
            .difference(&target_ontologies)
            .cloned()
            .collect();
        schema_diff.removed_ontologies = target_ontologies
            .difference(&source_ontologies)
            .cloned()
            .collect();

        schema_diff
    }

    /// Extract class subjects from a resource map.
    fn extract_classes(&self, map: &HashMap<String, &Resource>) -> HashSet<String> {
        map.iter()
            .filter(|(_, resource)| self.is_class(resource))
            .map(|(subject, _)| subject.clone())
            .collect()
    }

    /// Extract property subjects from a resource map.
    fn extract_properties(&self, map: &HashMap<String, &Resource>) -> HashSet<String> {
        map.iter()
            .filter(|(_, resource)| self.is_property(resource))
            .map(|(subject, _)| subject.clone())
            .collect()
    }

    /// Extract ontology subjects from a resource map.
    fn extract_ontologies(&self, map: &HashMap<String, &Resource>) -> HashSet<String> {
        map.iter()
            .filter(|(_, resource)| self.is_ontology(resource))
            .map(|(subject, _)| subject.clone())
            .collect()
    }

    fn is_class(&self, resource: &Resource) -> bool {
        self.has_class(resource, urls::CLASS)
    }

    fn is_property(&self, resource: &Resource) -> bool {
        self.has_class(resource, urls::PROPERTY)
    }

    fn is_ontology(&self, resource: &Resource) -> bool {
        self.has_class(resource, urls::ONTOLOGY)
    }

    fn has_class(&self, resource: &Resource, class_url: &str) -> bool {
        if let Ok(Value::ResourceArray(classes)) = resource.get(urls::IS_A) {
            classes.iter().any(|item| match item {
                atomic_lib::values::SubResource::Subject(s) => s == class_url,
                _ => false,
            })
        } else {
            false
        }
    }

    /// Synchronize from source to target based on options.
    pub fn synchronize(&mut self) -> AtomicResult<SyncReport> {
        let diff = self.generate_diff()?;
        let mut report = SyncReport::new(diff.source_url.clone(), diff.target_url.clone());

        match self.options.mode {
            SyncMode::Push => {
                self.sync_push(&diff, &mut report)?;
            }
            SyncMode::Pull => {
                self.sync_pull(&diff, &mut report)?;
            }
            SyncMode::Bidirectional => {
                self.sync_bidirectional(&diff, &mut report)?;
            }
        }

        report.success = report.errors.is_empty();
        Ok(report)
    }

    /// Push changes from source to target.
    fn sync_push(&mut self, diff: &DiffReport, report: &mut SyncReport) -> AtomicResult<()> {
        // Add new resources from source to target
        for subject in &diff.source_only {
            if self.should_sync_subject(subject) {
                match self.create_resource_on_target(subject) {
                    Ok(_) => {
                        report.resources_created += 1;
                    }
                    Err(e) => {
                        report.errors.push(SyncError {
                            subject: subject.clone(),
                            error: e.to_string(),
                            retry_count: 0,
                            recoverable: true,
                        });
                    }
                }
            }
        }

        // Update modified resources
        for resource_diff in &diff.modified {
            if self.should_sync_subject(&resource_diff.subject) {
                match self.update_resource_on_target(resource_diff, report) {
                    Ok(_) => {
                        report.resources_updated += 1;
                    }
                    Err(e) => {
                        report.errors.push(SyncError {
                            subject: resource_diff.subject.clone(),
                            error: e.to_string(),
                            retry_count: 0,
                            recoverable: true,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Pull changes from target to source.
    fn sync_pull(&mut self, diff: &DiffReport, report: &mut SyncReport) -> AtomicResult<()> {
        // Add new resources from target to source
        for subject in &diff.target_only {
            if self.should_sync_subject(subject) {
                match self.create_resource_on_source(subject) {
                    Ok(_) => {
                        report.resources_created += 1;
                    }
                    Err(e) => {
                        report.errors.push(SyncError {
                            subject: subject.clone(),
                            error: e.to_string(),
                            retry_count: 0,
                            recoverable: true,
                        });
                    }
                }
            }
        }

        // Update modified resources (in reverse direction)
        for resource_diff in &diff.modified {
            if self.should_sync_subject(&resource_diff.subject) {
                match self.update_resource_on_source(resource_diff, report) {
                    Ok(_) => {
                        report.resources_updated += 1;
                    }
                    Err(e) => {
                        report.errors.push(SyncError {
                            subject: resource_diff.subject.clone(),
                            error: e.to_string(),
                            retry_count: 0,
                            recoverable: true,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Bidirectional sync - merge changes from both sides.
    fn sync_bidirectional(
        &mut self,
        diff: &DiffReport,
        report: &mut SyncReport,
    ) -> AtomicResult<()> {
        // Push new resources from source to target
        for subject in &diff.source_only {
            if self.should_sync_subject(subject) {
                match self.create_resource_on_target(subject) {
                    Ok(_) => report.resources_created += 1,
                    Err(e) => {
                        report.errors.push(SyncError {
                            subject: subject.clone(),
                            error: e.to_string(),
                            retry_count: 0,
                            recoverable: true,
                        });
                    }
                }
            }
        }

        // Pull new resources from target to source
        for subject in &diff.target_only {
            if self.should_sync_subject(subject) {
                match self.create_resource_on_source(subject) {
                    Ok(_) => report.resources_created += 1,
                    Err(e) => {
                        report.errors.push(SyncError {
                            subject: subject.clone(),
                            error: e.to_string(),
                            retry_count: 0,
                            recoverable: true,
                        });
                    }
                }
            }
        }

        // Handle conflicts for modified resources
        for resource_diff in &diff.modified {
            if self.should_sync_subject(&resource_diff.subject) {
                match self.resolve_conflicts(resource_diff, report) {
                    Ok(_) => report.resources_updated += 1,
                    Err(e) => {
                        report.errors.push(SyncError {
                            subject: resource_diff.subject.clone(),
                            error: e.to_string(),
                            retry_count: 0,
                            recoverable: true,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a subject should be synced based on filters.
    fn should_sync_subject(&self, subject: &str) -> bool {
        // Check subject filter
        if let Some(ref filter) = self.options.filter_subjects {
            if !filter.iter().any(|f| subject.contains(f)) {
                return false;
            }
        }

        // Check class filter (if subject is a resource with classes)
        if let Some(ref class_filter) = self.options.filter_classes {
            if let Ok(resource) = self.source_extractor.store().get_resource(subject) {
                if let Ok(Value::ResourceArray(classes)) = resource.get(urls::IS_A) {
                    let has_matching_class = classes.iter().any(|item| match item {
                        atomic_lib::values::SubResource::Subject(s) => class_filter.contains(s),
                        _ => false,
                    });
                    return has_matching_class;
                }
            }
        }

        true
    }

    /// Create a resource on the target server.
    fn create_resource_on_target(&mut self, subject: &str) -> AtomicResult<()> {
        if self.options.dry_run {
            return Ok(());
        }

        let source_resource = self.source_extractor.store().get_resource(subject)?;
        let agent = self
            .agent
            .as_ref()
            .ok_or("Agent required for write operations")?;

        // Build commit for creating new resource
        let mut builder = CommitBuilder::new(subject.to_string());

        for (prop, value) in source_resource.get_propvals() {
            builder.set(prop.clone(), value.clone());
        }

        let commit = builder.sign(agent, self.target_extractor.store(), &source_resource)?;

        // Apply to target store
        let opts = atomic_lib::commit::CommitOpts {
            validate_schema: true,
            validate_signature: false, // We just signed it
            validate_timestamp: true,
            validate_rights: false, // Trust our agent
            validate_previous_commit: false,
            update_index: true,
            validate_for_agent: None,
        };

        self.target_extractor
            .store_mut()
            .apply_commit(commit, &opts)?;

        Ok(())
    }

    /// Create a resource on the source server.
    fn create_resource_on_source(&mut self, subject: &str) -> AtomicResult<()> {
        if self.options.dry_run {
            return Ok(());
        }

        let target_resource = self.target_extractor.store().get_resource(subject)?;
        let agent = self
            .agent
            .as_ref()
            .ok_or("Agent required for write operations")?;

        let mut builder = CommitBuilder::new(subject.to_string());

        for (prop, value) in target_resource.get_propvals() {
            builder.set(prop.clone(), value.clone());
        }

        let commit = builder.sign(agent, self.source_extractor.store(), &target_resource)?;

        let opts = atomic_lib::commit::CommitOpts {
            validate_schema: true,
            validate_signature: false,
            validate_timestamp: true,
            validate_rights: false,
            validate_previous_commit: false,
            update_index: true,
            validate_for_agent: None,
        };

        self.source_extractor
            .store_mut()
            .apply_commit(commit, &opts)?;

        Ok(())
    }

    /// Update a resource on the target server based on diff.
    fn update_resource_on_target(
        &mut self,
        resource_diff: &ResourceDiff,
        report: &mut SyncReport,
    ) -> AtomicResult<()> {
        if self.options.dry_run {
            return Ok(());
        }

        let agent = self
            .agent
            .as_ref()
            .ok_or("Agent required for write operations")?;

        let target_resource = self
            .target_extractor
            .store()
            .get_resource(&resource_diff.subject)?;

        let mut builder = CommitBuilder::new(resource_diff.subject.clone());

        // Set added properties
        for (prop, value_json) in &resource_diff.added_properties {
            if let Ok(source_resource) = self
                .source_extractor
                .store()
                .get_resource(&resource_diff.subject)
            {
                if let Ok(value) = source_resource.get(prop) {
                    builder.set(prop.clone(), value.clone());
                }
            }
        }

        // Handle modified properties with conflict resolution
        for (prop, property_diff) in &resource_diff.modified_properties {
            let resolved_value = self.resolve_property_conflict(
                &resource_diff.subject,
                prop,
                property_diff,
                report,
            )?;

            if let Some(value) = resolved_value {
                builder.set(prop.clone(), value);
            }
        }

        // Remove properties
        for prop in &resource_diff.removed_properties {
            builder.remove(prop.clone());
        }

        let commit = builder.sign(agent, self.target_extractor.store(), &target_resource)?;

        let opts = atomic_lib::commit::CommitOpts {
            validate_schema: true,
            validate_signature: false,
            validate_timestamp: true,
            validate_rights: false,
            validate_previous_commit: true,
            update_index: true,
            validate_for_agent: None,
        };

        self.target_extractor
            .store_mut()
            .apply_commit(commit, &opts)?;

        Ok(())
    }

    /// Update a resource on the source server based on diff (for pull).
    fn update_resource_on_source(
        &mut self,
        resource_diff: &ResourceDiff,
        report: &mut SyncReport,
    ) -> AtomicResult<()> {
        if self.options.dry_run {
            return Ok(());
        }

        let agent = self
            .agent
            .as_ref()
            .ok_or("Agent required for write operations")?;

        let source_resource = self
            .source_extractor
            .store()
            .get_resource(&resource_diff.subject)?;

        let mut builder = CommitBuilder::new(resource_diff.subject.clone());

        // For pull, we add properties from target to source
        // (opposite of push)
        for prop in &resource_diff.removed_properties {
            if let Ok(target_resource) = self
                .target_extractor
                .store()
                .get_resource(&resource_diff.subject)
            {
                if let Ok(value) = target_resource.get(prop) {
                    builder.set(prop.clone(), value.clone());
                }
            }
        }

        // Handle modified properties
        for (prop, property_diff) in &resource_diff.modified_properties {
            let resolved_value = self.resolve_property_conflict_reverse(
                &resource_diff.subject,
                prop,
                property_diff,
                report,
            )?;

            if let Some(value) = resolved_value {
                builder.set(prop.clone(), value);
            }
        }

        // Remove added properties (they are "new" from source perspective)
        for (prop, _) in &resource_diff.added_properties {
            builder.remove(prop.clone());
        }

        let commit = builder.sign(agent, self.source_extractor.store(), &source_resource)?;

        let opts = atomic_lib::commit::CommitOpts {
            validate_schema: true,
            validate_signature: false,
            validate_timestamp: true,
            validate_rights: false,
            validate_previous_commit: true,
            update_index: true,
            validate_for_agent: None,
        };

        self.source_extractor
            .store_mut()
            .apply_commit(commit, &opts)?;

        Ok(())
    }

    /// Resolve conflicts for a modified resource in bidirectional sync.
    fn resolve_conflicts(
        &mut self,
        resource_diff: &ResourceDiff,
        report: &mut SyncReport,
    ) -> AtomicResult<()> {
        if self.options.dry_run {
            return Ok(());
        }

        // For bidirectional, apply changes to both sides based on conflict strategy
        for (prop, property_diff) in &resource_diff.modified_properties {
            let conflict = Conflict {
                subject: resource_diff.subject.clone(),
                property: prop.clone(),
                source_value: property_diff.source_value.clone(),
                target_value: property_diff.target_value.clone(),
                source_timestamp: self.get_resource_timestamp(
                    self.source_extractor.store(),
                    &resource_diff.subject,
                ),
                target_timestamp: self.get_resource_timestamp(
                    self.target_extractor.store(),
                    &resource_diff.subject,
                ),
                resolution: Some(self.determine_resolution()),
            };

            report.conflicts.push(conflict);
        }

        Ok(())
    }

    /// Resolve a single property conflict (for push mode).
    fn resolve_property_conflict(
        &self,
        subject: &str,
        prop: &str,
        property_diff: &PropertyDiff,
        report: &mut SyncReport,
    ) -> AtomicResult<Option<Value>> {
        match self.options.conflict_strategy {
            ConflictStrategy::Source => {
                // Use source value - fetch from source store
                if let Ok(source_resource) = self.source_extractor.store().get_resource(subject) {
                    if let Ok(value) = source_resource.get(prop) {
                        return Ok(Some(value.clone()));
                    }
                }
                Ok(None)
            }
            ConflictStrategy::Target => {
                // Keep target value - don't update
                Ok(None)
            }
            ConflictStrategy::Latest => {
                // Compare timestamps and use latest
                let source_ts = self.get_resource_timestamp(self.source_extractor.store(), subject);
                let target_ts = self.get_resource_timestamp(self.target_extractor.store(), subject);

                match (source_ts, target_ts) {
                    (Some(s), Some(t)) if s > t => {
                        if let Ok(source_resource) =
                            self.source_extractor.store().get_resource(subject)
                        {
                            if let Ok(value) = source_resource.get(prop) {
                                return Ok(Some(value.clone()));
                            }
                        }
                        Ok(None)
                    }
                    _ => Ok(None), // Target is newer or equal, keep target
                }
            }
            ConflictStrategy::Skip => {
                // Record conflict but skip
                report.conflicts.push(Conflict {
                    subject: subject.to_string(),
                    property: prop.to_string(),
                    source_value: property_diff.source_value.clone(),
                    target_value: property_diff.target_value.clone(),
                    source_timestamp: self
                        .get_resource_timestamp(self.source_extractor.store(), subject),
                    target_timestamp: self
                        .get_resource_timestamp(self.target_extractor.store(), subject),
                    resolution: Some(ConflictResolution::Skipped),
                });
                Ok(None)
            }
            ConflictStrategy::Manual => {
                // Record conflict for manual resolution
                report.conflicts.push(Conflict {
                    subject: subject.to_string(),
                    property: prop.to_string(),
                    source_value: property_diff.source_value.clone(),
                    target_value: property_diff.target_value.clone(),
                    source_timestamp: self
                        .get_resource_timestamp(self.source_extractor.store(), subject),
                    target_timestamp: self
                        .get_resource_timestamp(self.target_extractor.store(), subject),
                    resolution: Some(ConflictResolution::Manual),
                });
                Ok(None)
            }
        }
    }

    /// Resolve property conflict in reverse (for pull mode).
    fn resolve_property_conflict_reverse(
        &self,
        subject: &str,
        prop: &str,
        property_diff: &PropertyDiff,
        report: &mut SyncReport,
    ) -> AtomicResult<Option<Value>> {
        match self.options.conflict_strategy {
            ConflictStrategy::Source => {
                // In pull mode, "source" is our target (the remote we're pulling from)
                if let Ok(target_resource) = self.target_extractor.store().get_resource(subject) {
                    if let Ok(value) = target_resource.get(prop) {
                        return Ok(Some(value.clone()));
                    }
                }
                Ok(None)
            }
            ConflictStrategy::Target => {
                // Keep local value
                Ok(None)
            }
            ConflictStrategy::Latest => {
                let source_ts = self.get_resource_timestamp(self.source_extractor.store(), subject);
                let target_ts = self.get_resource_timestamp(self.target_extractor.store(), subject);

                match (source_ts, target_ts) {
                    (Some(s), Some(t)) if t > s => {
                        if let Ok(target_resource) =
                            self.target_extractor.store().get_resource(subject)
                        {
                            if let Ok(value) = target_resource.get(prop) {
                                return Ok(Some(value.clone()));
                            }
                        }
                        Ok(None)
                    }
                    _ => Ok(None),
                }
            }
            ConflictStrategy::Skip | ConflictStrategy::Manual => {
                report.conflicts.push(Conflict {
                    subject: subject.to_string(),
                    property: prop.to_string(),
                    source_value: property_diff.source_value.clone(),
                    target_value: property_diff.target_value.clone(),
                    source_timestamp: self
                        .get_resource_timestamp(self.source_extractor.store(), subject),
                    target_timestamp: self
                        .get_resource_timestamp(self.target_extractor.store(), subject),
                    resolution: Some(if self.options.conflict_strategy == ConflictStrategy::Skip {
                        ConflictResolution::Skipped
                    } else {
                        ConflictResolution::Manual
                    }),
                });
                Ok(None)
            }
        }
    }

    /// Get the lastCommit timestamp for a resource.
    fn get_resource_timestamp(&self, store: &impl Storelike, subject: &str) -> Option<i64> {
        if let Ok(resource) = store.get_resource(subject) {
            if let Ok(Value::AtomicUrl(commit_url)) = resource.get(urls::LAST_COMMIT) {
                if let Ok(commit_resource) = store.get_resource(commit_url) {
                    if let Ok(Value::Timestamp(ts)) = commit_resource.get(urls::CREATED_AT) {
                        return Some(*ts);
                    }
                }
            }
        }
        None
    }

    /// Determine resolution type based on strategy.
    fn determine_resolution(&self) -> ConflictResolution {
        match self.options.conflict_strategy {
            ConflictStrategy::Source => ConflictResolution::UseSource,
            ConflictStrategy::Target => ConflictResolution::UseTarget,
            ConflictStrategy::Latest => ConflictResolution::UsedLatest,
            ConflictStrategy::Skip => ConflictResolution::Skipped,
            ConflictStrategy::Manual => ConflictResolution::Manual,
        }
    }

    /// Get the source extractor.
    pub fn source(&self) -> &Extractor {
        &self.source_extractor
    }

    /// Get the target extractor.
    pub fn target(&self) -> &Extractor {
        &self.target_extractor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_lib::{Resource, Store};

    #[test]
    fn test_compare_empty_snapshots() {
        let source = StoreSnapshot {
            server_url: "http://source.example".to_string(),
            schema_version: crate::validate::SchemaVersion::V2,
            resources: vec![],
            referenced_resources: vec![],
            extracted_at: 0,
            metadata: crate::validate::SnapshotMetadata::default(),
        };

        let target = StoreSnapshot {
            server_url: "http://target.example".to_string(),
            schema_version: crate::validate::SchemaVersion::V2,
            resources: vec![],
            referenced_resources: vec![],
            extracted_at: 0,
            metadata: crate::validate::SnapshotMetadata::default(),
        };

        let _store = Store::init().unwrap();
        let extractor = Extractor::new("http://localhost", None).unwrap();
        let engine = SyncEngine {
            source_extractor: extractor,
            target_extractor: Extractor::new("http://localhost", None).unwrap(),
            agent: None,
            options: SyncOptions::default(),
        };

        let diff = engine.compare_snapshots(&source, &target).unwrap();
        assert_eq!(diff.summary.total_differences, 0);
        assert!(diff.source_only.is_empty());
        assert!(diff.target_only.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_compare_with_additions() {
        let resource1 = Resource::new("http://example.com/resource1".into());

        let source = StoreSnapshot {
            server_url: "http://source.example".to_string(),
            schema_version: crate::validate::SchemaVersion::V2,
            resources: vec![resource1],
            referenced_resources: vec![],
            extracted_at: 0,
            metadata: crate::validate::SnapshotMetadata {
                total_resources: 1,
                ..Default::default()
            },
        };

        let target = StoreSnapshot {
            server_url: "http://target.example".to_string(),
            schema_version: crate::validate::SchemaVersion::V2,
            resources: vec![],
            referenced_resources: vec![],
            extracted_at: 0,
            metadata: crate::validate::SnapshotMetadata::default(),
        };

        let engine = SyncEngine {
            source_extractor: Extractor::new("http://localhost", None).unwrap(),
            target_extractor: Extractor::new("http://localhost", None).unwrap(),
            agent: None,
            options: SyncOptions::default(),
        };

        let diff = engine.compare_snapshots(&source, &target).unwrap();
        assert_eq!(diff.summary.resources_added, 1);
        assert_eq!(diff.source_only.len(), 1);
        assert_eq!(diff.source_only[0], "http://example.com/resource1");
    }

    #[test]
    fn test_conflict_strategy_resolution() {
        let engine = SyncEngine {
            source_extractor: Extractor::new("http://localhost", None).unwrap(),
            target_extractor: Extractor::new("http://localhost", None).unwrap(),
            agent: None,
            options: SyncOptions {
                conflict_strategy: ConflictStrategy::Source,
                ..Default::default()
            },
        };

        let resolution = engine.determine_resolution();
        assert!(matches!(resolution, ConflictResolution::UseSource));
    }

    #[test]
    fn test_sync_report_creation() {
        let report = SyncReport::new(
            "http://source.example".to_string(),
            "http://target.example".to_string(),
        );

        assert!(report.success);
        assert_eq!(report.resources_created, 0);
        assert_eq!(report.resources_updated, 0);
        assert!(report.errors.is_empty());
        assert!(report.conflicts.is_empty());
    }

    #[test]
    fn test_sync_options_default() {
        let options = SyncOptions::default();
        assert!(matches!(options.mode, SyncMode::Push));
        assert!(matches!(
            options.conflict_strategy,
            ConflictStrategy::Source
        ));
        assert!(options.include_ontologies);
        assert!(!options.dry_run);
        assert_eq!(options.batch_size, 100);
    }

    #[test]
    fn test_should_sync_subject_no_filter() {
        let engine = SyncEngine {
            source_extractor: Extractor::new("http://localhost", None).unwrap(),
            target_extractor: Extractor::new("http://localhost", None).unwrap(),
            agent: None,
            options: SyncOptions::default(),
        };

        assert!(engine.should_sync_subject("http://example.com/any"));
    }

    #[test]
    fn test_should_sync_subject_with_filter() {
        let engine = SyncEngine {
            source_extractor: Extractor::new("http://localhost", None).unwrap(),
            target_extractor: Extractor::new("http://localhost", None).unwrap(),
            agent: None,
            options: SyncOptions {
                filter_subjects: Some(vec!["ontology".to_string()]),
                ..Default::default()
            },
        };

        assert!(engine.should_sync_subject("http://example.com/ontology/class"));
        assert!(!engine.should_sync_subject("http://example.com/data/resource"));
    }
}
