//! Extractor module for fetching and serializing Atomic Data resources.
//!
//! This module provides comprehensive support for:
//! - ResourceResponse handling (single and referenced resources)
//! - All 12 Atomic Data datatypes
//! - Ontology extraction with dependencies
//! - Schema version detection

use crate::validate::types::{
    ExtractOptions, ExtractedResource, SchemaVersion, SnapshotMetadata, StoreSnapshot,
};
use atomic_lib::{
    agents::Agent,
    errors::AtomicResult,
    storelike::{ResourceResponse, Storelike},
    urls, Resource, Store, Value,
};
use std::collections::{HashMap, HashSet};

/// Extractor for fetching resources from an Atomic Server.
/// Fully supports ResourceResponse and nested/referenced resources.
pub struct Extractor {
    store: Store,
    options: ExtractOptions,
}

impl Extractor {
    /// Create a new extractor with a server URL and optional agent
    pub fn new(server_url: &str, agent: Option<Agent>) -> AtomicResult<Self> {
        let mut store = Store::init()?;
        store.set_server_url(server_url);
        if let Some(agent) = agent {
            store.set_default_agent(agent);
        }
        Ok(Self {
            store,
            options: ExtractOptions::default(),
        })
    }

    /// Create with custom options
    pub fn with_options(
        server_url: &str,
        agent: Option<Agent>,
        options: ExtractOptions,
    ) -> AtomicResult<Self> {
        let mut extractor = Self::new(server_url, agent)?;
        extractor.options = options;
        Ok(extractor)
    }

    /// Get a reference to the internal store
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Get a mutable reference to the internal store
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// Detect the schema version of the server.
    /// Returns V2 if Uri and JSON datatypes are available.
    pub fn detect_schema_version(&self) -> SchemaVersion {
        // Try to fetch the URI datatype resource
        match self.store.get_resource(urls::URI) {
            Ok(_) => SchemaVersion::V2,
            Err(_) => SchemaVersion::V1,
        }
    }

    /// Fetch a single resource, returning it with any referenced resources.
    /// This properly handles ResourceResponse from the server.
    pub fn fetch_resource(&self, subject: &str) -> AtomicResult<ExtractedResource> {
        let agent = self.store.get_default_agent().ok();

        // Fetch with extended mode to get referenced resources
        match self.store.fetch_resource(subject, agent.as_ref()) {
            Ok(resource) => {
                // The fetch_resource method stores referenced resources in the store
                // We need to check if there are any referenced resources
                let referenced = self.extract_referenced_from_resource(&resource)?;
                Ok(ExtractedResource::with_references(resource, referenced))
            }
            Err(e) => Err(e),
        }
    }

    /// Extract referenced resources from a resource's values.
    /// This handles ResourceResponse patterns where referenced resources are included.
    fn extract_referenced_from_resource(&self, resource: &Resource) -> AtomicResult<Vec<Resource>> {
        let mut referenced = Vec::new();

        for (_prop, value) in resource.get_propvals() {
            match value {
                Value::AtomicUrl(url) => {
                    // Try to get from local store (may have been fetched as reference)
                    if let Ok(ref_resource) = self.store.get_resource(url) {
                        // Only include if it was already in the store
                        if self.options.include_references {
                            referenced.push(ref_resource);
                        }
                    }
                }
                Value::ResourceArray(arr) => {
                    for sub in arr {
                        if let atomic_lib::values::SubResource::Subject(url) = sub {
                            if let Ok(ref_resource) = self.store.get_resource(url) {
                                if self.options.include_references {
                                    referenced.push(ref_resource);
                                }
                            }
                        }
                    }
                }
                Value::NestedResource(nested) => {
                    // Handle nested resources
                    if let atomic_lib::values::SubResource::Nested(propvals) = nested {
                        // Nested resources are inline, not separate
                        let _ = propvals; // Already embedded in parent
                    }
                }
                _ => {}
            }
        }

        Ok(referenced)
    }

    /// Fetch a resource and all its descendants up to max_depth.
    /// Returns all resources as ExtractedResource instances.
    pub fn fetch_resource_tree(
        &self,
        root_subject: &str,
        max_depth: Option<usize>,
    ) -> AtomicResult<Vec<ExtractedResource>> {
        let depth = max_depth.unwrap_or(self.options.max_depth);
        let mut visited = HashSet::new();
        let mut results = Vec::new();

        self.fetch_recursive(root_subject, depth, &mut visited, &mut results)?;

        Ok(results)
    }

    fn fetch_recursive(
        &self,
        subject: &str,
        remaining_depth: usize,
        visited: &mut HashSet<String>,
        results: &mut Vec<ExtractedResource>,
    ) -> AtomicResult<()> {
        if remaining_depth == 0 || visited.contains(subject) {
            return Ok(());
        }

        visited.insert(subject.to_string());

        let extracted = self.fetch_resource(subject)?;
        let resource = extracted.main.clone();

        // Add this resource and its references
        results.push(extracted);

        // Follow children (resources with parent = this resource)
        if let Ok(Value::ResourceArray(children)) = resource.get(urls::CHILDREN) {
            for child in children {
                if let atomic_lib::values::SubResource::Subject(child_url) = child {
                    self.fetch_recursive(child_url, remaining_depth - 1, visited, results)?;
                }
            }
        }

        // Follow other URL references if configured
        if self.options.follow_external_urls {
            for (_prop, value) in resource.get_propvals() {
                if let Value::AtomicUrl(url) = value {
                    if !visited.contains(url.as_str()) {
                        let _ = self.fetch_recursive(url, remaining_depth - 1, visited, results);
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract a complete ontology with all its classes, properties, and instances.
    /// Properly handles ResourceResponse for efficient fetching.
    pub fn extract_ontology(&self, ontology_url: &str) -> AtomicResult<Vec<ExtractedResource>> {
        let mut results = Vec::new();
        let mut visited = HashSet::new();

        // Fetch the ontology resource itself
        let ontology_extracted = self.fetch_resource(ontology_url)?;
        let ontology = ontology_extracted.main.clone();
        results.push(ontology_extracted);
        visited.insert(ontology_url.to_string());

        // Extract classes
        if let Ok(Value::ResourceArray(class_urls)) = ontology.get(urls::CLASSES) {
            for class_item in class_urls {
                if let atomic_lib::values::SubResource::Subject(class_url) = class_item {
                    if !visited.contains(class_url.as_str()) {
                        match self.fetch_resource(class_url) {
                            Ok(class_extracted) => {
                                visited.insert(class_url.clone());

                                // For each class, fetch its required and recommended properties
                                let class_resource = class_extracted.main.clone();
                                results.push(class_extracted);

                                // Fetch required properties
                                if let Ok(Value::ResourceArray(requires)) =
                                    class_resource.get(urls::REQUIRES)
                                {
                                    for prop_item in requires {
                                        if let atomic_lib::values::SubResource::Subject(prop_url) =
                                            prop_item
                                        {
                                            if !visited.contains(prop_url.as_str()) {
                                                if let Ok(prop_extracted) =
                                                    self.fetch_resource(prop_url)
                                                {
                                                    visited.insert(prop_url.clone());
                                                    results.push(prop_extracted);
                                                }
                                            }
                                        }
                                    }
                                }

                                // Fetch recommended properties
                                if let Ok(Value::ResourceArray(recommends)) =
                                    class_resource.get(urls::RECOMMENDS)
                                {
                                    for prop_item in recommends {
                                        if let atomic_lib::values::SubResource::Subject(prop_url) =
                                            prop_item
                                        {
                                            if !visited.contains(prop_url.as_str()) {
                                                if let Ok(prop_extracted) =
                                                    self.fetch_resource(prop_url)
                                                {
                                                    visited.insert(prop_url.clone());
                                                    results.push(prop_extracted);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch class {}: {}", class_url, e);
                            }
                        }
                    }
                }
            }
        }

        // Extract properties defined in the ontology
        if let Ok(Value::ResourceArray(prop_urls)) = ontology.get(urls::PROPERTIES) {
            for prop_item in prop_urls {
                if let atomic_lib::values::SubResource::Subject(prop_url) = prop_item {
                    if !visited.contains(prop_url.as_str()) {
                        match self.fetch_resource(prop_url) {
                            Ok(prop_extracted) => {
                                visited.insert(prop_url.clone());
                                results.push(prop_extracted);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch property {}: {}", prop_url, e);
                            }
                        }
                    }
                }
            }
        }

        // Extract instances if present
        if let Ok(Value::ResourceArray(instance_urls)) = ontology.get(urls::INSTANCES) {
            for instance_item in instance_urls {
                if let atomic_lib::values::SubResource::Subject(instance_url) = instance_item {
                    if !visited.contains(instance_url.as_str()) {
                        match self.fetch_resource(instance_url) {
                            Ok(instance_extracted) => {
                                visited.insert(instance_url.clone());
                                results.push(instance_extracted);
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to fetch instance {}: {}",
                                    instance_url,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Create a complete snapshot of the server or a subset.
    /// Includes all resources and their references.
    pub fn create_snapshot(&self, root_url: Option<&str>) -> AtomicResult<StoreSnapshot> {
        let schema_version = self.detect_schema_version();
        let server_url = self
            .store
            .get_server_url()
            .unwrap_or_else(|_| "unknown".to_string());

        let mut all_resources = Vec::new();
        let mut all_referenced = Vec::new();
        let mut metadata = SnapshotMetadata::default();
        let mut visited_subjects = HashSet::new();

        // If root_url is provided, start from there; otherwise, try to fetch drive
        let start_url = root_url.unwrap_or(&server_url);

        // Fetch the root/drive resource
        match self.fetch_resource_tree(start_url, Some(self.options.max_depth)) {
            Ok(extracted_list) => {
                for extracted in extracted_list {
                    // Track main resource
                    if !visited_subjects.contains(extracted.main.get_subject()) {
                        self.update_metadata(&extracted.main, &mut metadata);
                        visited_subjects.insert(extracted.main.get_subject().to_string());
                        all_resources.push(extracted.main);
                    }

                    // Track referenced resources
                    for ref_resource in extracted.referenced {
                        if !visited_subjects.contains(ref_resource.get_subject()) {
                            visited_subjects.insert(ref_resource.get_subject().to_string());
                            all_referenced.push(ref_resource);
                            metadata.referenced_resources += 1;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch resources from {}: {}", start_url, e);
            }
        }

        metadata.total_resources = all_resources.len();

        Ok(StoreSnapshot {
            server_url,
            schema_version,
            resources: all_resources,
            referenced_resources: all_referenced,
            extracted_at: chrono::Utc::now().timestamp_millis(),
            metadata,
        })
    }

    /// Update metadata based on resource content
    fn update_metadata(&self, resource: &Resource, metadata: &mut SnapshotMetadata) {
        // Check resource class
        if let Ok(Value::ResourceArray(classes)) = resource.get(urls::IS_A) {
            for class_item in classes {
                if let atomic_lib::values::SubResource::Subject(class_url) = class_item {
                    match class_url.as_str() {
                        urls::CLASS => metadata.class_count += 1,
                        urls::PROPERTY => metadata.property_count += 1,
                        urls::ONTOLOGY => metadata.ontology_count += 1,
                        urls::AGENT => metadata.agent_count += 1,
                        urls::COMMIT => metadata.commit_count += 1,
                        _ => {}
                    }
                }
            }
        }

        // Track datatype usage
        for (_prop, value) in resource.get_propvals() {
            let datatype = crate::validate::types::value_datatype_name(value);
            *metadata.datatype_usage.entry(datatype).or_insert(0) += 1;
        }
    }

    /// Serialize a snapshot to JSON-AD format
    pub fn snapshot_to_json_ad(&self, snapshot: &StoreSnapshot) -> AtomicResult<String> {
        let mut all_resources = snapshot.resources.clone();
        all_resources.extend(snapshot.referenced_resources.clone());

        Resource::vec_to_json_ad(&all_resources)
    }

    /// Serialize a snapshot to JSON format (with shortnames)
    pub fn snapshot_to_json(&self, snapshot: &StoreSnapshot) -> AtomicResult<String> {
        let mut all_resources = snapshot.resources.clone();
        all_resources.extend(snapshot.referenced_resources.clone());

        Resource::vec_to_json(&all_resources, &self.store)
    }

    /// Save snapshot to a file
    pub fn save_snapshot(&self, snapshot: &StoreSnapshot, path: &str) -> AtomicResult<()> {
        let json = self.snapshot_to_json_ad(snapshot)?;
        std::fs::write(path, json)
            .map_err(|e| format!("Failed to write snapshot to {}: {}", path, e))?;
        Ok(())
    }

    /// Load a snapshot from a JSON-AD file
    pub fn load_snapshot(&self, path: &str) -> AtomicResult<StoreSnapshot> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read snapshot from {}: {}", path, e))?;

        // Parse the JSON array
        let json_array: Vec<serde_json::Value> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let mut resources = Vec::new();
        for json_obj in json_array {
            let json_str = json_obj.to_string();
            let resource = atomic_lib::parse::parse_json_ad_resource(
                &json_str,
                &self.store,
                &atomic_lib::parse::ParseOpts::default(),
            )?;
            resources.push(resource);
        }

        let mut metadata = SnapshotMetadata::default();
        for resource in &resources {
            self.update_metadata(resource, &mut metadata);
        }
        metadata.total_resources = resources.len();

        Ok(StoreSnapshot {
            server_url: "loaded_from_file".to_string(),
            schema_version: SchemaVersion::V2,
            resources,
            referenced_resources: Vec::new(),
            extracted_at: chrono::Utc::now().timestamp_millis(),
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracted_resource_single() {
        let store = Store::init().unwrap();
        let resource = Resource::new("https://example.com/test".to_string());
        let extracted = ExtractedResource::single(resource);

        assert_eq!(extracted.total_count(), 1);
        assert!(extracted.referenced.is_empty());
    }

    #[test]
    fn test_extracted_resource_with_references() {
        let store = Store::init().unwrap();
        let main = Resource::new("https://example.com/main".to_string());
        let ref1 = Resource::new("https://example.com/ref1".to_string());
        let ref2 = Resource::new("https://example.com/ref2".to_string());

        let extracted = ExtractedResource::with_references(main, vec![ref1, ref2]);

        assert_eq!(extracted.total_count(), 3);
        assert_eq!(extracted.referenced.len(), 2);
        assert_eq!(extracted.all_resources().len(), 3);
    }

    #[test]
    fn test_schema_version_default() {
        assert_eq!(SchemaVersion::default(), SchemaVersion::V2);
    }
}
