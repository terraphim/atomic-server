# Atomic Server Validation Service: Formal Specification

**Version:** 1.0.0
**Date:** 2025-11-06
**Status:** Draft

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Background and Context](#background-and-context)
3. [Data Model Specification](#data-model-specification)
4. [Validation Requirements](#validation-requirements)
5. [Service Architecture](#service-architecture)
6. [Language Evaluation](#language-evaluation)
7. [Implementation Plan](#implementation-plan)
8. [Appendices](#appendices)

---

## 1. Executive Summary

This document specifies a formal validation service for Atomic Server implementations. The service shall:

- **Validate** atomic server implementations against the Atomic Data specification
- **Synchronize** all data types between two atomic instances, including ontologies, taxonomies, resources, and metadata
- **Verify** data integrity, schema compliance, and cryptographic signatures
- **Report** discrepancies and validation errors in a structured format
- **Enable** bidirectional synchronization with conflict resolution

The validation service acts as both a compliance checker and a synchronization engine, ensuring that atomic server instances maintain specification conformance while enabling reliable data migration and replication.

---

## 2. Background and Context

### 2.1 Atomic Data Overview

Atomic Data is a specification for linked, typed, graph data. Key characteristics:

- **Triple-based model**: All data represented as (Subject, Property, Value) atoms
- **Type safety**: Properties define datatypes; values are validated
- **Event-sourced**: Changes tracked through cryptographically-signed commits
- **Hierarchical authorization**: Tree-based permission inheritance
- **Linked data**: URLs as identifiers for resources, properties, and classes

### 2.2 Current Implementation Status

**Rust Implementation** (`atomic-lib` + `atomic-server`):
- Location: `/home/user/atomic-server/lib/` and `/home/user/atomic-server/server/`
- Features: Full specification implementation, Actix-web HTTP server, Sled database storage
- Strengths: Performance, type safety, complete feature set

**TypeScript Implementation** (`@tomic/lib`):
- Location: `/home/user/atomic-server/browser/lib/`
- Features: Client-side store, resource management, commit building
- Strengths: Browser compatibility, developer ergonomics, ecosystem integration

**Existing Tools**:
- `extract-ontology`: Deno-based tool for ontology extraction and serialization
- CLI tools for server management
- Browser-based data browser application

### 2.3 Validation Gap

Currently, no formal validation service exists to:
1. Verify cross-implementation compatibility
2. Validate complete server state against specification
3. Synchronize entire databases including ontologies and taxonomies
4. Provide compliance testing and certification

---

## 3. Data Model Specification

### 3.1 Core Data Types

#### 3.1.1 Primitive Datatypes

| Datatype | Rust Type | JSON Representation | Validation Rules |
|----------|-----------|-------------------|------------------|
| String | `String` | `"text"` | UTF-8, no constraints |
| Integer | `i64` | `42` | Signed 64-bit integer |
| Float | `f64` | `3.14` | 64-bit IEEE 754 |
| Boolean | `bool` | `true` or `false` | Boolean literal |
| Date | `String` | `"2025-11-06"` | ISO 8601 format: YYYY-MM-DD |
| Timestamp | `i64` | `1730851200000` | Unix epoch milliseconds |
| Slug | `String` | `"url-safe-text"` | Lowercase, alphanumeric + hyphens |
| Markdown | `String` | `"# Heading"` | Valid UTF-8, markdown syntax |
| AtomicUrl | `String` | `"https://example.com/resource"` | Valid HTTP(S) URL |
| ResourceArray | `Vec<String>` | `["url1", "url2"]` | Array of AtomicUrls |

**File:** `lib/src/datatype.rs:1-100`

#### 3.1.2 Composite Structures

**Atom** (Triple):
```rust
pub struct Atom {
    pub subject: String,    // Resource URL
    pub property: String,   // Property URL
    pub value: Value,       // Typed value
}
```
**File:** `lib/src/atoms.rs:10-15`

**Resource** (Entity):
```rust
pub struct Resource {
    propvals: PropVals,           // HashMap<String, Value>
    subject: String,              // Unique identifier URL
    commit: CommitBuilder,        // Change tracking
}
```
**File:** `lib/src/resources.rs:20-25`

**Value** (Discriminated Union):
```rust
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Date(String),
    Timestamp(i64),
    Slug(String),
    Markdown(String),
    AtomicUrl(String),
    ResourceArray(Vec<SubResource>),
    NestedResource(SubResource),
    Resource(Box<Resource>),
    Unsupported(UnsupportedValue),
}
```
**File:** `lib/src/values.rs:15-30`

### 3.2 Schema Specification

#### 3.2.1 Property Definition

```rust
pub struct Property {
    pub subject: String,              // Property URL
    pub shortname: String,            // e.g., "name"
    pub description: String,          // Human-readable explanation
    pub data_type: DataType,          // Value constraint
    pub class_type: Option<String>,   // Expected resource class
    pub allows_only: Option<Vec<String>>, // Enum values (optional)
}
```
**File:** `lib/src/schema.rs:50-60`

**Validation Rules:**
1. `subject` must be a valid HTTP(S) URL
2. `shortname` must be a valid Slug
3. `data_type` must reference a known DataType
4. If `data_type` is AtomicUrl, `class_type` may constrain the target resource class
5. If `allows_only` is present, values must be in the enumerated set

#### 3.2.2 Class Definition

```rust
pub struct Class {
    pub subject: String,              // Class URL
    pub shortname: String,            // e.g., "Document"
    pub description: String,          // Human-readable explanation
    pub requires: Vec<String>,        // Required property URLs
    pub recommends: Vec<String>,      // Optional recommended properties
}
```
**File:** `lib/src/schema.rs:70-80`

**Validation Rules:**
1. `subject` must be a valid HTTP(S) URL
2. `requires` properties must all be fetchable Property resources
3. `recommends` properties must all be fetchable Property resources
4. No circular dependencies in class inheritance

#### 3.2.3 Ontology Structure

An **Ontology** is a collection of related Classes, Properties, and instance resources:

```typescript
interface Ontology {
  '@id': string;                      // Ontology URL
  'https://atomicdata.dev/properties/classes': string[];    // Class URLs
  'https://atomicdata.dev/properties/properties': string[]; // Property URLs
  'https://atomicdata.dev/properties/instances'?: string[]; // Instance URLs
  'https://atomicdata.dev/properties/description': string;
  'https://atomicdata.dev/properties/shortname': string;
}
```

**Examples:**
- Core ontology: `https://atomicdata.dev/ontology/core`
- Server ontology: `https://atomicdata.dev/ontology/server`
- Chatroom ontology: `https://atomicdata.dev/ontology/chatroom`

**File:** `lib/src/plugins/mod.rs:1-50`

### 3.3 Commit Structure (Event Sourcing)

```rust
pub struct Commit {
    pub subject: String,                    // Resource being modified
    pub created_at: i64,                    // Unix timestamp (ms)
    pub signer: String,                     // Agent URL
    pub set: Option<HashMap<String, Value>>, // Properties to add/update
    pub remove: Option<Vec<String>>,        // Properties to delete
    pub destroy: Option<bool>,              // Delete entire resource
    pub push: Option<HashMap<String, Value>>, // Append to arrays
    pub signature: Option<String>,          // ED25519 signature (base64)
    pub previous_commit: Option<String>,    // Previous commit URL
    pub url: Option<String>,                // This commit's URL
}
```
**File:** `lib/src/commit.rs:20-35`

**Validation Rules:**
1. `signature` must be valid ED25519 signature of: `{subject} {created_at}` signed by `signer`
2. `created_at` must be within acceptable time window (default: ±10 seconds)
3. `signer` must have Write rights for `subject`
4. If `previous_commit` is present, it must exist and point to the previous state
5. `set`, `remove`, `push`, and `destroy` are mutually compatible operations

### 3.4 Taxonomy and Hierarchy

Atomic Data supports hierarchical organization via the `parent` property:

```
Drive (root)
├── Folder A
│   ├── Document 1
│   └── Document 2
└── Folder B
    └── Document 3
```

**Key Properties:**
- `https://atomicdata.dev/properties/parent`: Defines hierarchical relationship
- `https://atomicdata.dev/properties/read`: Inherited read permissions
- `https://atomicdata.dev/properties/write`: Inherited write permissions
- `https://atomicdata.dev/properties/append`: Inherited creation permissions

**Validation Rules:**
1. No circular parent relationships
2. `parent` must reference an existing resource
3. Permissions are inherited from ancestors
4. Root resources (Drives) have no parent

**File:** `lib/src/hierarchy.rs:1-100`

---

## 4. Validation Requirements

### 4.1 Structural Validation

#### 4.1.1 Atom-Level Validation

**Requirements:**
- [ ] Subject must be a valid HTTP(S) URL
- [ ] Property must be a valid HTTP(S) URL and reference a fetchable Property resource
- [ ] Value must conform to the Property's specified datatype
- [ ] If Property has `allows_only`, value must be in the enumerated set

**Implementation Reference:** `lib/src/validate.rs:50-100`

#### 4.1.2 Resource-Level Validation

**Requirements:**
- [ ] Subject must be unique within the store
- [ ] Must include `isA` property (class membership)
- [ ] All required properties (per class) must be present
- [ ] All property values must match their datatype constraints
- [ ] No unknown/invalid properties (unless unsupported flag set)

**Implementation Reference:** `lib/src/resources.rs:200-250`

#### 4.1.3 Schema Validation

**Requirements:**
- [ ] All referenced Properties must exist
- [ ] All referenced Classes must exist
- [ ] No circular class dependencies
- [ ] Property datatypes must reference valid DataType resources
- [ ] Class requirements must not conflict with recommends

### 4.2 Cryptographic Validation

#### 4.2.1 Signature Verification

**Requirements:**
- [ ] Signature must be valid ED25519 signature
- [ ] Signed message format: `{subject} {timestamp}`
- [ ] Public key must match the Agent's stored key
- [ ] Timestamp must be within acceptable window (default: ±10 seconds)

**Implementation Reference:** `lib/src/authentication.rs:50-100`

#### 4.2.2 Commit Chain Validation

**Requirements:**
- [ ] `previous_commit` must exist (if present)
- [ ] Commit chain must be unbroken
- [ ] No double-application of commits
- [ ] Timestamps must be monotonically increasing in chain

**Implementation Reference:** `lib/src/commit.rs:100-150`

### 4.3 Authorization Validation

**Requirements:**
- [ ] Agent performing commit must have Write rights for resource
- [ ] Read access validated for fetch operations
- [ ] Append rights validated for creating child resources
- [ ] Rights inherited from parent hierarchy

**Implementation Reference:** `lib/src/hierarchy.rs:100-200`

### 4.4 Semantic Validation

#### 4.4.1 Referential Integrity

**Requirements:**
- [ ] All AtomicUrl values must reference fetchable resources (or local IDs)
- [ ] ResourceArray elements must all be valid URLs
- [ ] `parent` relationships must not create cycles
- [ ] Collections must reference valid property-value queries

#### 4.4.2 Ontology Completeness

**Requirements:**
- [ ] All classes referenced in `isA` must be defined
- [ ] All properties used must be defined
- [ ] Ontology resources must list their classes and properties
- [ ] No orphaned schema resources (unreferenced by any ontology)

### 4.5 Synchronization Requirements

#### 4.5.1 Data Extraction

The validation service must extract:
1. **Ontologies**: All Ontology resources with their metadata
2. **Classes**: All Class definitions
3. **Properties**: All Property definitions
4. **Instances**: All non-schema resources
5. **Commits**: Complete commit history
6. **Agents**: All Agent resources with public keys
7. **Drives**: Root-level containers
8. **Collections**: Dynamic query definitions

#### 4.5.2 Data Comparison

The service must compare two atomic instances:
- **Structural diff**: Resources present in A but not B, and vice versa
- **Content diff**: Resources with different property values
- **Commit history diff**: Different event sequences
- **Schema diff**: Different ontology versions

#### 4.5.3 Conflict Resolution

Strategies:
1. **Last-write-wins**: Use timestamp to resolve conflicts
2. **Source-preferred**: Always prefer source instance
3. **Destination-preferred**: Always prefer destination instance
4. **Manual resolution**: Report conflicts for human decision
5. **Merge**: Combine non-conflicting changes (requires operational transformation)

---

## 5. Service Architecture

### 5.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Validation Service                        │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Extractor  │  │  Validator   │  │ Synchronizer │     │
│  │              │  │              │  │              │     │
│  │ - Fetch      │  │ - Structure  │  │ - Compare    │     │
│  │ - Parse      │  │ - Schema     │  │ - Diff       │     │
│  │ - Traverse   │  │ - Crypto     │  │ - Merge      │     │
│  │ - Serialize  │  │ - Auth       │  │ - Resolve    │     │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘     │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
└────────────────────────────┼─────────────────────────────────┘
                             │
            ┌────────────────┴────────────────┐
            │                                  │
    ┌───────▼────────┐              ┌─────────▼────────┐
    │  Source Server │              │  Target Server   │
    │                │              │                  │
    │  Atomic Data   │              │  Atomic Data     │
    │  Store         │              │  Store           │
    └────────────────┘              └──────────────────┘
```

### 5.2 Component Specifications

#### 5.2.1 Extractor Component

**Purpose:** Fetch and serialize all data from an atomic server instance.

**Interfaces:**
```typescript
interface Extractor {
  // Fetch entire store
  extractAll(serverUrl: string, agent?: Agent): Promise<StoreSnapshot>

  // Fetch specific ontology with dependencies
  extractOntology(ontologyUrl: string, agent?: Agent): Promise<OntologySnapshot>

  // Fetch resource tree from root
  extractTree(rootUrl: string, depth: number, agent?: Agent): Promise<ResourceTree>

  // Stream resources (for large stores)
  streamResources(serverUrl: string, agent?: Agent): AsyncIterator<Resource>
}

interface StoreSnapshot {
  serverUrl: string;
  ontologies: Ontology[];
  classes: Class[];
  properties: Property[];
  resources: Resource[];
  commits: Commit[];
  agents: Agent[];
  extractedAt: Timestamp;
  metadata: SnapshotMetadata;
}
```

**Algorithm (Ontology Extraction):**
```
function extractOntology(url):
  1. Fetch ontology resource
  2. Extract classes list
  3. Extract properties list
  4. Extract instances list (optional)
  5. For each class URL:
     a. Fetch class resource
     b. Extract requires and recommends
     c. Recursively fetch referenced properties
  6. For each property URL:
     a. Fetch property resource
     b. Extract datatype and constraints
  7. For each instance URL:
     a. Fetch instance resource recursively
  8. Build local ID mapping (absolute URL → relative path)
  9. Serialize to JSON-AD with local IDs
  10. Return structured snapshot
```

**Implementation Notes:**
- Leverage existing `extract-ontology` script as reference
- Handle authentication via Agent secrets
- Support rate limiting and retry logic
- Validate fetched resources during extraction
- Cache fetched resources to avoid duplicate requests

**File Reference:** `/tmp/extract-ontology/src/main.ts`

#### 5.2.2 Validator Component

**Purpose:** Validate store snapshots against the Atomic Data specification.

**Interfaces:**
```typescript
interface Validator {
  // Validate complete snapshot
  validateSnapshot(snapshot: StoreSnapshot): ValidationReport

  // Validate single resource
  validateResource(resource: Resource, store: Store): ResourceValidation

  // Validate schema consistency
  validateSchema(classes: Class[], properties: Property[]): SchemaValidation

  // Validate commit chain
  validateCommitChain(commits: Commit[]): CommitChainValidation

  // Validate authorization rules
  validateAuthorization(resource: Resource, agent: Agent, store: Store): AuthValidation
}

interface ValidationReport {
  valid: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
  summary: {
    totalResources: number;
    validResources: number;
    invalidResources: number;
    missingReferences: number;
    schemaViolations: number;
    signatureFailures: number;
  };
}

interface ValidationError {
  severity: 'error' | 'warning' | 'info';
  code: string;              // e.g., 'MISSING_REQUIRED_PROPERTY'
  subject: string;           // Resource URL
  property?: string;         // Property URL (if applicable)
  message: string;           // Human-readable description
  context?: any;             // Additional debug information
}
```

**Validation Levels:**
1. **Level 0 - Structural**: JSON-AD parsing, atom structure
2. **Level 1 - Datatype**: Value conforms to datatype constraints
3. **Level 2 - Schema**: Required properties present, class compliance
4. **Level 3 - Referential**: All URLs resolve, no broken links
5. **Level 4 - Cryptographic**: Signatures valid, commits authentic
6. **Level 5 - Authorization**: Permissions correctly enforced

**Implementation Reference:** `lib/src/validate.rs`

#### 5.2.3 Synchronizer Component

**Purpose:** Synchronize data between two atomic server instances.

**Interfaces:**
```typescript
interface Synchronizer {
  // Compare two snapshots
  compare(source: StoreSnapshot, target: StoreSnapshot): DiffReport

  // Synchronize source → target
  sync(source: string, target: string, options: SyncOptions): Promise<SyncReport>

  // Bidirectional sync with conflict resolution
  bidirectionalSync(instanceA: string, instanceB: string, options: SyncOptions): Promise<SyncReport>

  // Apply diff to target
  applyDiff(diff: DiffReport, target: string, agent: Agent): Promise<ApplyReport>
}

interface DiffReport {
  sourceOnly: Resource[];      // Resources in source but not target
  targetOnly: Resource[];      // Resources in target but not source
  modified: ResourceDiff[];    // Resources with different values
  schemaChanges: SchemaDiff;   // Ontology differences
  summary: DiffSummary;
}

interface ResourceDiff {
  subject: string;
  addedProperties: Map<string, Value>;
  removedProperties: string[];
  modifiedProperties: Map<string, [Value, Value]>; // [source, target]
}

interface SyncOptions {
  mode: 'push' | 'pull' | 'bidirectional';
  conflictStrategy: 'source' | 'target' | 'latest' | 'manual' | 'merge';
  includeOntologies: boolean;
  includeCommitHistory: boolean;
  dryRun: boolean;
  filterClasses?: string[];     // Only sync specific classes
  filterSubjects?: string[];    // Only sync specific subjects
  agent: Agent;                 // Authentication
}

interface SyncReport {
  success: boolean;
  resourcesCreated: number;
  resourcesUpdated: number;
  resourcesDeleted: number;
  errors: SyncError[];
  conflicts: Conflict[];
}
```

**Synchronization Algorithm:**
```
function sync(source, target, options):
  1. Extract snapshot from source
  2. Extract snapshot from target
  3. Compare snapshots → generate diff
  4. Validate diff (ensure no critical conflicts)

  5. If options.includeOntologies:
     a. Sync ontologies first (dependencies)
     b. Sync properties
     c. Sync classes

  6. Sort resources by dependency (parents before children)

  7. For each resource in diff.sourceOnly:
     a. Create resource in target via POST
     b. If authentication required, sign with agent
     c. Handle errors (log and continue)

  8. For each resource in diff.modified:
     a. Resolve conflict per options.conflictStrategy
     b. Create commit for changes
     c. POST commit to target
     d. Handle errors

  9. For each resource in diff.targetOnly:
     a. If options.mode == 'push':
        - Delete from target (optional)
     b. If options.mode == 'pull':
        - Ignore (keep target data)

  10. Return sync report with statistics
```

**Conflict Resolution Strategies:**
- **Source-preferred**: Always use source value
- **Target-preferred**: Always use target value
- **Latest-wins**: Use the resource with the most recent commit
- **Manual**: Prompt user or log for manual resolution
- **Merge**: Combine non-conflicting changes (complex, requires operational transformation)

#### 5.2.4 CLI Interface

```bash
# Validate a server instance
atomic-validate server <url> [--agent <secret>] [--level <0-5>] [--output <json|text>]

# Extract and save snapshot
atomic-validate extract <url> --out <file> [--agent <secret>] [--ontology <url>]

# Compare two instances
atomic-validate compare <source-url> <target-url> [--agent-source <secret>] [--agent-target <secret>]

# Synchronize instances
atomic-validate sync <source-url> <target-url> \
  --mode <push|pull|bidirectional> \
  --conflict <source|target|latest|manual> \
  --agent <secret> \
  [--dry-run] \
  [--include-ontologies] \
  [--filter-class <class-url>]

# Validate local snapshot file
atomic-validate validate-file <snapshot.json> [--level <0-5>]
```

### 5.3 Data Flow

#### Validation Flow
```
User → CLI → Extractor → Server A (fetch resources)
                ↓
            Store Snapshot
                ↓
           Validator → Validation Report → User
```

#### Synchronization Flow
```
User → CLI → Synchronizer
                ↓
         ┌──────┴──────┐
         ↓              ↓
    Extractor A    Extractor B
         ↓              ↓
    Snapshot A    Snapshot B
         └──────┬──────┘
                ↓
           Comparator
                ↓
           Diff Report
                ↓
        Conflict Resolver
                ↓
           Apply Diff → Server B (POST commits)
                ↓
           Sync Report → User
```

### 5.4 Error Handling

**Error Categories:**
1. **Network Errors**: Connection failures, timeouts
2. **Authentication Errors**: Invalid agent, signature failures
3. **Authorization Errors**: Insufficient permissions
4. **Validation Errors**: Schema violations, datatype mismatches
5. **Conflict Errors**: Irresolvable differences during sync

**Retry Strategy:**
- Network errors: Exponential backoff (2s, 4s, 8s, 16s, max 4 retries)
- Authentication errors: Fail immediately (no retry)
- Validation errors: Collect all, report at end
- Conflicts: Resolve per strategy or prompt user

### 5.5 Performance Considerations

**Scalability:**
- Stream large datasets (avoid loading entire store in memory)
- Batch requests (reduce HTTP overhead)
- Parallel fetching (concurrent resource retrieval)
- Incremental validation (validate as resources arrive)
- Index-based queries (leverage server indexes)

**Caching:**
- Cache fetched resources during extraction
- Cache property definitions (reused across resources)
- Cache class definitions (reused across resources)
- Use ETags for conditional requests (avoid redundant fetches)

**Optimization:**
- Topological sort for dependency resolution
- Bloom filters for existence checks
- Merkle trees for snapshot comparison (future enhancement)

---

## 6. Language Evaluation

### 6.1 Rust

**Advantages:**
- Existing implementation: `atomic-lib` provides all necessary primitives
- Type safety: Prevents many classes of bugs at compile time
- Performance: Zero-cost abstractions, memory safety without GC
- Ecosystem: Serde for serialization, Tokio for async, Actix-web for HTTP
- Code reuse: Can directly import and use `atomic-lib` crate
- Cryptography: Mature libraries (ed25519-dalek, ring)

**Disadvantages:**
- Steep learning curve for contributors unfamiliar with Rust
- Longer compile times compared to interpreted languages
- More verbose error handling (Result types)

**Implementation Approach:**
```toml
[dependencies]
atomic_lib = { path = "../lib" }
tokio = { version = "1", features = ["full"] }
reqwest = "0.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = "4"  # CLI argument parsing
```

**Feasibility:** HIGH - Directly leverages existing codebase, minimal duplication.

**Code Example:**
```rust
use atomic_lib::{Resource, Store, Storelike};

async fn extract_ontology(url: &str) -> Result<Vec<Resource>, Error> {
    let store = Store::init()?;
    let ontology = store.get_resource(url).await?;
    let classes = ontology.get_classes()?;
    let properties = ontology.get_properties()?;

    let mut resources = vec![ontology];
    for class_url in classes {
        resources.push(store.get_resource(&class_url).await?);
    }
    for prop_url in properties {
        resources.push(store.get_resource(&prop_url).await?);
    }

    Ok(resources)
}
```

**File Location:** `cli/src/validate.rs` (new file)

### 6.2 OCaml

**Advantages:**
- Strong type system: Algebraic data types, pattern matching
- Functional paradigm: Natural for data transformation pipelines
- Performance: Compiled, efficient GC
- Expressive: Concise syntax for complex operations
- Strong libraries: Lwt/Async for concurrency, Cohttp for HTTP

**Disadvantages:**
- No existing Atomic Data implementation in OCaml
- Must reimplement all data structures and logic
- Smaller ecosystem compared to Rust/TypeScript
- Less mainstream adoption (harder to find contributors)
- Cryptographic libraries less mature

**Implementation Approach:**
```ocaml
(* Define core types *)
type value =
  | String of string
  | Integer of int64
  | Float of float
  | Boolean of bool
  | AtomicUrl of string
  | ResourceArray of string list

type atom = {
  subject: string;
  property: string;
  value: value;
}

type resource = {
  subject: string;
  propvals: (string * value) list;
}

(* Validator module *)
module Validator = struct
  let validate_resource (r: resource) (store: store) : validation_result =
    (* Implementation *)
end
```

**Feasibility:** MEDIUM - Requires significant reimplementation, but language is well-suited.

**Challenges:**
- Must reimplement JSON-AD parsing
- Must reimplement datatype validation
- Must reimplement cryptographic verification
- Interop with existing Rust/TypeScript codebases limited

### 6.3 TypeScript (Bun)

**Advantages:**
- Existing implementation: `@tomic/lib` provides client-side logic
- Developer ergonomics: Familiar syntax, large ecosystem
- Bun benefits: Fast startup, built-in bundler, native TypeScript support
- JSON-native: Natural handling of JSON-AD format
- npm ecosystem: Rich library support (crypto, CLI tools, etc.)
- Easier contributions: Lower barrier to entry

**Disadvantages:**
- `@tomic/lib` is client-focused, not full server implementation
- Type safety weaker than Rust/OCaml (runtime errors possible)
- Performance lower than compiled languages (but Bun mitigates this)
- Async complexity: Promise chains, error handling verbose

**Implementation Approach:**
```typescript
import { Store, Resource, Agent } from '@tomic/lib';

async function extractOntology(url: string, agent?: Agent): Promise<Resource[]> {
  const store = new Store({ serverUrl: new URL(url).origin, agent });
  const ontology = await store.getResource(url);

  const classes = ontology.props.classes ?? [];
  const properties = ontology.props.properties ?? [];

  const resources = [ontology];

  for (const classUrl of classes) {
    resources.push(await store.getResource(classUrl));
  }

  for (const propUrl of properties) {
    resources.push(await store.getResource(propUrl));
  }

  return resources;
}
```

**Feasibility:** HIGH - Leverages existing library, rapid development.

**Enhancement Needed:**
- Extend `@tomic/lib` with validation logic
- Add server-side features (commit chain validation)
- Implement synchronization engine

**File Location:** `browser/lib/src/validate.ts` (new file)

### 6.4 Recommendation Matrix

| Criteria | Rust | OCaml | TypeScript (Bun) |
|----------|------|-------|------------------|
| Code Reuse | ★★★★★ | ★☆☆☆☆ | ★★★★☆ |
| Performance | ★★★★★ | ★★★★☆ | ★★★☆☆ |
| Type Safety | ★★★★★ | ★★★★★ | ★★★☆☆ |
| Ecosystem | ★★★★☆ | ★★☆☆☆ | ★★★★★ |
| Learning Curve | ★★☆☆☆ | ★★☆☆☆ | ★★★★★ |
| Dev Speed | ★★★☆☆ | ★★★☆☆ | ★★★★☆ |
| Maintainability | ★★★★☆ | ★★★★☆ | ★★★☆☆ |
| Contributor Accessibility | ★★★☆☆ | ★★☆☆☆ | ★★★★★ |
| **TOTAL** | **29/40** | **21/40** | **29/40** |

### 6.5 Final Recommendation

**Primary Implementation: Rust**

**Rationale:**
1. **Maximal code reuse**: Direct use of `atomic-lib` crate eliminates duplication
2. **Consistency**: Same language as server ensures spec alignment
3. **Performance**: Critical for large-scale synchronization operations
4. **Type safety**: Prevents entire classes of bugs in complex validation logic
5. **Cryptographic robustness**: Mature libraries for signature verification

**Secondary Implementation: TypeScript (Bun)**

**Rationale:**
1. **Accessibility**: Easier for web developers to contribute
2. **Rapid prototyping**: Faster iteration for experimental features
3. **Client-side use**: Can run in browser for distributed validation
4. **Complementary**: TypeScript validator can validate Rust implementation

**Not Recommended: OCaml**

**Rationale:**
- Requires full reimplementation with no reuse benefits
- Smaller community, harder to maintain
- No compelling advantage over Rust or TypeScript for this use case

---

## 7. Implementation Plan

### 7.1 Phase 1: Foundation (Weeks 1-3)

**Goals:**
- Set up project structure
- Implement basic extractor
- Implement level 0-2 validation

**Deliverables:**

#### 7.1.1 Project Setup
- [ ] Create `cli/src/validate/` directory structure
- [ ] Add `atomic-validate` binary to `Cargo.toml`
- [ ] Set up CLI argument parsing with `clap`
- [ ] Configure logging with `tracing`
- [ ] Set up integration tests

**Files to Create:**
- `cli/src/validate/mod.rs` - Module root
- `cli/src/validate/extractor.rs` - Extraction logic
- `cli/src/validate/validator.rs` - Validation logic
- `cli/src/validate/cli.rs` - CLI interface
- `cli/src/validate/types.rs` - Type definitions

#### 7.1.2 Basic Extractor
- [ ] Implement `fetch_resource_recursive()` - Traverse resource tree
- [ ] Implement `fetch_ontology()` - Extract ontology with dependencies
- [ ] Implement `serialize_snapshot()` - Save to JSON-AD file
- [ ] Add progress indicators for long operations
- [ ] Handle authentication via Agent secrets

**Key Functions:**
```rust
async fn fetch_resource_recursive(
    store: &impl Storelike,
    subject: &str,
    depth: usize,
    visited: &mut HashSet<String>,
) -> AtomicResult<Vec<Resource>>

async fn fetch_ontology(
    store: &impl Storelike,
    ontology_url: &str,
) -> AtomicResult<OntologySnapshot>
```

#### 7.1.3 Level 0-2 Validation
- [ ] Level 0: JSON-AD parsing validation
- [ ] Level 1: Datatype validation (leverage `Value::new()`)
- [ ] Level 2: Schema validation (leverage `Resource::check_required_props()`)
- [ ] Generate validation reports with error details
- [ ] Support JSON and human-readable output formats

**Key Functions:**
```rust
fn validate_snapshot(
    snapshot: &StoreSnapshot,
    level: ValidationLevel,
) -> ValidationReport

fn validate_resource(
    resource: &Resource,
    store: &impl Storelike,
    level: ValidationLevel,
) -> ResourceValidation
```

### 7.2 Phase 2: Advanced Validation (Weeks 4-6)

**Goals:**
- Implement level 3-5 validation
- Add referential integrity checks
- Implement cryptographic verification

**Deliverables:**

#### 7.2.1 Referential Integrity
- [ ] Check all AtomicUrl values resolve
- [ ] Validate ResourceArray elements exist
- [ ] Check parent relationships don't create cycles
- [ ] Validate collection queries are valid
- [ ] Report broken links and orphaned resources

#### 7.2.2 Cryptographic Validation
- [ ] Verify ED25519 signatures on commits
- [ ] Validate timestamp windows
- [ ] Check commit chain continuity
- [ ] Verify agent public keys match signatures
- [ ] Detect and report double-application of commits

**Reference Implementation:** `lib/src/authentication.rs`, `lib/src/commit.rs`

#### 7.2.3 Authorization Validation
- [ ] Check write permissions for commits
- [ ] Validate read permissions for fetch operations
- [ ] Verify hierarchical permission inheritance
- [ ] Detect unauthorized modifications
- [ ] Report permission violations

**Reference Implementation:** `lib/src/hierarchy.rs`

### 7.3 Phase 3: Synchronization Engine (Weeks 7-10)

**Goals:**
- Implement comparison and diff generation
- Implement synchronization with conflict resolution
- Add bidirectional sync support

**Deliverables:**

#### 7.3.1 Comparison Engine
- [ ] Implement `compare_snapshots()` - Generate diff report
- [ ] Detect structural differences (resource presence)
- [ ] Detect content differences (property values)
- [ ] Detect schema differences (ontology changes)
- [ ] Generate detailed diff reports (JSON and text)

**Key Functions:**
```rust
fn compare_snapshots(
    source: &StoreSnapshot,
    target: &StoreSnapshot,
) -> DiffReport

fn compare_resources(
    source: &Resource,
    target: &Resource,
) -> ResourceDiff
```

#### 7.3.2 Synchronization Logic
- [ ] Implement `sync_push()` - Source → Target
- [ ] Implement `sync_pull()` - Target → Source
- [ ] Implement topological sort for dependency resolution
- [ ] Handle ontology synchronization first (dependencies)
- [ ] Create commits for changes
- [ ] POST commits to target server with authentication

**Key Functions:**
```rust
async fn sync_push(
    source: &StoreSnapshot,
    target_url: &str,
    agent: &Agent,
    options: &SyncOptions,
) -> AtomicResult<SyncReport>
```

#### 7.3.3 Conflict Resolution
- [ ] Implement source-preferred strategy
- [ ] Implement target-preferred strategy
- [ ] Implement latest-wins strategy (based on commit timestamps)
- [ ] Implement manual resolution (interactive prompts)
- [ ] Log conflicts for later review
- [ ] Support conflict resolution via configuration file

**Conflict Detection:**
```rust
struct Conflict {
    subject: String,
    property: String,
    source_value: Value,
    target_value: Value,
    source_timestamp: Option<i64>,
    target_timestamp: Option<i64>,
}

fn detect_conflicts(diff: &DiffReport) -> Vec<Conflict>
fn resolve_conflict(conflict: &Conflict, strategy: ConflictStrategy) -> Value
```

### 7.4 Phase 4: Testing and Documentation (Weeks 11-12)

**Goals:**
- Comprehensive testing
- Documentation and examples
- Performance benchmarking

**Deliverables:**

#### 7.4.1 Testing
- [ ] Unit tests for all validators
- [ ] Integration tests with live servers
- [ ] Test synchronization scenarios (clean, conflict, partial)
- [ ] Test all conflict resolution strategies
- [ ] Test large dataset handling (streaming, performance)
- [ ] Test error conditions (network failures, auth errors)

**Test Scenarios:**
1. **Clean sync**: Empty target, full source
2. **Partial sync**: Some overlap, some new data
3. **Conflict sync**: Same resources, different values
4. **Schema migration**: Ontology changes between instances
5. **Large dataset**: 10,000+ resources
6. **Network failures**: Retry logic, partial failures

#### 7.4.2 Documentation
- [ ] CLI usage guide with examples
- [ ] API documentation (rustdoc)
- [ ] Architecture documentation (diagrams)
- [ ] Synchronization guide (strategies, best practices)
- [ ] Troubleshooting guide (common errors, solutions)
- [ ] Contributing guide for validators

**Documentation Files:**
- `docs/validation-service.md` - Overview and architecture
- `docs/validation-levels.md` - Detailed validation rules
- `docs/synchronization-guide.md` - Sync strategies and examples
- `cli/README.md` - CLI usage

#### 7.4.3 Performance Benchmarking
- [ ] Benchmark extraction speed (resources/second)
- [ ] Benchmark validation speed (resources/second per level)
- [ ] Benchmark synchronization speed (commits/second)
- [ ] Profile memory usage for large datasets
- [ ] Optimize bottlenecks (parallel fetching, batching)

### 7.5 Phase 5: Advanced Features (Future)

**Potential Enhancements:**

#### 7.5.1 Incremental Synchronization
- Use commit timestamps to sync only recent changes
- WebSocket-based real-time sync
- Conflict-free replicated data types (CRDTs) for merge strategies

#### 7.5.2 Merkle Tree Optimization
- Build merkle trees for snapshots
- Fast snapshot comparison via root hash
- Efficient diff generation (only changed subtrees)

#### 7.5.3 Distributed Validation
- Peer-to-peer validation network
- Consensus-based validation (multiple validators)
- Blockchain-like audit trail

#### 7.5.4 Machine Learning Validation
- Anomaly detection for unusual data patterns
- Predictive validation (likely errors before they occur)
- Automated conflict resolution via learned preferences

#### 7.5.5 Web Interface
- Browser-based validation dashboard
- Visual diff viewer
- Interactive conflict resolution UI

---

## 8. Appendices

### 8.1 Glossary

- **Atom**: A single (subject, property, value) triple
- **Resource**: A collection of atoms sharing a subject
- **Commit**: A signed change record for a resource
- **Ontology**: A collection of related classes and properties
- **Class**: A schema definition with required/recommended properties
- **Property**: A schema definition with a datatype constraint
- **Agent**: A user or service with a public/private key pair
- **Drive**: A root-level container resource
- **Collection**: A dynamic query over resources
- **JSON-AD**: JSON-based Atomic Data serialization format

### 8.2 References

- **Atomic Data Specification**: https://docs.atomicdata.dev/
- **Atomic Server Repository**: https://github.com/atomicdata-dev/atomic-server
- **Extract Ontology Tool**: https://github.com/atomicdata-dev/extract-ontology
- **Rust atomic-lib**: `/home/user/atomic-server/lib/`
- **TypeScript @tomic/lib**: `/home/user/atomic-server/browser/lib/`

### 8.3 File Locations (Key Implementation Files)

**Rust Implementation:**
- Core library: `lib/src/`
  - `atoms.rs:10-15` - Atom definition
  - `values.rs:15-30` - Value enum
  - `resources.rs:20-250` - Resource implementation
  - `schema.rs:50-80` - Class and Property
  - `datatype.rs:1-100` - DataType enum
  - `validate.rs:1-200` - Validation logic
  - `commit.rs:20-150` - Commit structure and validation
  - `authentication.rs:50-100` - Signature verification
  - `hierarchy.rs:1-200` - Authorization checking
  - `serialize.rs:1-200` - JSON-AD serialization
  - `parse.rs:1-300` - JSON-AD parsing

**TypeScript Implementation:**
- Client library: `browser/lib/src/`
  - `resource.ts` - Resource class
  - `commit.ts` - Commit builder
  - `store.ts` - Client-side store
  - `datatypes.ts` - Type validation
  - `parse.ts` - JSON-AD parsing

**Extract Ontology:**
- `/tmp/extract-ontology/src/main.ts` - Extractor reference implementation

### 8.4 Example Commands

```bash
# Example 1: Validate a local server
atomic-validate server http://localhost:9883 \
  --agent "agent-secret-here" \
  --level 5 \
  --output json > validation-report.json

# Example 2: Extract and save a snapshot
atomic-validate extract https://atomicdata.dev \
  --ontology https://atomicdata.dev/ontology/core \
  --out core-ontology.json

# Example 3: Compare two servers
atomic-validate compare \
  https://server-a.example.com \
  https://server-b.example.com \
  --agent-source "secret-a" \
  --agent-target "secret-b" \
  --output diff-report.json

# Example 4: Synchronize servers (dry run)
atomic-validate sync \
  https://source.example.com \
  https://target.example.com \
  --mode push \
  --conflict latest \
  --agent "agent-secret" \
  --include-ontologies \
  --dry-run

# Example 5: Actual synchronization
atomic-validate sync \
  https://source.example.com \
  https://target.example.com \
  --mode push \
  --conflict source \
  --agent "agent-secret" \
  --include-ontologies

# Example 6: Bidirectional sync with manual conflict resolution
atomic-validate sync \
  https://server-a.example.com \
  https://server-b.example.com \
  --mode bidirectional \
  --conflict manual \
  --agent "agent-secret"

# Example 7: Validate a local snapshot file
atomic-validate validate-file snapshot.json \
  --level 3 \
  --output text
```

### 8.5 Success Metrics

**Phase 1 (Foundation):**
- [ ] CLI successfully extracts 1,000+ resource ontology
- [ ] Validator detects 95%+ of schema violations in test suite
- [ ] Validation completes in <10 seconds for 1,000 resources

**Phase 2 (Advanced Validation):**
- [ ] Cryptographic validator detects 100% of invalid signatures
- [ ] Referential integrity checker finds all broken links
- [ ] Authorization validator matches server behavior 100%

**Phase 3 (Synchronization):**
- [ ] Sync successfully replicates 10,000+ resource database
- [ ] Conflict resolution handles 95%+ of conflicts automatically
- [ ] Sync completes at >100 commits/second throughput

**Phase 4 (Testing):**
- [ ] 90%+ code coverage
- [ ] Zero critical bugs in production scenarios
- [ ] Documentation covers all features with examples

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-11-06 | Claude | Initial specification |

