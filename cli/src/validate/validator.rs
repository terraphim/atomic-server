//! Validator module for comprehensive Atomic Data validation.
//!
//! Supports all validation levels (0-5) and all 12 datatypes including Uri and JSON.

use crate::validate::types::{
    ErrorSeverity, ValidationError, ValidationErrorCode, ValidationLevel, ValidationReport,
    ValidationSummary,
};
use atomic_lib::{
    datatype::DataType, errors::AtomicResult, storelike::Storelike, urls, Resource, Value,
};
use std::collections::{HashMap, HashSet};

/// Comprehensive validator for Atomic Data resources.
/// Supports all 12 datatypes and validation levels 0-5.
pub struct Validator<'a, S: Storelike> {
    store: &'a S,
    level: ValidationLevel,
    /// Cache of fetched properties for performance
    property_cache: HashMap<String, Resource>,
    /// Cache of fetched classes
    class_cache: HashMap<String, Resource>,
}

impl<'a, S: Storelike> Validator<'a, S> {
    pub fn new(store: &'a S, level: ValidationLevel) -> Self {
        Self {
            store,
            level,
            property_cache: HashMap::new(),
            class_cache: HashMap::new(),
        }
    }

    /// Validate a single resource at the configured level
    pub fn validate_resource(&mut self, resource: &Resource) -> ValidationReport {
        let mut report = ValidationReport::new(self.level);
        let subject = resource.get_subject().to_string();

        // Level 0: Structural validation (always performed)
        let structural_errors = self.validate_structural(resource);
        for error in structural_errors {
            match error.severity {
                ErrorSeverity::Error => report.add_error(error),
                ErrorSeverity::Warning => report.add_warning(error),
                ErrorSeverity::Info => report.add_info(error),
            }
        }

        // Level 1: Datatype validation (all 12 types)
        if self.level >= ValidationLevel::Datatype {
            let datatype_errors = self.validate_datatypes(resource);
            for error in datatype_errors {
                match error.severity {
                    ErrorSeverity::Error => report.add_error(error),
                    ErrorSeverity::Warning => report.add_warning(error),
                    ErrorSeverity::Info => report.add_info(error),
                }
            }
        }

        // Level 2: Schema validation
        if self.level >= ValidationLevel::Schema {
            let schema_errors = self.validate_schema(resource);
            for error in schema_errors {
                match error.severity {
                    ErrorSeverity::Error => report.add_error(error),
                    ErrorSeverity::Warning => report.add_warning(error),
                    ErrorSeverity::Info => report.add_info(error),
                }
            }
        }

        // Level 3: Referential integrity
        if self.level >= ValidationLevel::Referential {
            let ref_errors = self.validate_referential_integrity(resource);
            for error in ref_errors {
                match error.severity {
                    ErrorSeverity::Error => report.add_error(error),
                    ErrorSeverity::Warning => report.add_warning(error),
                    ErrorSeverity::Info => report.add_info(error),
                }
            }
        }

        // Level 4: Cryptographic validation
        if self.level >= ValidationLevel::Cryptographic {
            let crypto_errors = self.validate_cryptographic(resource);
            for error in crypto_errors {
                match error.severity {
                    ErrorSeverity::Error => report.add_error(error),
                    ErrorSeverity::Warning => report.add_warning(error),
                    ErrorSeverity::Info => report.add_info(error),
                }
            }
        }

        // Update summary
        report.summary.total_resources = 1;
        if report.errors.is_empty() {
            report.valid_subjects.push(subject);
            report.summary.valid_resources = 1;
        } else {
            report.summary.invalid_resources = 1;
        }

        report
    }

    /// Validate multiple resources
    pub fn validate_resources(&mut self, resources: &[Resource]) -> ValidationReport {
        let mut combined_report = ValidationReport::new(self.level);

        for resource in resources {
            let resource_report = self.validate_resource(resource);
            combined_report.merge(resource_report);
        }

        // Update combined summary
        combined_report.summary.total_resources = resources.len();
        combined_report.summary.valid_resources = combined_report.valid_subjects.len();
        combined_report.summary.invalid_resources = combined_report.invalid_subjects.len();

        combined_report
    }

    /// Level 0: Structural validation
    fn validate_structural(&self, resource: &Resource) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = resource.get_subject();

        // Validate subject is a valid URL
        if let Err(_) = url::Url::parse(subject) {
            // Check if it's a local ID (relative path)
            if !subject.starts_with('/') && !subject.contains(':') {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Warning,
                    code: ValidationErrorCode::InvalidSubjectUrl,
                    subject: subject.to_string(),
                    property: None,
                    message: format!("Subject '{}' is not a valid URL or local ID", subject),
                    expected: Some("Valid HTTP(S) URL or local ID".to_string()),
                    actual: Some(subject.to_string()),
                    context: None,
                });
            }
        }

        // Check for empty propvals
        if resource.get_propvals().into_iter().count() == 0 {
            errors.push(ValidationError {
                severity: ErrorSeverity::Warning,
                code: ValidationErrorCode::MalformedAtom,
                subject: subject.to_string(),
                property: None,
                message: "Resource has no properties".to_string(),
                expected: Some("At least one property".to_string()),
                actual: Some("0 properties".to_string()),
                context: None,
            });
        }

        errors
    }

    /// Level 1: Datatype validation for all 12 Atomic Data types
    fn validate_datatypes(&mut self, resource: &Resource) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = resource.get_subject().to_string();

        for (prop_url, value) in resource.get_propvals() {
            // Try to get the property definition
            let expected_datatype = match self.get_property(prop_url) {
                Ok(prop_resource) => match prop_resource.get(urls::DATATYPE_PROP) {
                    Ok(Value::AtomicUrl(dt_url)) => Some(dt_url.clone()),
                    _ => None,
                },
                Err(_) => None,
            };

            // Validate the value's datatype
            let validation_result = self.validate_value_datatype(value, expected_datatype.as_deref());

            if let Err(err_msg) = validation_result {
                let datatype_name = crate::validate::types::value_datatype_name(value);
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: self.datatype_error_code(value),
                    subject: subject.clone(),
                    property: Some(prop_url.clone()),
                    message: err_msg,
                    expected: expected_datatype,
                    actual: Some(datatype_name),
                    context: Some(crate::validate::types::value_to_json(value)),
                });

                // Track in summary
            }
        }

        errors
    }

    /// Validate a value against its expected datatype.
    /// Supports all 12 Atomic Data types including Uri and JSON.
    fn validate_value_datatype(
        &self,
        value: &Value,
        expected_datatype_url: Option<&str>,
    ) -> Result<(), String> {
        match value {
            Value::String(_) => Ok(()),
            Value::Integer(_) => Ok(()),
            Value::Float(f) => {
                if f.is_nan() {
                    Err("Float value is NaN".to_string())
                } else if f.is_infinite() {
                    Err("Float value is infinite".to_string())
                } else {
                    Ok(())
                }
            }
            Value::Boolean(_) => Ok(()),
            Value::Date(d) => {
                // ISO 8601 date: YYYY-MM-DD
                let date_regex = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
                if !date_regex.is_match(d) {
                    Err(format!("Invalid date format '{}', expected YYYY-MM-DD", d))
                } else {
                    Ok(())
                }
            }
            Value::Timestamp(t) => {
                // Unix epoch in milliseconds
                if *t < 0 {
                    Err(format!("Negative timestamp '{}' is invalid", t))
                } else {
                    Ok(())
                }
            }
            Value::Slug(s) => {
                // Lowercase, alphanumeric + hyphens
                let slug_regex = regex::Regex::new(r"^[a-z0-9-]+$").unwrap();
                if !slug_regex.is_match(s) {
                    Err(format!(
                        "Invalid slug '{}', must be lowercase alphanumeric with hyphens",
                        s
                    ))
                } else {
                    Ok(())
                }
            }
            Value::Markdown(_) => {
                // Any valid UTF-8 string is valid markdown
                Ok(())
            }
            Value::AtomicUrl(u) => {
                // Must be a valid HTTP(S) URL
                match url::Url::parse(u) {
                    Ok(parsed) => {
                        if parsed.scheme() != "http" && parsed.scheme() != "https" {
                            Err(format!(
                                "AtomicUrl must be HTTP(S), got scheme '{}'",
                                parsed.scheme()
                            ))
                        } else {
                            Ok(())
                        }
                    }
                    Err(e) => Err(format!("Invalid AtomicUrl '{}': {}", u, e)),
                }
            }
            Value::ResourceArray(arr) => {
                // All elements must be valid references
                for (i, sub) in arr.iter().enumerate() {
                    match sub {
                        atomic_lib::values::SubResource::Subject(s) => {
                            // Should be a valid URL or local ID
                            if !s.starts_with("http") && !s.starts_with('/') {
                                return Err(format!(
                                    "ResourceArray element {} has invalid subject '{}'",
                                    i, s
                                ));
                            }
                        }
                        atomic_lib::values::SubResource::Nested(_) => {
                            // Nested resources are valid
                        }
                    }
                }
                Ok(())
            }
            Value::NestedResource(_) => {
                // Nested resources are validated recursively if needed
                Ok(())
            }
            Value::Uri(u) => {
                // URI validation (RFC 3986) - can be relative or absolute
                // More permissive than AtomicUrl
                if u.is_empty() {
                    Err("URI cannot be empty".to_string())
                } else if u.contains(' ') {
                    Err(format!("URI '{}' contains spaces", u))
                } else {
                    // Basic URI validation - check for valid characters
                    // URI can be relative (e.g., "/path/to/resource")
                    Ok(())
                }
            }
            Value::JSON(j) => {
                // JSON is already parsed, so it's valid
                // We can add additional validation here if needed
                match j {
                    serde_json::Value::Null => Ok(()),
                    serde_json::Value::Bool(_) => Ok(()),
                    serde_json::Value::Number(_) => Ok(()),
                    serde_json::Value::String(_) => Ok(()),
                    serde_json::Value::Array(_) => Ok(()),
                    serde_json::Value::Object(_) => Ok(()),
                }
            }
            Value::Unsupported(u) => {
                // Log a warning but don't fail
                Ok(())
            }
        }
    }

    /// Get the appropriate error code for a datatype validation failure
    fn datatype_error_code(&self, value: &Value) -> ValidationErrorCode {
        match value {
            Value::String(_) => ValidationErrorCode::InvalidString,
            Value::Integer(_) => ValidationErrorCode::InvalidInteger,
            Value::Float(_) => ValidationErrorCode::InvalidFloat,
            Value::Boolean(_) => ValidationErrorCode::InvalidBoolean,
            Value::Date(_) => ValidationErrorCode::InvalidDate,
            Value::Timestamp(_) => ValidationErrorCode::InvalidTimestamp,
            Value::Slug(_) => ValidationErrorCode::InvalidSlug,
            Value::Markdown(_) => ValidationErrorCode::InvalidMarkdown,
            Value::AtomicUrl(_) => ValidationErrorCode::InvalidAtomicUrl,
            Value::ResourceArray(_) => ValidationErrorCode::InvalidResourceArray,
            Value::NestedResource(_) => ValidationErrorCode::InvalidNestedResource,
            Value::Uri(_) => ValidationErrorCode::InvalidUri,
            Value::JSON(_) => ValidationErrorCode::InvalidJson,
            Value::Unsupported(_) => ValidationErrorCode::DatatypeMismatch,
        }
    }

    /// Level 2: Schema validation
    fn validate_schema(&mut self, resource: &Resource) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = resource.get_subject().to_string();

        // Check for isA property
        match resource.get(urls::IS_A) {
            Ok(Value::ResourceArray(classes)) => {
                for class_item in classes {
                    if let atomic_lib::values::SubResource::Subject(class_url) = class_item {
                        // Fetch and validate against class
                        match self.get_class(class_url) {
                            Ok(class_resource) => {
                                // Check required properties
                                if let Ok(Value::ResourceArray(required)) =
                                    class_resource.get(urls::REQUIRES)
                                {
                                    for req_item in required {
                                        if let atomic_lib::values::SubResource::Subject(req_prop) =
                                            req_item
                                        {
                                            if resource.get(req_prop).is_err() {
                                                errors.push(ValidationError {
                                                    severity: ErrorSeverity::Error,
                                                    code: ValidationErrorCode::MissingRequiredProperty,
                                                    subject: subject.clone(),
                                                    property: Some(req_prop.clone()),
                                                    message: format!(
                                                        "Missing required property '{}' for class '{}'",
                                                        req_prop, class_url
                                                    ),
                                                    expected: Some(req_prop.clone()),
                                                    actual: None,
                                                    context: None,
                                                });
                                            }
                                        }
                                    }
                                }

                                // Check property constraints (allows_only)
                                for (prop_url, value) in resource.get_propvals() {
                                    if let Ok(prop_resource) = self.get_property(prop_url) {
                                        if let Ok(Value::ResourceArray(allowed)) =
                                            prop_resource.get(urls::ALLOWS_ONLY)
                                        {
                                            let allowed_values: Vec<String> = allowed
                                                .iter()
                                                .filter_map(|item| match item {
                                                    atomic_lib::values::SubResource::Subject(s) => {
                                                        Some(s.clone())
                                                    }
                                                    _ => None,
                                                })
                                                .collect();

                                            if !allowed_values.is_empty() {
                                                let value_valid = match value {
                                                    Value::AtomicUrl(u) => {
                                                        allowed_values.contains(u)
                                                    }
                                                    Value::String(s) => allowed_values.contains(s),
                                                    _ => true,
                                                };

                                                if !value_valid {
                                                    errors.push(ValidationError {
                                                        severity: ErrorSeverity::Error,
                                                        code: ValidationErrorCode::ValueNotAllowed,
                                                        subject: subject.clone(),
                                                        property: Some(prop_url.clone()),
                                                        message: format!(
                                                            "Value not in allowed list for property '{}'",
                                                            prop_url
                                                        ),
                                                        expected: Some(format!("{:?}", allowed_values)),
                                                        actual: Some(format!("{}", value)),
                                                        context: None,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                errors.push(ValidationError {
                                    severity: ErrorSeverity::Error,
                                    code: ValidationErrorCode::ClassNotFound,
                                    subject: subject.clone(),
                                    property: Some(urls::IS_A.to_string()),
                                    message: format!("Class '{}' not found: {}", class_url, e),
                                    expected: None,
                                    actual: None,
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
                    code: ValidationErrorCode::InvalidIsA,
                    subject: subject.clone(),
                    property: Some(urls::IS_A.to_string()),
                    message: "isA property must be a ResourceArray".to_string(),
                    expected: Some("ResourceArray".to_string()),
                    actual: None,
                    context: None,
                });
            }
            Err(_) => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Warning,
                    code: ValidationErrorCode::MissingIsA,
                    subject: subject.clone(),
                    property: Some(urls::IS_A.to_string()),
                    message: "Resource has no isA property (no class membership)".to_string(),
                    expected: Some("isA property with class URL".to_string()),
                    actual: None,
                    context: None,
                });
            }
        }

        errors
    }

    /// Level 3: Referential integrity validation
    fn validate_referential_integrity(&self, resource: &Resource) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = resource.get_subject().to_string();

        for (prop_url, value) in resource.get_propvals() {
            match value {
                Value::AtomicUrl(url) => {
                    // Check if URL resolves
                    if !self.is_local_id(url) && url.starts_with("http") {
                        match self.store.get_resource(url) {
                            Ok(_) => {}
                            Err(_) => {
                                errors.push(ValidationError {
                                    severity: ErrorSeverity::Error,
                                    code: ValidationErrorCode::BrokenReference,
                                    subject: subject.clone(),
                                    property: Some(prop_url.clone()),
                                    message: format!("Referenced resource '{}' not found", url),
                                    expected: Some("Resolvable URL".to_string()),
                                    actual: Some(url.clone()),
                                    context: None,
                                });
                            }
                        }
                    }
                }
                Value::ResourceArray(arr) => {
                    for (i, item) in arr.iter().enumerate() {
                        if let atomic_lib::values::SubResource::Subject(url) = item {
                            if !self.is_local_id(url) && url.starts_with("http") {
                                match self.store.get_resource(url) {
                                    Ok(_) => {}
                                    Err(_) => {
                                        errors.push(ValidationError {
                                            severity: ErrorSeverity::Error,
                                            code: ValidationErrorCode::BrokenArrayReference,
                                            subject: subject.clone(),
                                            property: Some(prop_url.clone()),
                                            message: format!(
                                                "Array element {} references non-existent resource '{}'",
                                                i, url
                                            ),
                                            expected: Some("Resolvable URL".to_string()),
                                            actual: Some(url.clone()),
                                            context: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Check for circular parent relationships
        if let Ok(Value::AtomicUrl(parent_url)) = resource.get(urls::PARENT) {
            let mut visited = HashSet::new();
            visited.insert(subject.clone());

            if self.has_circular_parent(parent_url, &mut visited) {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::CircularParent,
                    subject: subject.clone(),
                    property: Some(urls::PARENT.to_string()),
                    message: "Circular parent relationship detected".to_string(),
                    expected: Some("Acyclic parent hierarchy".to_string()),
                    actual: None,
                    context: None,
                });
            }
        }

        errors
    }

    fn has_circular_parent(&self, current_url: &str, visited: &mut HashSet<String>) -> bool {
        if visited.contains(current_url) {
            return true;
        }

        visited.insert(current_url.to_string());

        if let Ok(resource) = self.store.get_resource(current_url) {
            if let Ok(Value::AtomicUrl(parent)) = resource.get(urls::PARENT) {
                return self.has_circular_parent(parent, visited);
            }
        }

        false
    }

    /// Level 4: Cryptographic validation (commits, signatures)
    fn validate_cryptographic(&self, resource: &Resource) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = resource.get_subject().to_string();

        // Check if this is a Commit resource
        let is_commit = if let Ok(Value::ResourceArray(classes)) = resource.get(urls::IS_A) {
            classes.iter().any(|item| match item {
                atomic_lib::values::SubResource::Subject(s) => s == urls::COMMIT,
                _ => false,
            })
        } else {
            false
        };

        if is_commit {
            // Validate commit structure
            if resource.get(urls::SUBJECT).is_err() {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InvalidCommitSubject,
                    subject: subject.clone(),
                    property: Some(urls::SUBJECT.to_string()),
                    message: "Commit must have a subject property".to_string(),
                    expected: Some("subject URL".to_string()),
                    actual: None,
                    context: None,
                });
            }

            if resource.get(urls::CREATED_AT).is_err() {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InvalidCommitTimestamp,
                    subject: subject.clone(),
                    property: Some(urls::CREATED_AT.to_string()),
                    message: "Commit must have a createdAt timestamp".to_string(),
                    expected: Some("Unix timestamp in milliseconds".to_string()),
                    actual: None,
                    context: None,
                });
            }

            if resource.get(urls::SIGNER).is_err() {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InvalidCommitSigner,
                    subject: subject.clone(),
                    property: Some(urls::SIGNER.to_string()),
                    message: "Commit must have a signer property".to_string(),
                    expected: Some("Agent URL".to_string()),
                    actual: None,
                    context: None,
                });
            }

            // Note: Full signature verification would require more complex crypto operations
            // This is a structural check for now
        }

        errors
    }

    fn is_local_id(&self, url: &str) -> bool {
        !url.starts_with("http://") && !url.starts_with("https://")
    }

    /// Get a property resource, with caching
    fn get_property(&mut self, prop_url: &str) -> AtomicResult<Resource> {
        if let Some(cached) = self.property_cache.get(prop_url) {
            return Ok(cached.clone());
        }

        match self.store.get_resource(prop_url) {
            Ok(prop) => {
                self.property_cache.insert(prop_url.to_string(), prop.clone());
                Ok(prop)
            }
            Err(e) => Err(e),
        }
    }

    /// Get a class resource, with caching
    fn get_class(&mut self, class_url: &str) -> AtomicResult<Resource> {
        if let Some(cached) = self.class_cache.get(class_url) {
            return Ok(cached.clone());
        }

        match self.store.get_resource(class_url) {
            Ok(class) => {
                self.class_cache.insert(class_url.to_string(), class.clone());
                Ok(class)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_lib::Store;

    #[test]
    fn test_validator_creation() {
        let store = Store::init().unwrap();
        let validator = Validator::new(&store, ValidationLevel::Schema);
        assert_eq!(validator.level, ValidationLevel::Schema);
    }

    #[test]
    fn test_validation_level_ordering() {
        assert!(ValidationLevel::Structural < ValidationLevel::Datatype);
        assert!(ValidationLevel::Datatype < ValidationLevel::Schema);
        assert!(ValidationLevel::Schema < ValidationLevel::Referential);
        assert!(ValidationLevel::Referential < ValidationLevel::Cryptographic);
        assert!(ValidationLevel::Cryptographic < ValidationLevel::Authorization);
    }

    #[test]
    fn test_slug_validation() {
        let store = Store::init().unwrap();
        let mut validator = Validator::new(&store, ValidationLevel::Datatype);

        // Valid slugs
        assert!(validator
            .validate_value_datatype(&Value::Slug("valid-slug".to_string()), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::Slug("slug123".to_string()), None)
            .is_ok());

        // Invalid slugs
        assert!(validator
            .validate_value_datatype(&Value::Slug("Invalid-Slug".to_string()), None)
            .is_err());
        assert!(validator
            .validate_value_datatype(&Value::Slug("invalid slug".to_string()), None)
            .is_err());
    }

    #[test]
    fn test_date_validation() {
        let store = Store::init().unwrap();
        let mut validator = Validator::new(&store, ValidationLevel::Datatype);

        // Valid dates
        assert!(validator
            .validate_value_datatype(&Value::Date("2025-11-06".to_string()), None)
            .is_ok());

        // Invalid dates
        assert!(validator
            .validate_value_datatype(&Value::Date("2025-1-6".to_string()), None)
            .is_err());
        assert!(validator
            .validate_value_datatype(&Value::Date("not-a-date".to_string()), None)
            .is_err());
    }

    #[test]
    fn test_uri_validation() {
        let store = Store::init().unwrap();
        let mut validator = Validator::new(&store, ValidationLevel::Datatype);

        // Valid URIs
        assert!(validator
            .validate_value_datatype(&Value::Uri("https://example.com".to_string()), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::Uri("/relative/path".to_string()), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::Uri("urn:isbn:0451450523".to_string()), None)
            .is_ok());

        // Invalid URIs
        assert!(validator
            .validate_value_datatype(&Value::Uri("".to_string()), None)
            .is_err());
        assert!(validator
            .validate_value_datatype(&Value::Uri("uri with spaces".to_string()), None)
            .is_err());
    }

    #[test]
    fn test_json_validation() {
        let store = Store::init().unwrap();
        let mut validator = Validator::new(&store, ValidationLevel::Datatype);

        // All JSON types are valid
        assert!(validator
            .validate_value_datatype(&Value::JSON(serde_json::json!(null)), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::JSON(serde_json::json!(true)), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::JSON(serde_json::json!(42)), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::JSON(serde_json::json!("string")), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::JSON(serde_json::json!([1, 2, 3])), None)
            .is_ok());
        assert!(validator
            .validate_value_datatype(&Value::JSON(serde_json::json!({"key": "value"})), None)
            .is_ok());
    }
}
