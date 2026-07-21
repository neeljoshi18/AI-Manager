\# Technical Architecture Specification: Vertical 1  
\#\# Enterprise Telemetry Ingestion, Canonical Data Engine, & Real-Time ACL Mirroring

\---

\#\# 1\. Executive Summary & Core Invariants

\#\#\# 1.1 Scope & Purpose  
**\*\*Vertical 1\*\*** constitutes the immutable, high-throughput data foundation for the entire Autonomous AI Manager platform. Its primary objective is to ingest, validate, normalize, permission-tag, and store real-time telemetry events from external enterprise ecosystems (GitHub, GitLab, Jira, Linear, Slack, Microsoft Teams, Zendesk) without data loss, unauthorized exposure, or performance degradation under load.

\#\#\# 1.2 System Invariants (Non-Negotiable Guarantees)  
1\. **\*\*Zero Data Loss:\*\*** Every valid webhook payload must be durably written to an append-only log prior to issuing an HTTP \`200 OK\` acknowledgment to the upstream provider.  
2\. **\*\*Zero-Trust Permission Isolation:\*\*** No downstream component (including LLM agents in Vertical 3\) may access a telemetry event unless the request context satisfies the exact Access Control List (ACL) constraints mirrored from the source system at query execution time.  
3\. **\*\*Strict Sub-50ms Ingestion P99 Latency:\*\*** The ingestion edge must accept, authenticate, and enqueue incoming requests in under 50ms at P99 to prevent upstream provider timeout drops and retry cascades.  
4\. **\*\*Deterministic Type Safety:\*\*** Unstructured source payloads must be translated into compile-time typed Protobuf schemas at the ingestion edge; raw unstructured payloads are isolated to cold S3 storage.

\---

\#\# 2\. Technical Stack Selection & Trade-Off Matrix

To ensure multi-year operational durability and eliminate premature platform rewrites, technology choices are evaluated on throughput, memory safety, tail-latency stability, and operational complexity.

\+-----------------------------------------------------------------------------------+  
| INCOMING WEBHOOKS |  
\+-----------------------------------------------------------------------------------+  
|  
v  
\+-----------------------------------------------------------------------------------+  
| ENVOY EDGE GATEWAY (TLS / RATE-LIMIT) |  
\+-----------------------------------------------------------------------------------+  
|  
v  
\+-----------------------------------------------------------------------------------+  
| INGESTION SERVICE (RUST / AXUM) |  
| \[ HMAC Auth \] \---\> \[ Redis Sliding Window \] \---\> \[ Redis Bloom Deduplication \] |  
\+-----------------------------------------------------------------------------------+  
|  
v  
\+-----------------------------------------------------------------------------------+  
| NORMALIZATION & ACL ENFORCEMENT ENGINE |  
| \[ Protobuf Schema Validation \] \<---\> \[ CockroachDB ACL Bitmap Lookup Engine \] |  
\+-----------------------------------------------------------------------------------+  
|  
v  
\+-----------------------------------------------------------------------------------+  
| REDPANDA STREAMING CLUSTER (KAFKA API) |  
| \[ Topic: events.raw \] \<---\> \[ Tiered Storage (S3 / GCS) \] |  
\+-----------------------------------------------------------------------------------+  
|  
v  
\+-----------------------------------------------------------------------------------+  
| CLICKHOUSE OLAP ANALYTICS LAKE |  
| \[ Null Engine Consumer \] \---\> \[ Materialized Views \] |  
| \[ ReplacingMergeTree \] \<---\> \[ Query-Time ACL Filter \] |  
\+-----------------------------------------------------------------------------------+

\#\#\# 2.1 Technical Stack Comparison & Justification

| Architectural Layer | Evaluated Alternatives | Selected Architecture | Technical Rationale & Trade-Off Analysis |  
| :--- | :--- | :--- | :--- |  
| \*\*Ingestion Edge Runtime\*\* | Go (Gin), Node.js (Fastify), Java (Quarkus) | \*\*Rust (Axum \+ Tower Middleware)\*\* | \*\*Decision:\*\* Rust eliminates Garbage Collection (GC) pauses entirely, guaranteeing flat tail latencies ($P\_{99} \< 10\\text{ms}$) during massive webhook bursts. \<br\>\*\*Trade-Off:\*\* Higher initial development effort and steeper learning curve compared to Go/Node.js, offset by memory safety and zero cold-start latency. |  
| \*\*Streaming Message Bus\*\* | Apache Kafka (JVM), RabbitMQ, NATS JetStream | \*\*Redpanda Cluster (C++ Native)\*\* | \*\*Decision:\*\* Native Kafka API compatibility with up to 10x lower tail latency under high write concurrency. Built-in Tiered Storage natively offloads cold partitions directly to S3/GCS without needing external connectors (Debezium/Kafka Connect). \<br\>\*\*Trade-Off:\*\* Commercial enterprise features cost money, but the open-source core provides C++ native throughput and zero JVM garbage collection tuning overhead. |  
| \*\*Transactional State & ACL Store\*\* | PostgreSQL (RDS), MongoDB, DynamoDB | \*\*CockroachDB (Distributed SQL)\*\* | \*\*Decision:\*\* Provides native multi-region distributed ACID transactions with dynamic scale-out capability. Essential for strict serializability when tracking real-time user permissions and organization hierarchy. \<br\>\*\*Trade-Off:\*\* Higher query latency ($2\\text{--}5\\text{ms}$) than single-node Postgres, mitigated by a local Redis cluster serving as a read-through permission cache. |  
| \*\*Analytical Event Store\*\* | Elasticsearch, PostgreSQL (TimescaleDB), Snowflake | \*\*ClickHouse OLAP Engine\*\* | \*\*Decision:\*\* Columnar compression ratios (up to 5:1 on JSON metadata) combined with vectorized query execution allow sub-second aggregations across billions of event rows. Native \`ReplacingMergeTree\` handles event deduplication on disk. \<br\>\*\*Trade-Off:\*\* Poor fit for point updates; mitigated by treating all incoming events as immutable append-only logs. |  
| \*\*Schema Definition & Serialization\*\* | JSON Schema, Apache Avro, OpenAPI v3 | \*\*Protocol Buffers v3 (Protobuf) \+ Buf Schema Registry\*\* | \*\*Decision:\*\* Strictly typed binary format reduces payload sizes by \~60% compared to JSON. Ensures compile-time code generation for Rust, Go, and Python, preventing runtime schema mismatches. \<br\>\*\*Trade-Off:\*\* Requires a schema compilation step during build, managed via automated CI/CD pipelines. |

\---

\#\# 3\. Subsystem Architecture & Pipeline Specification

\#\#\# 3.1 Webhook Ingestion Engine & Deduplication  
Upstream providers (e.g., GitHub, Jira) issue HTTP POST requests upon event occurrences. The ingestion layer handles validation and deduplication deterministically.

1\. \*\*Authentication & Cryptographic Verification:\*\*  
   \* Each incoming request passes through an Axum \`Tower\` middleware.  
   \* HMAC-SHA256 signatures (\`X-Hub-Signature-256\` for GitHub, \`X-Hub-Signature\` for Jira) are verified using constant-time string comparison (\`subtle::ConstantTimeEq\`) to prevent timing attacks.  
2\. \*\*Rate Limiting & Denial-of-Service Defense:\*\*  
   \* Token bucket algorithm implemented in Redis (\`Redis-Cell\`).  
   \* Tiered limit: 10,000 requests/minute per tenant ID. Exceeded limits trigger an immediate \`429 Too Many Requests\` response with standard \`Retry-After\` headers.  
3\. \*\*Two-Tier Deduplication Strategy:\*\*  
   \* \*\*Tier 1 (Volatile Cache):\*\* Upon receipt, the system extracts the unique delivery identifier (e.g., \`X-GitHub-Delivery\`). A Redis command \`SET event\_id:val EX 86400 NX\` is executed. If Redis returns \`nil\`, the event is a duplicate and is instantly acknowledged with \`200 OK\` (dropped from processing pipeline).  
   \* \*\*Tier 2 (Analytical Engine):\*\* In ClickHouse, the \`ReplacingMergeTree\` engine uses \`event\_id\` as part of the primary sorting key to deduplicate events that bypass Tier 1 during Redis cluster failover events.

\#\#\# 3.2 Canonical Data Schema (Protobuf v3)

All raw events are mapped to a strongly-typed schema to ensure consistency across source systems:

\`\`\`protobuf  
syntax \= "proto3";

package enterprise.telemetry.v1;

import "google/protobuf/timestamp.proto";  
import "google/protobuf/struct.proto";

enum SourceProvider {  
  SOURCE\_PROVIDER\_UNSPECIFIED \= 0;  
  SOURCE\_PROVIDER\_GITHUB \= 1;  
  SOURCE\_PROVIDER\_GITLAB \= 2;  
  SOURCE\_PROVIDER\_JIRA \= 3;  
  SOURCE\_PROVIDER\_LINEAR \= 4;  
  SOURCE\_PROVIDER\_SLACK \= 5;  
  SOURCE\_PROVIDER\_TEAMS \= 6;  
}

enum EventCategory {  
  EVENT\_CATEGORY\_UNSPECIFIED \= 0;  
  EVENT\_CATEGORY\_CODE \= 1;       // Commits, PRs, Branch Creation  
  EVENT\_CATEGORY\_WORK\_ITEM \= 2;  // Issues, Tickets, Epics  
  EVENT\_CATEGORY\_COMMUNICATION \= 3; // Slack messages, PR reviews  
  EVENT\_CATEGORY\_IDENTITY \= 4;   // User additions, role changes  
}

message UserIdentity {  
  string global\_user\_id \= 1;      // Our internal cross-system mapping ID  
  string provider\_user\_id \= 2;    // Source system internal ID (e.g., "gh\_usr\_1234")  
  string email \= 3;  
  string display\_name \= 4;  
}

message ACLContext {  
  string tenant\_id \= 1;  
  repeated string allowed\_group\_ids \= 2; // Source-system Group/Team IDs with access  
  bool is\_private \= 3;                  // Private repo, restricted ticket flag  
  uint64 acl\_version \= 4;               // Incremental schema version for revocation updates  
}

message CanonicalEvent {  
  // Identity and Metadata  
  string event\_id \= 1;                  // Unique GUID for tracking  
  SourceProvider provider \= 2;  
  EventCategory category \= 3;  
  string event\_type \= 4;                // e.g., "pull\_request.opened", "issue.assigned"  
  google.protobuf.Timestamp timestamp \= 5;// Event occurrence timestamp from origin system  
  google.protobuf.Timestamp ingested\_at \= 6; // Platform ingestion timestamp

  // Contextual Entities  
  UserIdentity actor \= 7;  
  ACLContext acl\_context \= 8;

  // Normalized References  
  string resource\_id \= 9;               // e.g., "repo\_id/pr\_number" or "project\_key/issue\_id"  
  string parent\_resource\_id \= 10;        // e.g., parent Epic ID or repository ID

  // Structured Metadata & Raw Traceability  
  google.protobuf.Struct attributes \= 11;// Standardized key-value pairs  
  string raw\_payload\_s3\_uri \= 12;       // Pointer to raw encrypted JSON in S3  
}

### **3.3 Dynamic ACL Mirroring Engine**

To prevent data leaks across authorization boundaries, Vertical 1 maintains a dynamic authorization tree synchronized continuously with source access control updates.

#### **1\. Identity Mapping & Group Resolution**

* When a enterprise onboard occurs, a background job syncs workspace memberships from GitHub Orgs, Jira Enterprise Projects, and Slack User Groups.  
* Mapping relationships are stored in CockroachDB:

$$\\text{UserMap}: \\quad \\text{tenant\\\_id} \\parallel \\text{provider\\\_user\\\_id} \\longrightarrow \\text{global\\\_user\\\_id}$$  
$$\\text{GroupMap}: \\quad \\text{global\\\_user\\\_id} \\longrightarrow \\{ \\text{group\\\_id}\_1, \\text{group\\\_id}\_2, \\dots, \\text{group\\\_id}\_n \\}$$

#### **2\. Query-Time Bitwise Filtering Strategy**

* Access groups within a tenant are mapped to 64-bit/128-bit integer dynamic bitmaps or explicit string arrays in ClickHouse.  
* **Query Execution:** When an analytical read occurs (e.g., Vertical 2 fetching context), ClickHouse executes a mandatory SQL filter enforcing group membership:

SQL  
SELECT   
    event\_id, event\_type, attributes, actor\_global\_user\_id  
FROM enterprise\_telemetry.canonical\_events\_local  
WHERE tenant\_id \= 'ten\_4f8a91b'  
  AND (  
      is\_private \= false   
      OR hasAny(allowed\_group\_ids, \['grp\_eng\_core', 'grp\_sec\_lead'\])  
  )  
  AND timestamp \>= now() \- INTERVAL 7 DAY;

#### **3\. Real-Time Revocation Synchronization**

* **Push-Based Invalidation:** When a user is removed from a team in GitHub/Jira, an immediate low-latency ACLRevocationEvent is ingested.  
* CockroachDB updates the user’s GroupMap instantly, and issues an invalidation message to the Redis ACL cache via Pub/Sub.  
* **Effect:** Within $\<200\\text{ms}$ of permission removal, subsequent context queries by that user or on behalf of that user fail to retrieve restricted historical telemetry.

## **4\. Preemptive Architectural Challenges & Engineering Mitigations**

\+---------------------------------------------------------------------------------------------------+  
| PREEMPTIVE ENGINEERING CHALLENGE MATRIX                                                            |  
\+---------------------------------------------------------------------------------------------------+  
|                                                                                                   |  
|  1\. Out-of-Order Webhooks      \---\> Resolved by Event-Sourcing Sequence Engine                     |  
|  2\. Schema Evolution Drift     \---\> Resolved by Protobuf Compatibility & S3 Raw Vault             |  
|  3\. Rate Limit Exhaustion      \---\> Resolved by Dynamic Backoff & Partitioned Queue Priority      |  
|  4\. ACL Revocation Race Cond.  \---\> Resolved by Monotonic ACL Versions & Transactional Invalidation |  
|                                                                                                   |  
\+---------------------------------------------------------------------------------------------------+

### **Challenge 1: Out-of-Order Event Processing**

* **Scenario:** Under heavy network contention, an upstream provider sends a PullRequest:Closed event that arrives *before* the PullRequest:Created event.  
* **Mitigation:**  
  * Downstream analytical views rely on state reconstruction using ClickHouse's ReplacingMergeTree(event\_sequence\_number) or argMax(state, timestamp).  
  * The actual application state is derived from the event timestamp ($\\text{timestamp}\_{\\text{origin}}$) rather than insertion sequence ($\\text{timestamp}\_{\\text{ingest}}$).

### **Challenge 2: Upstream API Rate Limit Exhaustion During Historical Backfills**

* **Scenario:** Initial onboarding of a 1,000-developer org requires fetching 90 days of historical Jira tickets and GitHub PRs, exhausting third-party API rate limits.  
* **Mitigation:**  
  * Historical sync workloads are routed to a distinct Redpanda queue (events.backfill) isolated from real-time webhooks (events.realtime).  
  * Backfill workers implement an adaptive sliding-window rate limiter monitoring third-party response headers (X-RateLimit-Remaining). If remaining capacity drops below 15%, backfill workers pause automatically.

### **Challenge 3: Third-Party Schema Mutations (Breaking API Drift)**

* **Scenario:** GitHub introduces a breaking change by converting a previously string field into an object.  
* **Mitigation:**  
  * Protobuf parsing uses non-strict field mapping (ignore\_unknown\_fields \= true).  
  * If serialization fails, an Axum middleware intercepts the raw JSON payload, stores it safely in the S3 Dead-Letter Queue (s3://tenant-dlq/raw/YYYY/MM/DD/), and fires a PagerDuty alert to the infrastructure team. Webhook processing **does not halt**.

## **4.4 Analytical Storage Schema (ClickHouse DDL)**

To support sub-second query performance across millions of incoming enterprise events, ClickHouse uses the following optimized structure:  
SQL  
CREATE DATABASE IF NOT EXISTS enterprise\_telemetry;

\-- Ingestion Landing Table (ReplacingMergeTree handles deduplication)  
CREATE TABLE IF NOT EXISTS enterprise\_telemetry.canonical\_events\_local ON CLUSTER telemetry\_cluster  
(  
    event\_id String,  
    tenant\_id String,  
    provider Enum8('GITHUB' \= 1, 'GITLAB' \= 2, 'JIRA' \= 3, 'LINEAR' \= 4, 'SLACK' \= 5, 'TEAMS' \= 6),  
    category Enum8('CODE' \= 1, 'WORK\_ITEM' \= 2, 'COMMUNICATION' \= 3, 'IDENTITY' \= 4),  
    event\_type LowCardinality(String),  
    event\_timestamp DateTime64(3, 'UTC'),  
    ingested\_at DateTime64(3, 'UTC') DEFAULT now64(3),  
      
    \-- Actor Details  
    actor\_global\_user\_id String,  
    actor\_provider\_user\_id String,  
    actor\_email String,  
      
    \-- Access Control List Data  
    is\_private UInt8,  
    allowed\_group\_ids Array(String),  
    acl\_version UInt64,  
      
    \-- Metadata & Payloads  
    resource\_id String,  
    parent\_resource\_id String,  
    attributes\_json String, \-- Compressed JSON string for low-frequency queries  
    raw\_payload\_s3\_uri String  
)  
ENGINE \= ReplacingMergeTree(acl\_version)  
PARTITION BY toYYYYMM(event\_timestamp)  
PRIMARY KEY (tenant\_id, category, provider)  
ORDER BY (tenant\_id, category, provider, event\_type, resource\_id, event\_id)  
SETTINGS index\_granularity \= 8192, ttl\_only\_drop\_parts \= 1;

\-- Distributed Table Wrapper for Multi-Node Scaling  
CREATE TABLE IF NOT EXISTS enterprise\_telemetry.canonical\_events ON CLUSTER telemetry\_cluster  
AS enterprise\_telemetry.canonical\_events\_local  
ENGINE \= Distributed(telemetry\_cluster, enterprise\_telemetry, canonical\_events\_local, rand());

## **5\. Exhaustive Test Suite & Verification Matrix**

Vertical 1 must pass all strict validation scenarios prior to unblocking Vertical 2\.  
\+----------------------------------------------------------------------------------------------------+  
|                                    VERIFICATION BATTERY DASHBOARD                                  |  
\+----------------------------------------------------------------------------------------------------+  
|  \[TEST SUITE 1: LOAD\]      \---\> 25,000 req/sec Spike    | Target: P99 \< 50ms   | Drop Rate: 0%      |  
|  \[TEST SUITE 2: IDEMPOT\]   \---\> 10k Replay Attack       | Target: 1 Record     | Duplicate: 0      |  
|  \[TEST SUITE 3: ACL\]       \---\> Perm Revocation Sync    | Target: Latency \<200ms| Leakage: 0%       |  
|  \[TEST SUITE 4: CHAOS\]     \---\> Redpanda Node Kill      | Target: Zero Loss    | Recovery \< 3s     |  
\+----------------------------------------------------------------------------------------------------+

### **Test Battery Execution Matrix**

| Test ID | Category | Scenario / Injected Failure | Execution Script / Tooling | Target Pass Metric |
| :---- | :---- | :---- | :---- | :---- |
| **TC-01** | **Load & Scale** | Continuous load of 5,000 req/sec with sudden 30-second spikes to 25,000 req/sec simulating enterprise automated CI run bursts. | Distributed K6 test runner deployed on GKE cluster issuing signed payloads. | Ingestion edge $P\_{99} \< 50\\text{ms}$; 0 dropped requests; 100% enqueued to Redpanda. |
| **TC-02** | **Idempotency** | Replay attack: Submit 10,000 identical webhooks (same delivery ID) across 50 concurrent HTTP threads within 100ms. | Custom Rust concurrent test client using tokio::spawn. | Exactly **1** unique record written to ClickHouse; 9,999 requests return 200 OK (deduplicated at Redis layer). |
| **TC-03** | **ACL Leakage** | Issue UserRemovedFromGroup API event. Within 10ms, execute 1,000 context queries for that user against ClickHouse. | Automated Go test harness calling internal authorization endpoint. | **0 queries leak private data**. Permitted access drops to zero within $\<200\\text{ms}$ of revocation event ingestion. |
| **TC-04** | **Schema Mutations** | Send valid JSON containing extra fields, missing optional fields, and type-mutated optional fields. | Python test script injecting randomized JSON mutations via hypothesis library. | Zero application crashes; malformed optional fields safely default; severe type violations routed to S3 DLQ. |
| **TC-05** | **Chaos & Fault** | Forcefully terminate 1 of 3 active Redpanda brokers (kill \-9) during sustained 10,000 req/sec ingestion. | Chaos Mesh executing pod destruction scenarios on Kubernetes cluster. | Zero data loss; Raft leader re-election completes in $\<3\\text{s}$; Ingestion edge buffers payloads in memory without dropping. |
| **TC-06** | **Out-of-Order** | Inject PR\_CLOSED event with origin timestamp $T\_2$, followed 5 seconds later by PR\_OPENED event with origin timestamp $T\_1$ ($T\_1 \< T\_2$). | Custom synthetic event producer. | Analytical query calculating PR state returns state CLOSED correctly using state timestamp sorting. |

## **6\. Definition of Done (Exit Criteria for Vertical 1\)**

Vertical 1 is formally certified as complete and unblocks the execution of Vertical 2 when the following measurable criteria are met:

* \[ \] **Infrastructure Provisioning:** Envoy, Rust Ingestion Service, Redpanda Cluster, CockroachDB, and ClickHouse deployed via Terraform to staging environments.  
* \[ \] **Protobuf Contract Lock:** Protobuf schemas compiled, validated with buf breaking, and published to the internal artifact registry.  
* \[ \] **Zero-Data-Loss Verification:** Verification test suite TC-01 through TC-06 executed with a **100% pass rate**.  
* \[ \] **Observability Baseline:** Grafana dashboard operational tracking:  
  * Webhook Ingestion Throughput (req/sec)  
  * Ingestion Latency Percentiles ($P\_{50}, P\_{95}, P\_{99}$)  
  * Redpanda Consumer Lag by Partition  
  * ClickHouse Ingestion Buffer Rates & Disk Usage  
  * Redis Deduplication Hit/Miss Ratios  
* \[ \] **Security Review Signed Off:** HMAC validation and ACL filter enforcement verified by security audit.

