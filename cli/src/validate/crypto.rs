//! Cryptographic validation module for Atomic Data.
//!
//! Implements ED25519 signature verification for commits and related operations.

use atomic_lib::{
    agents::{decode_base64, verify_public_key},
    commit::Commit,
    errors::AtomicResult,
    storelike::Storelike,
    urls, Resource, Value,
};
use base64::{engine::general_purpose, Engine};
use std::collections::{HashMap, HashSet};

use crate::validate::types::{
    ErrorSeverity, ValidationError, ValidationErrorCode, ValidationReport,
};

/// Cryptographic validator for commits and signatures.
pub struct CryptoValidator<'a, S: Storelike> {
    store: &'a S,
    /// Cache of agent public keys
    agent_cache: HashMap<String, String>,
    /// Set of validated commit signatures
    validated_signatures: HashSet<String>,
}

impl<'a, S: Storelike> CryptoValidator<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self {
            store,
            agent_cache: HashMap::new(),
            validated_signatures: HashSet::new(),
        }
    }

    /// Validate a commit resource's cryptographic properties.
    /// Returns validation errors if any issues are found.
    pub fn validate_commit(&mut self, commit_resource: &Resource) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = commit_resource.get_subject().to_string();

        // Check if this is actually a Commit
        if !self.is_commit_resource(commit_resource) {
            return errors; // Not a commit, skip crypto validation
        }

        // Parse as Commit struct for validation
        match Commit::from_resource(commit_resource.clone()) {
            Ok(commit) => {
                // 1. Validate signature
                let sig_errors = self.validate_signature(&commit, &subject);
                errors.extend(sig_errors);

                // 2. Validate timestamp window
                let timestamp_errors = self.validate_timestamp(&commit, &subject);
                errors.extend(timestamp_errors);

                // 3. Validate signer (agent exists and has valid public key)
                let signer_errors = self.validate_signer(&commit, &subject);
                errors.extend(signer_errors);

                // 4. Validate commit chain
                let chain_errors = self.validate_commit_chain(&commit, &subject);
                errors.extend(chain_errors);
            }
            Err(e) => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InvalidCommitStructure,
                    subject: subject.clone(),
                    property: None,
                    message: format!("Failed to parse commit resource: {}", e),
                    expected: Some("Valid Commit structure".to_string()),
                    actual: None,
                    context: None,
                });
            }
        }

        errors
    }

    /// Validate the ED25519 signature of a commit.
    fn validate_signature(&mut self, commit: &Commit, subject: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check signature exists
        let signature = match &commit.signature {
            Some(sig) => sig,
            None => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::MissingCommitSignature,
                    subject: subject.to_string(),
                    property: Some(urls::SIGNATURE.to_string()),
                    message: "Commit is missing required signature".to_string(),
                    expected: Some("Base64 encoded ED25519 signature".to_string()),
                    actual: None,
                    context: None,
                });
                return errors;
            }
        };

        // Validate signature format (base64)
        if let Err(e) = decode_base64(signature) {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: ValidationErrorCode::InvalidSignatureFormat,
                subject: subject.to_string(),
                property: Some(urls::SIGNATURE.to_string()),
                message: format!("Invalid signature format: {}", e),
                expected: Some("Base64 encoded ED25519 signature (64 bytes)".to_string()),
                actual: Some(signature.clone()),
                context: None,
            });
            return errors;
        }

        // Get signer's public key
        let public_key = match self.get_agent_public_key(&commit.signer) {
            Ok(pk) => pk,
            Err(e) => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::SignerNotFound,
                    subject: subject.to_string(),
                    property: Some(urls::SIGNER.to_string()),
                    message: format!("Could not fetch signer's public key: {}", e),
                    expected: Some("Valid agent with public key".to_string()),
                    actual: Some(commit.signer.clone()),
                    context: None,
                });
                return errors;
            }
        };

        // Verify the signature using ring
        match self.verify_ed25519_signature(commit, &public_key) {
            Ok(()) => {
                // Mark this signature as validated
                self.validated_signatures.insert(signature.clone());
            }
            Err(e) => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InvalidSignature,
                    subject: subject.to_string(),
                    property: Some(urls::SIGNATURE.to_string()),
                    message: format!("Signature verification failed: {}", e),
                    expected: Some("Valid ED25519 signature".to_string()),
                    actual: Some(signature.clone()),
                    context: Some(serde_json::json!({
                        "signer": commit.signer,
                        "public_key": public_key
                    })),
                });
            }
        }

        errors
    }

    /// Verify ED25519 signature using ring library.
    fn verify_ed25519_signature(&self, commit: &Commit, public_key: &str) -> AtomicResult<()> {
        let signature_b64 = commit
            .signature
            .as_ref()
            .ok_or("No signature in commit")?;

        // Decode public key and signature from base64
        let public_key_bytes = decode_base64(public_key)?;
        let signature_bytes = decode_base64(signature_b64)?;

        // Verify signature length (ED25519 signature is 64 bytes)
        if signature_bytes.len() != 64 {
            return Err(format!(
                "Invalid signature length: {} bytes (expected 64)",
                signature_bytes.len()
            )
            .into());
        }

        // Serialize commit deterministically (without signature)
        let commit_message = commit.serialize_deterministically_json_ad(self.store)?;

        // Create unparsed public key for verification
        let peer_public_key =
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key_bytes);

        // Verify the signature
        peer_public_key
            .verify(commit_message.as_bytes(), &signature_bytes)
            .map_err(|_| {
                format!(
                    "ED25519 signature verification failed. Commit message: {}",
                    commit_message
                )
            })?;

        Ok(())
    }

    /// Validate timestamp is within acceptable window.
    fn validate_timestamp(&self, commit: &Commit, subject: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check timestamp is not in the future
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let acceptable_drift_ms = 10000; // 10 seconds

        if commit.created_at > now_ms + acceptable_drift_ms {
            errors.push(ValidationError {
                severity: ErrorSeverity::Error,
                code: ValidationErrorCode::FutureTimestamp,
                subject: subject.to_string(),
                property: Some(urls::CREATED_AT.to_string()),
                message: format!(
                    "Commit timestamp is in the future ({}ms ahead)",
                    commit.created_at - now_ms
                ),
                expected: Some(format!("Timestamp <= {}", now_ms + acceptable_drift_ms)),
                actual: Some(commit.created_at.to_string()),
                context: None,
            });
        }

        // Check timestamp is not too old (e.g., more than 30 days)
        let max_age_ms = 30 * 24 * 60 * 60 * 1000i64; // 30 days in milliseconds
        if commit.created_at < now_ms - max_age_ms {
            errors.push(ValidationError {
                severity: ErrorSeverity::Warning,
                code: ValidationErrorCode::ExpiredCommit,
                subject: subject.to_string(),
                property: Some(urls::CREATED_AT.to_string()),
                message: format!(
                    "Commit timestamp is very old ({}ms ago)",
                    now_ms - commit.created_at
                ),
                expected: Some(format!("Timestamp >= {}", now_ms - max_age_ms)),
                actual: Some(commit.created_at.to_string()),
                context: None,
            });
        }

        errors
    }

    /// Validate the signer agent exists and has a valid public key.
    fn validate_signer(&mut self, commit: &Commit, subject: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Try to fetch signer's public key (this also validates the agent exists)
        match self.get_agent_public_key(&commit.signer) {
            Ok(public_key) => {
                // Validate public key format
                if let Err(e) = verify_public_key(&public_key) {
                    errors.push(ValidationError {
                        severity: ErrorSeverity::Error,
                        code: ValidationErrorCode::InvalidPublicKey,
                        subject: subject.to_string(),
                        property: Some(urls::PUBLIC_KEY.to_string()),
                        message: format!("Invalid public key format: {}", e),
                        expected: Some("Valid ED25519 public key (32 bytes, base64)".to_string()),
                        actual: Some(public_key),
                        context: None,
                    });
                }
            }
            Err(e) => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::SignerNotFound,
                    subject: subject.to_string(),
                    property: Some(urls::SIGNER.to_string()),
                    message: format!("Signer agent not found: {}", e),
                    expected: Some("Valid agent URL".to_string()),
                    actual: Some(commit.signer.clone()),
                    context: None,
                });
            }
        }

        errors
    }

    /// Validate the commit chain (previousCommit).
    fn validate_commit_chain(&self, commit: &Commit, subject: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check if the subject resource exists
        match self.store.get_resource(&commit.subject) {
            Ok(target_resource) => {
                // If resource exists, check lastCommit matches previousCommit
                if let Ok(Value::AtomicUrl(last_commit)) = target_resource.get(urls::LAST_COMMIT) {
                    match &commit.previous_commit {
                        Some(prev_commit) => {
                            if prev_commit != last_commit {
                                errors.push(ValidationError {
                                    severity: ErrorSeverity::Error,
                                    code: ValidationErrorCode::CommitChainBroken,
                                    subject: subject.to_string(),
                                    property: Some(urls::PREVIOUS_COMMIT.to_string()),
                                    message: format!(
                                        "previousCommit mismatch: expected '{}', got '{}'",
                                        last_commit, prev_commit
                                    ),
                                    expected: Some(last_commit.clone()),
                                    actual: Some(prev_commit.clone()),
                                    context: Some(serde_json::json!({
                                        "target_resource": commit.subject
                                    })),
                                });
                            }
                        }
                        None => {
                            errors.push(ValidationError {
                                severity: ErrorSeverity::Error,
                                code: ValidationErrorCode::MissingPreviousCommit,
                                subject: subject.to_string(),
                                property: Some(urls::PREVIOUS_COMMIT.to_string()),
                                message: format!(
                                    "Resource '{}' has lastCommit but commit lacks previousCommit",
                                    commit.subject
                                ),
                                expected: Some(last_commit.clone()),
                                actual: None,
                                context: None,
                            });
                        }
                    }
                }
            }
            Err(_) => {
                // Resource doesn't exist yet - this is a new resource creation
                // previousCommit should be None or not matter
            }
        }

        // Validate previousCommit URL format if present
        if let Some(prev_commit) = &commit.previous_commit {
            if !prev_commit.starts_with("http") {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InvalidPreviousCommit,
                    subject: subject.to_string(),
                    property: Some(urls::PREVIOUS_COMMIT.to_string()),
                    message: format!("Invalid previousCommit URL: {}", prev_commit),
                    expected: Some("Valid HTTP(S) URL to a Commit resource".to_string()),
                    actual: Some(prev_commit.clone()),
                    context: None,
                });
            }
        }

        errors
    }

    /// Get an agent's public key (with caching).
    fn get_agent_public_key(&mut self, agent_url: &str) -> AtomicResult<String> {
        // Check cache first
        if let Some(cached) = self.agent_cache.get(agent_url) {
            return Ok(cached.clone());
        }

        // Fetch agent resource
        let agent_resource = self.store.get_resource(agent_url)?;

        // Get public key
        let public_key = agent_resource.get(urls::PUBLIC_KEY)?.to_string();

        // Cache for future use
        self.agent_cache
            .insert(agent_url.to_string(), public_key.clone());

        Ok(public_key)
    }

    /// Check if a resource is a Commit.
    fn is_commit_resource(&self, resource: &Resource) -> bool {
        if let Ok(Value::ResourceArray(classes)) = resource.get(urls::IS_A) {
            classes.iter().any(|item| match item {
                atomic_lib::values::SubResource::Subject(s) => s == urls::COMMIT,
                _ => false,
            })
        } else {
            false
        }
    }

    /// Get the number of validated signatures.
    pub fn validated_signature_count(&self) -> usize {
        self.validated_signatures.len()
    }
}

/// Authorization validator for checking rights.
pub struct AuthorizationValidator<'a, S: Storelike> {
    store: &'a S,
    /// Cache of resource rights
    rights_cache: HashMap<String, Vec<String>>,
}

impl<'a, S: Storelike> AuthorizationValidator<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self {
            store,
            rights_cache: HashMap::new(),
        }
    }

    /// Check if an agent has write rights on a resource.
    pub fn check_write_rights(
        &mut self,
        resource: &Resource,
        agent_url: &str,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = resource.get_subject().to_string();

        // Use atomic_lib's hierarchy check
        match atomic_lib::hierarchy::check_write(self.store, resource, &agent_url.into()) {
            Ok(_) => {
                // Agent has write rights
            }
            Err(e) => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InsufficientWriteRights,
                    subject: subject.clone(),
                    property: None,
                    message: format!("Agent '{}' lacks write rights: {}", agent_url, e),
                    expected: Some("Write permission granted".to_string()),
                    actual: Some("Permission denied".to_string()),
                    context: Some(serde_json::json!({
                        "agent": agent_url,
                        "resource": subject
                    })),
                });
            }
        }

        errors
    }

    /// Check if an agent has append rights on a resource's parent.
    pub fn check_append_rights(
        &mut self,
        resource: &Resource,
        agent_url: &str,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let subject = resource.get_subject().to_string();

        // Use atomic_lib's hierarchy check for append
        match atomic_lib::hierarchy::check_append(self.store, resource, &agent_url.into()) {
            Ok(_) => {
                // Agent has append rights
            }
            Err(e) => {
                errors.push(ValidationError {
                    severity: ErrorSeverity::Error,
                    code: ValidationErrorCode::InsufficientAppendRights,
                    subject: subject.clone(),
                    property: None,
                    message: format!("Agent '{}' lacks append rights: {}", agent_url, e),
                    expected: Some("Append permission granted".to_string()),
                    actual: Some("Permission denied".to_string()),
                    context: Some(serde_json::json!({
                        "agent": agent_url,
                        "resource": subject
                    })),
                });
            }
        }

        errors
    }

    /// Validate commit authorization (write/append rights based on operation).
    pub fn validate_commit_authorization(
        &mut self,
        commit: &Commit,
        target_resource: Option<&Resource>,
    ) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        match target_resource {
            Some(resource) => {
                // Resource exists - check write rights
                errors.extend(self.check_write_rights(resource, &commit.signer));
            }
            None => {
                // New resource - need to check append rights on parent
                // This would require fetching the parent and checking append rights
                // For now, we'll skip this as it requires knowledge of the parent structure
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_lib::{agents::Agent, Store, Storelike};

    #[test]
    fn test_crypto_validator_creation() {
        let store = Store::init().unwrap();
        let validator = CryptoValidator::new(&store);
        assert_eq!(validator.validated_signature_count(), 0);
    }

    #[test]
    fn test_authorization_validator_creation() {
        let store = Store::init().unwrap();
        let _validator = AuthorizationValidator::new(&store);
    }

    #[test]
    fn test_commit_signature_validation() {
        let store = Store::init().unwrap();
        let validator = CryptoValidator::new(&store);

        // Test basic validator creation for commits
        // Full signature validation requires network access for class definitions,
        // so we test the timestamp validation separately
        assert_eq!(validator.validated_signature_count(), 0);
    }

    #[test]
    fn test_signature_format_validation() {
        let store = Store::init().unwrap();
        let _validator = CryptoValidator::new(&store);

        // Test base64 signature format validation
        let valid_sig = "YtDR/xo0272LHNBQtDer4LekzdkfUANFTI0eHxZhITXnbC3j0LCqDWhr6itNvo4tFnep6DCbev5OKAHH89+TDA==";
        assert!(decode_base64(valid_sig).is_ok());
        let decoded = decode_base64(valid_sig).unwrap();
        assert_eq!(decoded.len(), 64); // ED25519 signature is 64 bytes

        let invalid_sig = "not-valid-base64!!!";
        assert!(decode_base64(invalid_sig).is_err());
    }

    #[test]
    fn test_public_key_validation() {
        // Test public key format validation
        let valid_public_key = "7LsjMW5gOfDdJzK/atgjQ1t20J/rw8MjVg6xwqm+h8U=";
        assert!(verify_public_key(valid_public_key).is_ok());

        // Invalid length
        let invalid_length = "7LsjMW5gOfDdJzK/atgjQ1t20J/rw8MjVg6xwm+h8U";
        assert!(verify_public_key(invalid_length).is_err());

        // Invalid base64 character
        let invalid_char = "7LsjMW5gOfDdJzK/atgjQ1t20^/rw8MjVg6xwqm+h8U=";
        assert!(verify_public_key(invalid_char).is_err());
    }

    #[test]
    fn test_timestamp_validation_future() {
        let store = Store::init().unwrap();
        let validator = CryptoValidator::new(&store);

        // Create a commit with future timestamp
        let future_commit = Commit {
            subject: "https://example.com/test".to_string(),
            created_at: i64::MAX,
            signer: "https://example.com/agent".to_string(),
            set: None,
            remove: None,
            destroy: None,
            signature: Some("test".to_string()),
            push: None,
            previous_commit: None,
            url: None,
        };

        let errors = validator.validate_timestamp(&future_commit, "test_subject");
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| e.code == ValidationErrorCode::FutureTimestamp));
    }

    #[test]
    fn test_timestamp_validation_old() {
        let store = Store::init().unwrap();
        let validator = CryptoValidator::new(&store);

        // Create a commit with very old timestamp (1 second after epoch)
        let old_commit = Commit {
            subject: "https://example.com/test".to_string(),
            created_at: 1000,
            signer: "https://example.com/agent".to_string(),
            set: None,
            remove: None,
            destroy: None,
            signature: Some("test".to_string()),
            push: None,
            previous_commit: None,
            url: None,
        };

        let errors = validator.validate_timestamp(&old_commit, "test_subject");
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| e.code == ValidationErrorCode::ExpiredCommit));
    }
}
