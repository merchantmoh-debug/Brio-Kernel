# Distributed Mesh Deployment Guide

This guide covers deploying Brio-Kernel across multiple nodes to form a service mesh for horizontal scaling and high availability.

---

## When to Use Distributed Mode

**Distributed mode is recommended when:**

- **High availability** - Need redundancy if individual nodes fail
- **Horizontal scaling** - Single node cannot handle workload
- **Geographic distribution** - Agents need to run close to data sources
- **Resource specialization** - Dedicated nodes for specific workloads (GPU inference, I/O intensive tasks)
- **Multi-team isolation** - Different teams on separate infrastructure

**Single-node mode is sufficient when:**

- Development or testing environments
- Workloads under 100 concurrent agents
- Simplicity is preferred over scale
- Network latency between nodes would be prohibitive (>50ms)

## Benefits and Trade-offs

| Benefits | Trade-offs |
|----------|------------|
| Horizontal scaling beyond single machine limits | Network latency for inter-node calls |
| Fault tolerance through redundancy | Increased operational complexity |
| Geographic workload distribution | Shared database becomes single point of failure |
| Load balancing across nodes | Requires stable network connectivity |
| Rolling updates without downtime | TLS certificate management overhead |

## Architecture

### Node Roles

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Brio Service Mesh                                │
│                                                                       │
│  ┌──────────────┐         ┌──────────────┐         ┌──────────────┐ │
│  │ Coordinator  │◄───────►│ Coordinator  │◄───────►│   Worker     │ │
│  │   Node 1     │   gRPC  │   Node 2     │   gRPC  │   Node 3     │ │
│  │  (Primary)   │         │  (Standby)   │         │              │ │
│  └──────┬───────┘         └──────┬───────┘         └──────┬───────┘ │
│         │                        │                        │         │
│         │    ┌───────────────────┴───────────────────┐    │         │
│         └────►                                     ◄────┘         │
│              │         Shared Database              │              │
│              │     (PostgreSQL/SQLite Cluster)      │              │
│              └──────────────────────────────────────┘              │
└─────────────────────────────────────────────────────────────────────┘
```

**Coordinator Node:**
- Accepts client connections (HTTP/WebSocket)
- Manages task distribution
- Maintains cluster state
- Can run agents locally
- Acts as entry point for external clients

**Worker Node:**
- Runs agents only
- No direct client connections
- Receives tasks via mesh calls
- Reports status to coordinators
- Stateless (can be added/removed dynamically)

**Multi-Coordinator (HA Mode):**
- Multiple coordinator nodes for redundancy
- Client connections distributed via load balancer
- Only one active coordinator processes tasks
- Standby coordinators ready for failover

### Service Mesh Topology

The mesh uses a fully-connected topology where every node maintains gRPC connections to all other nodes:

```mermaid
graph TB
    subgraph "Cluster"
        C1[Coordinator-1<br/>0.0.0.0:50051]
        C2[Coordinator-2<br/>0.0.0.0:50051]
        W1[Worker-1<br/>0.0.0.0:50051]
        W2[Worker-2<br/>0.0.0.0:50051]
        W3[Worker-3<br/>0.0.0.0:50051]
    end
    
    C1 <-->|gRPC| C2
    C1 <-->|gRPC| W1
    C1 <-->|gRPC| W2
    C1 <-->|gRPC| W3
    C2 <-->|gRPC| W1
    C2 <-->|gRPC| W2
    C2 <-->|gRPC| W3
```

### Inter-Node Communication

**gRPC Protocol:**
- Port: 50051 (default, configurable)
- Protocol: HTTP/2 with TLS/mTLS
- Messages: Protobuf
- Keepalive: 30-second heartbeats

**Message Types:**
```protobuf
// Mesh call routing
rpc Call(MeshRequest) returns (MeshResponse);

// Health checking
rpc Heartbeat(HeartbeatRequest) returns (HeartbeatResponse);
```

### State Synchronization

**Shared State (via database):**
- Sessions and conversation history
- Agent registrations and capabilities
- Task queue and status
- Configuration

**In-Memory State (per node):**
- Active agent instances
- WebSocket connections
- Local cache

**Important:** All nodes must connect to the same database. For production, use PostgreSQL with streaming replication rather than SQLite.

## Configuration

### Environment Variables

```bash
# Required for all nodes
export BRIO_NODE_ID="node-1"              # Unique identifier
export BRIO_MESH_ENABLED="true"           # Enable mesh mode
export BRIO_MESH_LISTEN="0.0.0.0:50051"   # gRPC listen address

# Optional discovery
export BRIO_MESH_DISCOVERY="static"       # static, dns, kubernetes
export BRIO_MESH_PEERS="node-2:50051,node-3:50051"
```

### TOML Configuration

**Coordinator Node:**

```toml
[server]
host = "0.0.0.0"
port = 8080
websocket_enabled = true

[mesh]
enabled = true
node_id = "coordinator-1"
listen_addr = "0.0.0.0:50051"
role = "coordinator"

# Peer discovery
[mesh.discovery]
method = "static"
refresh_interval = 60

# List of known peers
[[mesh.peers]]
id = "coordinator-2"
address = "10.0.1.2:50051"
role = "coordinator"

[[mesh.peers]]
id = "worker-1"
address = "10.0.1.10:50051"
role = "worker"

[[mesh.peers]]
id = "worker-2"
address = "10.0.1.11:50051"
role = "worker"

# TLS Configuration
[mesh.tls]
enabled = true
cert_file = "/etc/brio/certs/server.crt"
key_file = "/etc/brio/certs/server.key"
ca_file = "/etc/brio/certs/ca.crt"
verify_client = true  # Enable mTLS

[database]
url = "postgresql://brio:password@db-cluster:5432/brio"
max_connections = 50
```

**Worker Node:**

```toml
[server]
enabled = false  # Workers don't accept client connections

[mesh]
enabled = true
node_id = "worker-1"
listen_addr = "0.0.0.0:50051"
role = "worker"
max_agents = 50  # Limit concurrent agents

[mesh.discovery]
method = "static"

[[mesh.peers]]
id = "coordinator-1"
address = "10.0.1.1:50051"
role = "coordinator"

[[mesh.peers]]
id = "coordinator-2"
address = "10.0.1.2:50051"
role = "coordinator"

[mesh.tls]
enabled = true
cert_file = "/etc/brio/certs/worker-1.crt"
key_file = "/etc/brio/certs/worker-1.key"
ca_file = "/etc/brio/certs/ca.crt"

[database]
url = "postgresql://brio:password@db-cluster:5432/brio"
```

### Network Requirements

| Requirement | Specification |
|-------------|---------------|
| TCP Ports | 50051 (gRPC), 8080 (HTTP/WebSocket for coordinators) |
| Bandwidth | 10 Mbps minimum between nodes |
| Latency | <10ms preferred, <50ms acceptable |
| Connectivity | Full mesh - all nodes must reach all others |
| Firewall | Allow bidirectional TCP on mesh port |

## Deployment Patterns

### Pattern 1: Single Coordinator + Workers

**Best for:** Small to medium deployments, simpler operations

```
                   ┌─────────────┐
                   │   Load      │
                   │  Balancer   │
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │ Coordinator │
                   │   Node 1    │
                   └──────┬──────┘
                          │ gRPC
           ┌──────────────┼──────────────┐
           │              │              │
    ┌──────▼──────┐ ┌─────▼──────┐ ┌─────▼──────┐
    │  Worker 1   │ │  Worker 2  │ │  Worker 3  │
    └─────────────┘ └────────────┘ └────────────┘
```

**Pros:** Simplest setup, easy to reason about
**Cons:** Single point of failure for client connections

### Pattern 2: Multi-Coordinator (HA)

**Best for:** Production workloads requiring high availability

```
                   ┌─────────────┐
                   │   Load      │
                   │  Balancer   │
                   │  (Active)   │
                   └──────┬──────┘
                          │
           ┌──────────────┼──────────────┐
           │              │              │
    ┌──────▼──────┐ ┌─────▼──────┐      │
    │Coordinator 1│ │Coordinator 2│      │
    │  (Primary)  │ │  (Standby)  │      │
    └──────┬──────┘ └─────┬──────┘      │
           │              │              │
           └──────────────┼──────────────┘
                          │
           ┌──────────────┼──────────────┐
           │              │              │
    ┌──────▼──────┐ ┌─────▼──────┐ ┌─────▼──────┐
    │  Worker 1   │ │  Worker 2  │ │  Worker 3  │
    └─────────────┘ └────────────┘ └────────────┘
```

**Pros:** No single point of failure, rolling updates
**Cons:** More complex, requires load balancer or DNS

### Pattern 3: Cloud Deployment

**AWS Example:**

```toml
# coordinator.toml
[mesh]
node_id = "coordinator-1"
listen_addr = "0.0.0.0:50051"

[mesh.discovery]
method = "dns"
dns_name = "brio-mesh.internal"
hosted_zone = "Z123456789"
refresh_interval = 30

[database]
url = "postgresql://brio:${DB_PASSWORD}@brio-db.cluster-xxx.us-east-1.rds.amazonaws.com:5432/brio"
```

**Kubernetes Example:**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: brio-coordinator
spec:
  serviceName: brio-mesh
  replicas: 2
  template:
    spec:
      containers:
      - name: brio
        image: brio-kernel:latest
        env:
        - name: BRIO_NODE_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: BRIO_MESH_DISCOVERY
          value: "kubernetes"
        - name: BRIO_MESH_K8S_NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        - name: BRIO_MESH_K8S_LABEL_SELECTOR
          value: "app=brio-kernel"
        ports:
        - containerPort: 50051
          name: mesh
        - containerPort: 8080
          name: http
```

### Docker Compose Example

```yaml
version: '3.8'

services:
  coordinator:
    image: brio-kernel:latest
    environment:
      - BRIO_NODE_ID=coordinator-1
      - BRIO_MESH_ENABLED=true
      - BRIO_MESH_LISTEN=0.0.0.0:50051
      - BRIO_DATABASE_URL=postgresql://brio:password@postgres:5432/brio
    ports:
      - "8080:8080"
      - "50051:50051"
    volumes:
      - ./certs:/etc/brio/certs:ro
    depends_on:
      - postgres

  worker-1:
    image: brio-kernel:latest
    environment:
      - BRIO_NODE_ID=worker-1
      - BRIO_MESH_ENABLED=true
      - BRIO_MESH_LISTEN=0.0.0.0:50051
      - BRIO_MESH_PEERS=coordinator-1:50051
      - BRIO_DATABASE_URL=postgresql://brio:password@postgres:5432/brio
    volumes:
      - ./certs:/etc/brio/certs:ro
    depends_on:
      - coordinator

  postgres:
    image: postgres:15
    environment:
      - POSTGRES_USER=brio
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=brio
    volumes:
      - postgres_data:/var/lib/postgresql/data

volumes:
  postgres_data:
```

## Service Routing

### Local vs Remote Routing

The mesh automatically routes calls based on target location:

```rust
// Target format determines routing:
// "agent-name"           -> Local routing
// "node-id/agent-name"   -> Remote routing to specific node
// "worker-1/coder"       -> Routes to worker-1 node
```

**Routing Decision Flow:**

1. Parse target for node prefix (contains `/`)
2. If no prefix, route locally
3. If prefix matches local node ID, route locally
4. If prefix matches remote node, send gRPC call
5. If node unknown, return error

### Agent Distribution

Agents can be deployed to specific nodes:

```toml
# Deploy coder agent to GPU workers
[agents.coder]
node_affinity = ["worker-gpu-1", "worker-gpu-2"]
model = "gpt-4"

# Deploy file operations to I/O optimized nodes
[agents.file-ops]
node_affinity = ["worker-storage-1"]
```

**Without affinity:** Coordinator distributes tasks round-robin across all capable nodes.

### Task Affinity

Keep related tasks on the same node to minimize network calls:

```toml
[mesh]
# Enable session affinity
session_affinity = true
affinity_ttl = 3600  # seconds
```

Sessions are hashed to a consistent node. If that node fails, tasks are redistributed.

### Load Balancing

The mesh supports multiple load balancing strategies:

```toml
[mesh.load_balancing]
strategy = "least-connections"  # round-robin, least-connections, weighted

# Weighted distribution
[[mesh.load_balancing.weights]]
node_id = "worker-1"
weight = 2

[[mesh.load_balancing.weights]]
node_id = "worker-2"
weight = 1
```

## Security Considerations

### Inter-Node Authentication

**mTLS (Recommended for Production):**

```bash
# Generate CA
openssl req -x509 -newkey rsa:4096 -keyout ca.key -out ca.crt -days 365 -nodes

# Generate node certificates
openssl req -newkey rsa:4096 -keyout node-1.key -out node-1.csr -nodes
openssl x509 -req -in node-1.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out node-1.crt -days 365
```

**Configuration:**

```toml
[mesh.tls]
enabled = true
cert_file = "/etc/brio/certs/node.crt"
key_file = "/etc/brio/certs/node.key"
ca_file = "/etc/brio/certs/ca.crt"
verify_client = true  # Require client certificates (mTLS)
```

### Network Segmentation

**Recommended Network Layout:**

```
┌─────────────────────────────────────────────────────┐
│                    Public Network                    │
│  (Load Balancer: 443 -> Coordinator:8080)           │
└──────────────────┬──────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────┐
│                  Mesh Network                        │
│  (10.0.0.0/24 - Internal gRPC communication)        │
│                                                      │
│  ┌──────────────┐      ┌──────────────┐             │
│  │ Coordinator  │◄────►│   Worker     │             │
│  └──────────────┘      └──────────────┘             │
└─────────────────────────────────────────────────────┘
```

**Firewall Rules:**

```bash
# Allow mesh communication between nodes
iptables -A INPUT -p tcp -s 10.0.0.0/24 --dport 50051 -j ACCEPT
iptables -A INPUT -p tcp --dport 50051 -j DROP

# Allow coordinator client connections
iptables -A INPUT -p tcp --dport 8080 -j ACCEPT
```

### Access Control Between Nodes

Node roles determine access permissions:

```toml
[mesh.access_control]
# Workers can only receive tasks
coordinator_only_endpoints = ["/api/admin", "/api/config"]

# IP allowlist for mesh connections
allowed_cidrs = ["10.0.0.0/24", "10.0.1.0/24"]
```

## Monitoring and Observability

### Distributed Tracing

Enable OpenTelemetry tracing across nodes:

```toml
[telemetry]
enabled = true
exporter = "otlp"
endpoint = "http://jaeger:4317"
service_name = "brio-mesh"
propagation = "trace-context"  # W3C standard

[telemetry.tags]
node_id = "coordinator-1"
region = "us-east-1"
```

Traces automatically propagate across mesh calls, allowing you to track a single request across multiple nodes.

### Node Health Checks

**Built-in Health Check Endpoint:**

```bash
# Check node health
curl http://coordinator-1:8080/health

# Response:
{
  "status": "healthy",
  "node_id": "coordinator-1",
  "version": "0.1.0",
  "mesh": {
    "connected_peers": 3,
    "last_heartbeat": "2024-01-15T10:30:00Z"
  },
  "agents": {
    "active": 12,
    "queued": 3
  }
}
```

**Mesh Heartbeat Status:**

```bash
# Check mesh connectivity
brio-kernel mesh status

# Output:
Node ID        Address           Status    Last Seen    Latency
worker-1       10.0.1.10:50051   healthy   2s ago       5ms
worker-2       10.0.1.11:50051   healthy   3s ago       8ms
worker-3       10.0.1.12:50051   degraded  15s ago      45ms
```

### Metrics Aggregation

```toml
[metrics]
enabled = true
endpoint = "0.0.0.0:9090"
path = "/metrics"

# Mesh-specific metrics
[metrics.mesh]
collect_peer_latency = true
collect_routing_stats = true
```

**Key Metrics:**

| Metric | Description |
|--------|-------------|
| `brio_mesh_connected_peers` | Number of connected peers |
| `brio_mesh_rpc_duration_seconds` | gRPC call latency |
| `brio_mesh_rpc_errors_total` | Failed mesh calls |
| `brio_mesh_messages_routed_total` | Total messages routed |
| `brio_mesh_bytes_transferred_total` | Network throughput |

### Failure Detection

The mesh automatically detects and handles node failures:

```toml
[mesh.failure_detection]
heartbeat_interval = 30        # seconds
heartbeat_timeout = 10         # seconds
suspicion_threshold = 3        # missed heartbeats before marking failed
auto_remove_failed = true      # Remove failed nodes from pool
remove_delay = 300            # seconds before removal
```

**Failure Handling:**

1. Node misses 3 consecutive heartbeats
2. Marked as "suspected" in cluster state
3. Tasks redistributed to healthy nodes
4. After 5 minutes, node removed from peer list
5. If node recovers, it rejoins automatically

## Operational Procedures

### Adding Nodes

1. **Prepare the new node:**
   ```bash
   # Install Brio-Kernel
   curl -sSL https://install.brio.sh | sh
   
   # Copy TLS certificates
   scp ca.crt node.key node.crt new-node:/etc/brio/certs/
   ```

2. **Create configuration:**
   ```toml
   # /etc/brio/brio.toml
   [mesh]
   node_id = "worker-4"
   listen_addr = "0.0.0.0:50051"
   
   [[mesh.peers]]
   id = "coordinator-1"
   address = "10.0.1.1:50051"
   ```

3. **Start the node:**
   ```bash
   systemctl enable brio-kernel
   systemctl start brio-kernel
   ```

4. **Verify connection:**
   ```bash
   brio-kernel mesh status
   # Should show new worker-4 as connected
   ```

### Removing Nodes

**Graceful Removal:**

```bash
# Drain the node (stop accepting new tasks)
brio-kernel mesh drain worker-3

# Wait for active tasks to complete
brio-kernel mesh status worker-3
# Verify: active_agents = 0

# Remove from cluster
brio-kernel mesh leave

# Stop service
systemctl stop brio-kernel
```

**Force Removal (if node is unresponsive):**

```bash
# From any coordinator
brio-kernel mesh remove worker-3 --force
```

### Rolling Updates

Update nodes without cluster downtime:

```bash
#!/bin/bash
# rolling-update.sh

NODES=("worker-1" "worker-2" "worker-3" "coordinator-2" "coordinator-1")

for NODE in "${NODES[@]}"; do
  echo "Updating $NODE..."
  
  # Drain
  ssh $NODE "brio-kernel mesh drain"
  sleep 30
  
  # Stop service
  ssh $NODE "systemctl stop brio-kernel"
  
  # Update binary
  scp brio-kernel $NODE:/usr/local/bin/
  
  # Start service
  ssh $NODE "systemctl start brio-kernel"
  
  # Verify
  sleep 10
  ssh $NODE "brio-kernel mesh status"
  
  echo "$NODE updated successfully"
done
```

**Note:** Always update workers first, then standby coordinators, then primary coordinator last.

### Backup and Recovery

**Database Backup:**

```bash
# PostgreSQL backup
pg_dump -h db-cluster -U brio brio > brio-backup-$(date +%Y%m%d).sql

# Automated daily backup (cron)
0 2 * * * /usr/local/bin/backup-brio.sh
```

**Configuration Backup:**

```bash
# Backup all node configurations
tar czf brio-configs-$(date +%Y%m%d).tar.gz /etc/brio/

# Include TLS certificates
tar czf brio-certs-$(date +%Y%m%d).tar.gz /etc/brio/certs/
```

**Disaster Recovery:**

1. Restore database from backup
2. Deploy new nodes with backed-up configurations
3. Regenerate TLS certificates if needed
4. Verify mesh connectivity
5. Resume operations

## Example: Two-Node Setup

This example demonstrates a minimal distributed deployment with one coordinator and one worker.

### Node 1: Coordinator

**Configuration (`/etc/brio/coordinator.toml`):**

```toml
[server]
host = "0.0.0.0"
port = 8080
websocket_enabled = true

[mesh]
enabled = true
node_id = "coordinator-1"
listen_addr = "0.0.0.0:50051"
role = "coordinator"

[[mesh.peers]]
id = "worker-1"
address = "192.168.1.11:50051"
role = "worker"

[database]
url = "postgresql://brio:password@192.168.1.10:5432/brio"
max_connections = 20

[logging]
level = "info"
format = "json"
```

**Start command:**

```bash
export BRIO_CONFIG=/etc/brio/coordinator.toml
brio-kernel serve
```

### Node 2: Worker

**Configuration (`/etc/brio/worker.toml`):**

```toml
[server]
enabled = false

[mesh]
enabled = true
node_id = "worker-1"
listen_addr = "0.0.0.0:50051"
role = "worker"
max_agents = 20

[[mesh.peers]]
id = "coordinator-1"
address = "192.168.1.10:50051"
role = "coordinator"

[database]
url = "postgresql://brio:password@192.168.1.10:5432/brio"
max_connections = 10

[logging]
level = "info"
format = "json"
```

**Start command:**

```bash
export BRIO_CONFIG=/etc/brio/worker.toml
brio-kernel serve
```

### Verification Steps

1. **Check coordinator is running:**
   ```bash
   curl http://192.168.1.10:8080/health
   ```

2. **Verify mesh connectivity:**
   ```bash
   # On coordinator
   brio-kernel mesh status
   
   # Expected output:
   # Node ID    Address            Status
   # worker-1   192.168.1.11:50051 connected
   ```

3. **Test agent distribution:**
   ```bash
   # Create a task
   curl -X POST http://192.168.1.10:8080/api/tasks \
     -H "Content-Type: application/json" \
     -d '{"agent": "coder", "prompt": "Hello from distributed mesh"}'
   
   # Check it executed on worker
   brio-kernel mesh stats worker-1
   ```

### Troubleshooting

**Issue: Nodes cannot connect**

```bash
# Check firewall
telnet 192.168.1.11 50051

# Verify certificates
openssl x509 -in /etc/brio/certs/node.crt -text -noout

# Check logs
journalctl -u brio-kernel -f
```

**Issue: High latency between nodes**

```bash
# Test network latency
ping -c 10 worker-1

# Check gRPC latency
brio-kernel mesh ping worker-1

# Review topology
brio-kernel mesh topology
```

**Issue: Database connection failures**

```bash
# Verify database connectivity
psql -h 192.168.1.10 -U brio -c "SELECT 1"

# Check connection pool status
brio-kernel debug db-status
```

---

## Summary

A distributed Brio-Kernel deployment provides:

- **Horizontal scaling** across multiple machines
- **High availability** through redundant coordinators
- **Geographic distribution** for latency-sensitive workloads
- **Resource specialization** with dedicated worker nodes

Key operational considerations:

1. Always use PostgreSQL (not SQLite) for multi-node deployments
2. Enable mTLS for production security
3. Monitor mesh health and latency
4. Plan for rolling updates to minimize downtime
5. Maintain backups of both database and configurations

For additional help, see:
- [Configuration Reference](../getting-started/configuration.md)
- [Troubleshooting Guide](../reference/troubleshooting.md)
- [Architecture Overview](../concepts/architecture.md)
