# Fork vs Upstream Analysis

**Date:** 2025-11-06
**Fork Base:** terraphim/atomic-server @ 521ee2b (Add folder and bookmark to default resources)
**Upstream:** atomicdata-dev/atomic-server @ develop (6947650)
**Commits Behind:** ~30 commits

## Executive Summary

The terraphim fork is approximately **30 commits behind** the upstream atomic-server develop branch. The upstream has significant changes that **MUST** be accounted for in the validation service implementation:

### Critical Changes Affecting Validation Service

1. ✅ **New Datatypes**: URI and JSON datatypes added
2. ✅ **Value Enum Restructuring**: Resource variant removed, Uri and JSON variants added
3. ✅ **ResourceResponse Type**: New wrapper for resources with referenced resources
4. ⚠️ **Database Encoding**: New encoding/decoding system for PropVals
5. ⚠️ **Database Migrations**: v1_types migration system added
6. ⚠️ **Storelike Interface**: Return types changed to ResourceResponse
7. ⚠️ **Transaction System**: Enhanced batching and error handling

### Impact Assessment

**HIGH IMPACT** - These changes require updates to validation specification:
- Datatype validation must include URI and JSON types
- Value parsing must handle new variants
- Resource serialization/deserialization affected
- Database encoding changes affect snapshot format

**MEDIUM IMPACT** - These changes affect implementation approach:
- ResourceResponse changes how resources are fetched and stored
- Class extenders provide new extension points
- Migration system affects version compatibility

**LOW IMPACT** - These changes don't affect core validation:
- AI features (browser-only, doesn't affect atomic-lib core)
- UI improvements
- Search enhancements (server-side only)

---

## Detailed Changes Analysis

### 1. Datatype System Changes

#### New Datatypes Added

**File:** `lib/src/datatype.rs`

```diff
+ Uri,
+ JSON,
```

**URL Constants Added:**
```rust
pub const URI: &str = "https://atomicdata.dev/datatypes/uri";
pub const JSON: &str = "https://atomicdata.dev/datatypes/json";
```

**Impact on Validation:**
- Level 1 (Datatype validation) must validate URI format (similar to AtomicUrl but less strict)
- Level 1 must validate JSON format (any valid JSON)
- Specification document needs updating with these datatypes

**Validation Rules:**
```rust
// URI validation (lib/src/utils.rs - check_valid_uri)
- Must be valid URI format (RFC 3986)
- Can be relative or absolute
- Does not require http/https scheme

// JSON validation
- Must be valid JSON (parsed by serde_json)
- Can be any JSON value: object, array, string, number, boolean, null
```

### 2. Value Enum Restructuring

#### Changes to Value Enum

**File:** `lib/src/values.rs`

**Removed:**
```rust
- Resource(Box<Resource>),  // From Value enum
- Resource(Box<Resource>),  // From SubResource enum
```

**Added:**
```rust
+ Uri(String),              // New variant
+ JSON(serde_json::Value),  // New variant
```

**Impact on Validation:**
- Parsing logic must not expect Value::Resource variant
- Serialization must handle Uri and JSON variants
- SubResource handling simplified (only Nested and Subject variants remain)

**Migration Note:**
The removal of `Value::Resource` means resources are no longer nested directly as values. Instead, they use:
- `Value::AtomicUrl` for references
- `SubResource::Nested` for inline nested resources
- `SubResource::Subject` for subject-only references

This is a **breaking change** that affects:
- JSON-AD parsing (parse.rs)
- Resource serialization (serialize.rs)
- Value conversion logic

### 3. ResourceResponse Type

#### New Response Wrapper

**File:** `lib/src/storelike.rs`

```rust
pub enum ResourceResponse {
    Resource(Resource),
    ResourceWithReferenced(Resource, Vec<Resource>),
}
```

**Key Methods:**
```rust
impl ResourceResponse {
    pub fn to_single(&self) -> Resource
    pub fn to_json_ad(&self) -> AtomicResult<String>
    pub fn to_json(&self, store: &impl Storelike) -> AtomicResult<String>
    pub fn to_json_ld(&self, store: &impl Storelike) -> AtomicResult<String>
    pub fn to_atoms(&self) -> Vec<Atom>
    pub fn to_n_triples(&self, store: &impl Storelike) -> AtomicResult<String>
    pub fn from_vec(main_subject: &str, vec: Vec<Resource>) -> AtomicResult<Self>
}
```

**Impact on Validation:**
- Extractor must handle ResourceResponse instead of just Resource
- When fetching resources, may receive multiple resources (main + referenced)
- Validation must handle both response types
- Synchronization must preserve referenced resources

**Use Case:**
When fetching a resource with nested or referenced resources, the server can return all related resources in one response, improving efficiency.

**Example:**
```json
[
  {
    "@id": "https://example.com/person/1",
    "https://atomicdata.dev/properties/name": "Alice",
    "https://example.com/properties/knows": "https://example.com/person/2"
  },
  {
    "@id": "https://example.com/person/2",
    "https://atomicdata.dev/properties/name": "Bob"
  }
]
```

The main resource is person/1, but person/2 is included as a referenced resource.

### 4. Database Encoding Changes

#### New Encoding System

**Files:**
- `lib/src/db/encoding.rs` (new file)
- `lib/src/db/v1_types.rs` (new file)
- `lib/src/db/migrations.rs` (updated)

**New Functions:**
```rust
pub fn encode_propvals(propvals: &PropVals) -> AtomicResult<Vec<u8>>
pub fn decode_propvals(bytes: &[u8]) -> AtomicResult<PropVals>
```

**Impact on Validation:**
- Database snapshot format has changed
- Extracting from older versions may require migration
- Binary encoding is now separate from serialization
- Version compatibility becomes important

**Migration System:**
The database now has versioned schemas (v1_types) and migrations between versions. This affects:
- **Snapshot extraction**: Must handle different schema versions
- **Validation**: Must validate against correct schema version
- **Synchronization**: Source and target may have different schema versions

**Recommendation:**
Add schema version tracking to StoreSnapshot:
```rust
pub struct StoreSnapshot {
    pub schema_version: String,  // NEW: e.g., "v1", "v2"
    pub server_url: String,
    pub resources: Vec<Resource>,
    // ...
}
```

### 5. Storelike Interface Changes

#### Method Signature Changes

**File:** `lib/src/storelike.rs`

```rust
// Before (fork):
fn get_resource_extended(
    &self,
    subject: &str,
    skip_dynamic: bool,
    for_agent: &ForAgent,
) -> AtomicResult<Resource>

// After (upstream):
fn get_resource_extended(
    &self,
    subject: &str,
    skip_dynamic: bool,
    for_agent: &ForAgent,
) -> AtomicResult<ResourceResponse>  // Changed return type
```

**New Methods:**
```rust
fn search(
    &self,
    query: &str,
    opts: crate::client::search::SearchOpts,
) -> AtomicResult<Vec<Resource>>
```

**Impact on Validation:**
- Extractor must handle ResourceResponse
- get_resource calls need updating
- Search functionality can be leveraged for validation

### 6. Class Extenders System

#### New Plugin Architecture

**File:** `lib/src/class_extender.rs` (new file)

```rust
pub struct ClassExtender {
    pub target_class: String,
    pub extend_get: Option<ExtendGet>,
    pub extend_commit: Option<ExtendCommit>,
}
```

**Purpose:**
Allows custom logic to run when:
- Getting a resource of a specific class
- Committing a resource of a specific class

**Impact on Validation:**
- Validation must be aware that resources may be dynamically extended
- Class extenders can modify resource properties on-the-fly
- Validation Level 2 (Schema) may need to account for dynamically added properties

**Example Use Cases:**
- Auto-calculating derived properties
- Enforcing custom business rules
- Adding metadata fields

### 7. Transaction and Batch System

#### Enhanced Transaction Handling

**File:** `lib/src/db.rs`

```rust
(
    &self.resources,
    &self.prop_val_sub_index,
    &self.reference_index,
    &self.watched_queries,
    &self.query_index,
).transaction(|(tx_resources, tx_prop_val_sub_index, ...) | {
    tx_resources.apply_batch(&batch_resources)?;
    tx_prop_val_sub_index.apply_batch(&batch_propvalsub)?;
    // ...
    Ok(())
})
```

**Changes:**
- Atomic transactions across multiple index trees
- Batch operations for performance
- Better error handling with TransactionError

**Impact on Validation:**
- Synchronization must use batched operations for performance
- Transaction failures must be handled atomically
- Index consistency is guaranteed by transactions

**Benefit for Sync:**
When syncing large datasets, batching commits improves performance significantly.

### 8. Recursive Delete

#### New Functionality

**File:** `lib/src/db.rs`

```rust
fn recursive_remove(&self, subject: &str, transaction: &mut Transaction) -> AtomicResult<()>
```

**Purpose:**
Deletes a resource and all its children in a hierarchy.

**Impact on Validation:**
- Validation must check for orphaned resources after deletes
- Synchronization must handle cascading deletes
- Referential integrity validation must account for recursive deletes

### 9. Parse.rs Major Refactoring

**File:** `lib/src/parse.rs`

**Changes:** ~599 insertions, ~300 deletions

**Key Changes:**
- Removed support for Value::Resource variant
- Enhanced nested resource handling
- Improved error messages
- Better handling of local IDs vs absolute URLs
- ResourceResponse integration

**Impact on Validation:**
- Parsing validation (Level 0) must use updated parse logic
- Nested resource validation improved
- Error handling more robust

---

## Compatibility Matrix

| Component | Fork (521ee2b) | Upstream (6947650) | Compatible? |
|-----------|---------------|-------------------|-------------|
| Datatypes | 10 types | 12 types (+ URI, JSON) | ⚠️ Partial |
| Value Enum | Has Resource variant | No Resource variant | ❌ Breaking |
| Storelike Interface | Returns Resource | Returns ResourceResponse | ❌ Breaking |
| Database Encoding | bincode direct | encode_propvals/decode | ⚠️ Partial |
| Serialization | JSON-AD v1 | JSON-AD v1 (same) | ✅ Yes |
| URL Constants | Missing URI, JSON | Has URI, JSON | ⚠️ Partial |

---

## Recommendations for Validation Service

### 1. **Base Implementation on Upstream**

**Recommendation:** Update the fork to match upstream develop before implementing validation service.

**Rationale:**
- Avoid implementing against outdated codebase
- Benefit from bug fixes and improvements
- Ensure long-term maintainability

**Action Items:**
```bash
# Option A: Merge upstream develop into fork
git merge upstream/develop

# Option B: Rebase fork onto upstream develop
git rebase upstream/develop

# Option C: Cherry-pick specific commits
git cherry-pick <commit-range>
```

### 2. **Update Validation Specification**

**Changes Required:**

#### VALIDATION_SPECIFICATION.md Updates

**Section 3.1.1 - Primitive Datatypes:**

Add two new rows to the datatype table:

| Datatype | Rust Type | JSON Representation | Validation Rules |
|----------|-----------|-------------------|------------------|
| Uri | `String` | `"https://example.com"` or `"/relative/path"` | Valid URI per RFC 3986, can be relative |
| JSON | `serde_json::Value` | `{"any": "json"}` | Valid JSON (object, array, primitive, null) |

**Section 3.1.2 - Composite Structures:**

Update Value enum:
```rust
pub enum Value {
    // ... existing variants ...
-   Resource(Box<Resource>),     // REMOVED
+   Uri(String),                 // NEW
+   JSON(serde_json::Value),     // NEW
    Unsupported(UnsupportedValue),
}
```

**Section 5.2.1 - Extractor Component:**

Update interfaces to use ResourceResponse:
```rust
interface Extractor {
  // Returns ResourceResponse instead of Resource
  extractResource(url: string): Promise<ResourceResponse>

  // Handle referenced resources
  extractWithReferences(url: string): Promise<{
    main: Resource,
    referenced: Resource[]
  }>
}
```

**Section 4.1.1 - Atom-Level Validation:**

Add validation rules:
```
- [ ] URI values must conform to RFC 3986 format
- [ ] JSON values must be parseable by serde_json
- [ ] URI can be relative or absolute
- [ ] JSON can be any valid JSON type
```

### 3. **Implementation Strategy**

#### Phase 1 Updates

**Week 1, Day 1-2: Project Setup**

Update Cargo.toml to use upstream-compatible atomic-lib:
```toml
[dependencies]
atomic_lib = { git = "https://github.com/atomicdata-dev/atomic-server", branch = "develop" }
# OR if using local path:
atomic_lib = { path = "../lib" }  # After merging upstream
```

**Week 1, Day 3-4: Extractor**

Update extractor to handle ResourceResponse:
```rust
pub async fn fetch_resource(&self, subject: &str) -> AtomicResult<ResourceResponse> {
    self.store.get_resource_extended(subject, false, &ForAgent::Sudo).await
}

pub async fn extract_with_references(
    &self,
    subject: &str,
) -> AtomicResult<(Resource, Vec<Resource>)> {
    let response = self.fetch_resource(subject).await?;
    match response {
        ResourceResponse::Resource(r) => Ok((r, vec![])),
        ResourceResponse::ResourceWithReferenced(r, refs) => Ok((r, refs)),
    }
}
```

**Week 1, Day 5: Validator**

Add URI and JSON validation:
```rust
fn validate_value_datatype(value: &Value, expected: &DataType) -> Result<(), ValidationError> {
    match (value, expected) {
        // ... existing matches ...
        (Value::Uri(uri), DataType::Uri) => {
            check_valid_uri(uri)?;
            Ok(())
        }
        (Value::JSON(json), DataType::JSON) => {
            // JSON is already validated by serde_json during parsing
            Ok(())
        }
        _ => Err(ValidationError {
            code: "DATATYPE_MISMATCH".to_string(),
            message: format!("Expected {:?}, got {:?}", expected, value.datatype()),
            // ...
        })
    }
}
```

### 4. **Version Compatibility Handling**

Add version detection to validation service:

```rust
pub enum SchemaVersion {
    V1,  // Fork version (no URI/JSON, has Resource variant)
    V2,  // Upstream version (URI/JSON, no Resource variant)
}

impl Extractor {
    pub async fn detect_version(&self) -> AtomicResult<SchemaVersion> {
        // Try to fetch a known datatype
        match self.store.get_resource(urls::URI).await {
            Ok(_) => Ok(SchemaVersion::V2),
            Err(_) => Ok(SchemaVersion::V1),
        }
    }
}
```

### 5. **Migration Path**

If maintaining backward compatibility is required:

```rust
pub struct VersionedValidator {
    version: SchemaVersion,
}

impl VersionedValidator {
    pub fn validate_resource(&self, resource: &Resource) -> ValidationReport {
        match self.version {
            SchemaVersion::V1 => self.validate_v1(resource),
            SchemaVersion::V2 => self.validate_v2(resource),
        }
    }

    fn validate_v1(&self, resource: &Resource) -> ValidationReport {
        // Validate against fork schema (10 datatypes)
    }

    fn validate_v2(&self, resource: &Resource) -> ValidationReport {
        // Validate against upstream schema (12 datatypes)
    }
}
```

---

## Updated Implementation Plan

### Phase 1: Foundation (Weeks 1-3) - UPDATED

#### Week 1: Project Setup and Upstream Alignment

**Day 1:** Merge/rebase upstream develop
```bash
git checkout claude/atomic-validation-spec-011CUrURTSRTMU7trPGjFcPd
git fetch upstream develop
git merge upstream/develop  # Resolve any conflicts
```

**Day 2:** Update validation specification
- Add URI and JSON datatypes
- Update Value enum documentation
- Add ResourceResponse handling

**Day 3:** Update implementation plan
- Reflect upstream changes
- Update code examples
- Adjust timelines if needed

**Day 4-5:** Project scaffolding
- Create directory structure
- Set up Cargo.toml with upstream-compatible dependencies
- Create type definitions (types.rs) with ResourceResponse support

#### Week 2-3: Extractor and Validator (No changes needed)

Continue with existing plan, using updated types.

---

## Testing Strategy Updates

### Additional Test Scenarios

1. **URI Datatype Tests**
```rust
#[test]
fn test_validate_uri_datatype() {
    // Valid absolute URI
    assert!(validate_uri("https://example.com").is_ok());
    // Valid relative URI
    assert!(validate_uri("/path/to/resource").is_ok());
    // Invalid URI
    assert!(validate_uri("not a uri!").is_err());
}
```

2. **JSON Datatype Tests**
```rust
#[test]
fn test_validate_json_datatype() {
    assert!(validate_json(r#"{"key": "value"}"#).is_ok());
    assert!(validate_json(r#"[1, 2, 3]"#).is_ok());
    assert!(validate_json(r#"42"#).is_ok());
    assert!(validate_json(r#"invalid json"#).is_err());
}
```

3. **ResourceResponse Tests**
```rust
#[test]
async fn test_extract_with_references() {
    let extractor = Extractor::new("https://atomicdata.dev", None).unwrap();
    let response = extractor.fetch_resource("https://atomicdata.dev/classes/Class").await.unwrap();

    match response {
        ResourceResponse::ResourceWithReferenced(main, refs) => {
            assert_eq!(main.get_subject(), "https://atomicdata.dev/classes/Class");
            assert!(!refs.is_empty()); // Should have referenced properties
        }
        _ => {}
    }
}
```

4. **Version Compatibility Tests**
```rust
#[test]
async fn test_version_detection() {
    let v1_store = create_v1_test_store();
    let v2_store = create_v2_test_store();

    assert_eq!(detect_version(&v1_store).await, SchemaVersion::V1);
    assert_eq!(detect_version(&v2_store).await, SchemaVersion::V2);
}
```

---

## Conclusion

### Critical Action Items

1. ✅ **MERGE UPSTREAM** - Update fork to include upstream changes
2. ✅ **UPDATE SPEC** - Add URI, JSON datatypes and ResourceResponse
3. ✅ **UPDATE PLAN** - Reflect changes in implementation plan
4. ⚠️ **TEST COMPATIBILITY** - Ensure backward compatibility if needed
5. ⚠️ **DOCUMENT VERSIONS** - Clearly document schema version differences

### Priority Level

**🔴 HIGH PRIORITY** - Must be addressed before starting Phase 1 implementation

The upstream changes are **NOT** cosmetic - they include breaking changes to core datatypes and interfaces. Implementing the validation service against the fork's current state would result in:
- Incompatibility with upstream servers
- Missing validation for 2 datatypes (URI, JSON)
- Incorrect handling of ResourceResponse
- Potential data corruption during synchronization

### Recommendation

**Option 1 (Recommended):** Merge upstream develop immediately
```bash
git merge upstream/develop
# Resolve conflicts
# Update VALIDATION_SPECIFICATION.md
# Continue with Phase 1
```

**Option 2 (If conflicts are complex):** Create new branch from upstream
```bash
git checkout -b validation-service-upstream upstream/develop
# Cherry-pick the two validation spec commits
git cherry-pick 521ee2b 77942ad
# Continue with Phase 1
```

**Option 3 (If fork divergence is intentional):** Implement versioned validation
```bash
# Keep fork as-is
# Implement version detection
# Support both fork and upstream schemas
# More complex, but maintains compatibility
```

---

## Appendix: Upstream Commit Summary

### Commits in Upstream Not in Fork (30 commits)

**AI/MCP Features (Browser only):**
- #951 - AI chat, generative features, Ollama support
- #1049 - MCP server support

**Performance & Stability:**
- #1115 - readOnly support in tables
- #1060 - Jaeger tracing improvements
- #1105 - UI state improvements

**Core Changes (Affecting atomic-lib):**
- Resource references in end blocks serialization fix
- JSONValue for generated ontologies
- Reactivity improvements in svelte lib
- Large resourceArray rendering performance

**Testing & Quality:**
- Linting fixes
- Lockfile updates
- Test improvements
- E2E test coverage

None of these commits appear to be Turso-specific, suggesting the "turso_options" branch either:
1. Doesn't exist in this repository
2. Was renamed or merged
3. Is in a different repository

---

**END OF ANALYSIS**
