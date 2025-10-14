# Atomic Server Architecture Documentation

## System Overview

Atomic Server is deployed as a containerized web application with a reverse proxy frontend, providing secure access to a graph-based content management system.

## Components

### 1. Atomic Server Application

**Technology Stack:**
- **Language**: Rust
- **Framework**: Actix-web
- **Database**: SQLite with FTS5 full-text search
- **Protocol**: HTTP/2, WebSockets
- **Data Format**: Atomic Data (JSON-LD based)

**Key Features:**
- Graph database with subject-predicate-object triples
- Real-time collaboration via WebSockets
- Full-text search with fuzzy matching
- User authentication and authorization
- RESTful API with JSON responses
- Schema validation and ontologies

### 2. Docker Container

**Configuration:**
- **Base Image**: Alpine Linux 3.19
- **Runtime**: Single binary deployment
- **Security**: Non-root user (atomic:1000)
- **Health Checks**: HTTP endpoint monitoring
- **Resource Limits**: Memory and CPU constraints

**Storage Architecture:**
```
Container Volume Mapping:
/atomic/data → ./data (SQLite database)
/atomic/config → ./config (Server configuration)
/atomic/logs → ./logs (Application logs)
```

### 3. Caddy Reverse Proxy

**Features:**
- **Automatic HTTPS**: Let's Encrypt certificates
- **HTTP/2 Support**: Modern web protocols
- **Load Balancing**: Ready for horizontal scaling
- **Security Headers**: HSTS, CSP, CORS protection
- **Request Routing**: Domain-based routing rules

**Configuration:**
```
Internet Traffic → Caddy (ports 80/443) → Container (port 8081)
                 ↓
           SSL Termination
                 ↓
           HTTP/2 with Compression
                 ↓
           Reverse Proxy to Atomic Server
```

## Data Flow

### Request Processing

1. **HTTPS Request** → Caddy receives on port 443
2. **SSL Termination** → Certificate validation and decryption
3. **Header Processing** → Security headers and forwarding
4. **Reverse Proxy** → Route to Atomic Server container
5. **Application Processing** → Atomic Server handles request
6. **Database Operations** → SQLite queries and updates
7. **Response Generation** → JSON/HTML response
8. **Return Path** → Response through Caddy to client

### Database Architecture

**SQLite Schema:**
- **Triples Table**: Subject-Predicate-Object storage
- **Indexing**: Optimized for graph traversals
- **FTS5**: Full-text search virtual tables
- **WAL Mode**: Write-Ahead Logging for performance
- **Foreign Keys**: Referential integrity

**Data Organization:**
```
Resources (Nodes)
    ↓
Properties (Edges)
    ↓
Values (Leaf nodes)
```

## Security Architecture

### Network Security

**Layers:**
1. **Transport Layer**: TLS 1.3 with Let's Encrypt certificates
2. **Application Layer**: HTTP security headers
3. **Container Isolation**: Docker sandbox with user namespace
4. **Host Security**: Firewall and system hardening

**Security Headers:**
- **HSTS**: HTTPS enforcement
- **CSP**: Content Security Policy
- **X-Frame-Options**: Clickjacking protection
- **X-Content-Type-Options**: MIME type sniffing protection

### Application Security

**Authentication:**
- **Agent-based**: Cryptographic identity system
- **JWT Tokens**: Secure session management
- **Setup Invites**: One-time initialization tokens
- **Role-based Access Control**: Permission management

**Data Protection:**
- **Encryption**: Data at rest and in transit
- **Access Control**: Resource-level permissions
- **Audit Logging**: Request tracking and monitoring
- **Backup Encryption**: Encrypted backup storage

## Performance Architecture

### Caching Strategy

**Multi-layer Caching:**
1. **HTTP Caching**: Browser cache headers
2. **Application Cache**: In-memory resource caching
3. **Database Cache**: SQLite query optimization
4. **CDN Integration**: CloudFlare edge caching

### Database Optimization

**Performance Features:**
- **WAL Mode**: Concurrent reads and writes
- **Connection Pooling**: Efficient database connections
- **Query Optimization**: Index-aware query planning
- **Batch Operations**: Bulk transaction processing

### Resource Management

**Container Resources:**
- **Memory**: 14MB base + database growth
- **CPU**: Event-driven I/O with Actix-web
- **Storage**: Persistent volume with growth monitoring
- **Network**: HTTP/2 with compression

## Deployment Architecture

### Infrastructure Components

**Server Configuration:**
- **Host**: Hetzner cloud server
- **OS**: Ubuntu 20.04 LTS
- **IPv4**: 78.46.87.136
- **IPv6**: 2a01:4f8:120:2094::2
- **Domain**: evolve.privacy1st.org

**Container Orchestration:**
- **Runtime**: Docker Engine
- **Networking**: Bridge network with port mapping
- **Storage**: Local persistent volumes
- **Monitoring**: Health checks and logging

### Service Dependencies

**Required Services:**
1. **Docker Engine**: Container runtime
2. **Caddy Server**: Reverse proxy and TLS termination
3. **DNS Resolution**: Domain name resolution
4. **Time Synchronization**: NTP for accurate timestamps

**Optional Services:**
- **Monitoring**: Prometheus/Grafana integration
- **Logging**: Centralized log aggregation
- **Backup**: Automated backup solutions
- **CI/CD**: Deployment pipelines

## Monitoring and Observability

### Health Monitoring

**Metrics Collected:**
- **Application Health**: HTTP endpoint responses
- **Container Status**: Docker health checks
- **Resource Usage**: CPU, memory, disk, network
- **Database Performance**: Query times and connection counts
- **Error Rates**: HTTP error response tracking

### Logging Architecture

**Log Sources:**
1. **Application Logs**: Atomic Server operational logs
2. **Access Logs**: HTTP request/response logging
3. **System Logs**: Container and host system logs
4. **Security Logs**: Authentication and authorization events

### Alerting

**Alert Conditions:**
- **Service Unavailability**: Health check failures
- **High Error Rates**: HTTP 5xx responses
- **Resource Exhaustion**: Memory or disk space limits
- **Security Events**: Failed authentication attempts

## Scaling Architecture

### Vertical Scaling

**Resource Scaling:**
- **Memory**: Increase container memory limits
- **CPU**: Add CPU cores for request processing
- **Storage**: Scale persistent volume size
- **Network**: Increase bandwidth allocation

### Horizontal Scaling

**Multi-container Deployment:**
```
Load Balancer (Caddy)
    ↓
Multiple Atomic Server Containers
    ↓
Shared Database (PostgreSQL/MySQL)
    ↓
Distributed Cache (Redis)
```

**Scaling Strategies:**
- **Session Affinity**: Sticky sessions for user state
- **Database Sharding**: Partition data across instances
- **Cache Distribution**: Shared cache layer
- **Global Load Balancing**: Geographic distribution

## Backup and Recovery

### Backup Strategy

**Data Backup Components:**
1. **Database Backup**: SQLite database snapshots
2. **Configuration Backup**: Server configuration files
3. **Log Backup**: Application and system logs
4. **Container Backup**: Docker images and volumes

**Backup Schedule:**
- **Database**: Hourly snapshots, daily full backups
- **Configuration**: On-change backup
- **Logs**: Weekly rotation, monthly archival
- **System**: Weekly full system backup

### Recovery Procedures

**Disaster Recovery:**
1. **System Restore**: From system backups
2. **Container Recovery**: Docker volume restoration
3. **Database Recovery**: SQLite database restoration
4. **Configuration Recovery**: Server config restoration

**Recovery Time Objectives:**
- **RTO**: 4 hours (maximum acceptable downtime)
- **RPO**: 1 hour (maximum data loss)
- **MTTR**: 2 hours (mean time to recovery)

## Development Architecture

### Local Development

**Development Environment:**
- **Docker Compose**: Multi-container local setup
- **Hot Reloading**: Development server with auto-reload
- **Database Seeding**: Test data and fixtures
- **Debugging**: Integrated debugging tools

### CI/CD Pipeline

**Build Process:**
1. **Source Code**: Git repository with branches
2. **Build Stage**: Docker image compilation
3. **Test Stage**: Automated testing and validation
4. **Deploy Stage**: Production deployment pipeline
5. **Monitor Stage**: Post-deployment verification

**Quality Gates:**
- **Code Quality**: Rust linting and formatting
- **Security Scanning**: Vulnerability assessment
- **Performance Testing**: Load and stress testing
- **Integration Testing**: End-to-end validation

This architecture documentation provides a comprehensive overview of the Atomic Server deployment, covering all aspects from system design to operational procedures.