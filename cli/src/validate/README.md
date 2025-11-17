# Atomic Data Validation Service

A comprehensive validation and synchronization service for Atomic Data servers, providing full compatibility with all 12 Atomic Data datatypes including the latest Uri and JSON types.

## Features

- **Full Datatype Support**: All 12 Atomic Data types (String, Integer, Float, Boolean, Date, Timestamp, Slug, Markdown, AtomicUrl, ResourceArray, Uri, JSON)
- **ResourceResponse Handling**: Full support for nested/referenced resources
- **6 Validation Levels**: From structural parsing to authorization checking
- **Cryptographic Validation**: ED25519 signature verification for commits
- **Synchronization Engine**: Diff generation, conflict resolution, bidirectional sync
- **HTTP Client**: Retry logic, progress reporting, connectivity testing

## CLI Commands

### Validation Commands

```bash
# Validate a server at a specific level
atomic-cli validate-server <url> --level <0-5> --output <json|text>

# Examples:
atomic-cli validate-server https://atomicdata.dev --level 2
atomic-cli validate-server http://localhost:9883 --level 4 --agent "your-agent-secret"
atomic-cli validate-server http://localhost:9883 --level 5 --output json > report.json
```

**Validation Levels:**
- `0` - Structural: JSON-AD parsing, basic structure
- `1` - Datatype: Validates all 12 datatypes
- `2` - Schema: Class requirements, property constraints
- `3` - Referential: Broken links, circular dependencies
- `4` - Cryptographic: Signatures, timestamps, commit chains
- `5` - Authorization: Permissions, rights hierarchy

### Server Information

```bash
# Detect schema version (V1: 10 datatypes, V2: 12 datatypes)
atomic-cli detect-version <url>

# Test server connectivity
atomic-cli test-connection <url> --agent "optional-secret"

# Examples:
atomic-cli detect-version https://atomicdata.dev
atomic-cli test-connection http://localhost:9883
```

### Ontology Extraction

```bash
# Extract an ontology to JSON-AD file
atomic-cli extract-ontology <ontology-url> --out <file-path>

# Examples:
atomic-cli extract-ontology https://atomicdata.dev/ontology --out ontology.json
atomic-cli extract-ontology http://localhost:9883/ontology --out local.json --agent "secret"
```

### Synchronization Commands

```bash
# Generate a diff report between two servers
atomic-cli diff-servers <source-url> <target-url> --output <json|text>

# Synchronize data between servers
atomic-cli sync-servers <source> <target> \
  --mode <push|pull|bidirectional> \
  --conflict-strategy <source|target|latest|manual|skip> \
  --dry-run \
  --include-ontologies \
  --filter-subjects "pattern1,pattern2"

# Examples:
# Compare two servers
atomic-cli diff-servers http://localhost:9883 http://localhost:9884

# Push changes from source to target (dry run)
atomic-cli sync-servers http://localhost:9883 http://localhost:9884 \
  --mode push --dry-run --agent "your-agent-secret"

# Pull changes with latest-wins conflict resolution
atomic-cli sync-servers http://remote-server http://localhost:9883 \
  --mode pull --conflict-strategy latest --agent "secret"

# Bidirectional sync with ontologies
atomic-cli sync-servers http://server1 http://server2 \
  --mode bidirectional --include-ontologies --agent "secret"
```

### Conflict Resolution Strategies

- **source**: Always use the source value
- **target**: Always keep the target value
- **latest**: Use the most recently modified value (based on commit timestamps)
- **manual**: Record conflicts for manual resolution
- **skip**: Skip conflicting resources

## Architecture

```
cli/src/validate/
├── mod.rs          # Module root, public API
├── types.rs        # Core types (600+ lines)
├── extractor.rs    # Resource extraction (500+ lines)
├── validator.rs    # Validation logic (700+ lines)
├── crypto.rs       # Cryptographic validation (620+ lines)
├── sync.rs         # Synchronization engine (1000+ lines)
├── client.rs       # HTTP client with retry (410+ lines)
└── README.md       # This documentation
```

## Programmatic Usage

```rust
use atomic_cli::validate::{
    ValidationLevel,
    Validator,
    Extractor,
    SyncEngine,
    SyncOptions,
    ConflictStrategy,
    SyncMode,
    AtomicClient,
};

// Validate a server
let report = validate::validate_server(
    "http://localhost:9883",
    Some("agent-secret".to_string()),
    ValidationLevel::Schema,
)?;
println!("Valid: {}, Errors: {}", report.valid, report.errors.len());

// Extract ontology
let resources = validate::extract_ontology(
    "http://localhost:9883/ontology",
    None,
)?;
println!("Extracted {} resources", resources.len());

// Test connectivity
let info = validate::test_server_connection("http://localhost:9883", None)?;
println!("Latency: {}ms", info.latency_ms);

// Generate diff between servers
let diff = validate::diff_servers(
    "http://source:9883",
    "http://target:9884",
    None,
)?;
println!("Differences: {}", diff.summary.total_differences);

// Synchronize servers
let options = SyncOptions {
    mode: SyncMode::Push,
    conflict_strategy: ConflictStrategy::Latest,
    dry_run: true,
    ..Default::default()
};
let report = validate::sync_servers(
    "http://source:9883",
    "http://target:9884",
    Some("agent-secret".to_string()),
    options,
)?;
println!("Created: {}, Updated: {}", report.resources_created, report.resources_updated);
```

## Error Codes

The validation service provides comprehensive error codes organized by validation level:

### Level 0 - Structural
- `InvalidJsonAd` - Malformed JSON-AD
- `MalformedAtom` - Invalid triple structure
- `InvalidSubjectUrl` - Bad subject URL format

### Level 1 - Datatype
- `DatatypeMismatch` - Value doesn't match declared type
- `InvalidUri` - Invalid URI format (NEW)
- `InvalidJson` - Invalid JSON structure (NEW)
- Plus validation for all other 10 datatypes

### Level 2 - Schema
- `MissingIsA` - Resource lacks class declaration
- `MissingRequiredProperty` - Required property not present
- `PropertyNotFound` - Unknown property URL
- `ClassNotFound` - Unknown class URL

### Level 3 - Referential
- `BrokenReference` - Referenced resource doesn't exist
- `CircularParent` - Circular parent hierarchy
- `OrphanedResource` - Resource without valid parent

### Level 4 - Cryptographic
- `InvalidSignature` - ED25519 signature verification failed
- `MissingCommitSignature` - Commit lacks required signature
- `FutureTimestamp` - Commit timestamp in the future
- `CommitChainBroken` - previousCommit mismatch

### Level 5 - Authorization
- `InsufficientWriteRights` - Agent lacks write permission
- `InsufficientAppendRights` - Agent lacks append permission

## Schema Version Detection

The service automatically detects server schema versions:

- **V1**: Original 10 datatypes (no Uri/JSON, has Value::Resource)
- **V2**: Current 12 datatypes (Uri, JSON, ResourceResponse support)

```bash
$ atomic-cli detect-version http://localhost:9883
Schema Version: V2 (12 datatypes - includes Uri/JSON)
OK: Server supports all current Atomic Data features.
```

## Testing

The validation service includes comprehensive unit tests:

```bash
# Run all tests
cargo test --package atomic-cli

# Current test count: 31 tests
# - 5 client tests (retry logic, connectivity)
# - 7 crypto tests (signatures, timestamps)
# - 3 extractor tests (schema detection)
# - 7 sync tests (diff, conflict resolution)
# - 6 validator tests (datatype validation)
# - 3 module tests (parsing, conversion)
```

## Future Enhancements

- [ ] Incremental synchronization (only changed resources)
- [ ] Webhook support for real-time sync
- [ ] Conflict resolution UI
- [ ] Performance benchmarking suite
- [ ] Multi-server mesh synchronization
- [ ] Backup/restore functionality
- [ ] Migration tools for V1 to V2 servers
