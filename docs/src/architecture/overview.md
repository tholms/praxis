# Architecture Overview

Praxis has a distributed architecture designed for monitoring and controlling AI agents across multiple systems. Let's walk through how the pieces fit together.

## The Big Picture

```diagram
    ┌─────────────────┐                ┌─────────────────┐
    │   Web Browser   │                │   Claude Code   │
    │  (React SPA)    │                │  (SDK Client)   │
    └────────┬────────┘                └────────┬────────┘
             │ HTTP/WebSocket                   │ WebSocket (SDK)
    ┌────────▼─────────────────────────────────▼────────┐
    │                    Service                        │
    │              (Backend + SDK Server)               │
    └────────┬──────────────────────────────────────────┘
             │ RabbitMQ (AMQP)
    ┌────────▼──────────────────────────────────────────┐
    │               Connected Nodes                     │
    │  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
    │  │ Node A   │  │ Node B   │  │ Node C   │  ...    │
    │  └──────────┘  └──────────┘  └──────────┘         │
    └───────────────────────────────────────────────────┘
```

## Components

### Node

The node runs on target systems where AI agents are installed. It's the "eyes and hands" of Praxis on each endpoint.

**What it does:**
- Fingerprints installed agents
- Performs reconnaissance on agent configurations and sessions
- Intercepts traffic between agents and LLM backends
- Creates and manages sessions with agents
- Provides PTY terminal access to the system

**Key characteristics:**
- Stateless - all persistent data lives on the service
- Single binary, no dependencies
- Communicates with service over RabbitMQ

See [Node Architecture](./node.md) for details.

### Service

The service is the central backend that coordinates everything.

**What it does:**
- Tracks all connected nodes and their agents
- Stores configuration, operation definitions, and chain workflows
- Manages the semantic operations queue
- Executes chains by orchestrating multi-step workflows
- Persists intercepted traffic and recon results
- Handles LLM provider integrations

**Key characteristics:**
- Persistent storage (SQLite default, PostgreSQL for production)
- Stateful - knows about all nodes and their state
- Runs the operation manager and chain executor

See [Service Architecture](./service.md) for details.

### Web

The web component serves the frontend and provides the API.

**What it does:**
- Serves the React single-page application
- Provides WebSocket endpoint for real-time communication
- Handles HTTP requests for static assets
- Bridges between browser clients and the service

**Key characteristics:**
- React/TypeScript frontend with Tailwind CSS
- WebSocket for bidirectional communication
- Builds into the binary (embedded assets)

See [Web Architecture](./web.md) for details.

### SDK Server

The service also runs an optional SDK WebSocket server that allows Claude Code instances to connect directly as remote nodes.

**What it does:**
- Accepts WebSocket connections from Claude Code instances
- Manages SDK remote node sessions and state
- Routes tool requests and approvals between Claude Code and the service
- Applies permission modes and auto-approval policies

**Key characteristics:**
- Configurable via service settings (disabled by default)
- Runs on a separate port (8586 by default)
- Optional Bearer token authentication
- System prompt injection for connected instances

See [CLI Documentation](../usage/cli.md#sdk-remote-nodes) for usage details.

## Communication

### RabbitMQ

All communication between nodes, service, and web clients flows through RabbitMQ:

| Queue | Direction | Purpose |
|-------|-----------|---------|
| `NodeSignal` | Node → Service | Registration, traffic, recon results |
| `Node_{id}` | Service → Node | Commands, parser responses |
| `NodeBroadcast` | Service → All Nodes | Refresh requests (fanout exchange) |
| `ClientSignal` | Client → Service | UI requests |
| `Client_{id}` | Service → Client | Direct responses |
| `ClientBroadcast` | Service → All Clients | State updates (fanout exchange) |

RabbitMQ provides:
- Reliable message delivery
- Decoupling between components
- Easy scaling (nodes can come and go)
- Persistence for messages in flight

### Message Flow Example

Here's what happens when you run an operation from the UI:

1. **Browser** → WebSocket → **Web**
2. **Web** → `ClientSignal` queue → **Service**
3. **Service** queues operation, sends to node
4. **Service** → `Node_{id}` queue → **Node**
5. **Node** creates session, executes operation
6. **Node** → `NodeSignal` queue → **Service** (updates)
7. **Service** → `Client_{id}` queue → **Web**
8. **Web** → WebSocket → **Browser**

## Data Flow

### Intercepted Traffic

```diagram
Agent ─HTTPS─▶ Proxy ─▶ Node ─RabbitMQ─▶ Service ─▶ Database
                                           │
                                           └─▶ Web ─WebSocket─▶ Browser
```

### Operations

```diagram
Browser ─▶ Web ─▶ Service ─▶ LLM (planning)
                     │
                     └─▶ Node ─▶ Agent (execution)
                           │
                           └─▶ Output ─▶ Service ─▶ Browser
```

## Database Schema

The service stores everything in a relational database:

- **config** - key-value settings (LLM configs, etc.)
- **operation_definitions** - saved operation templates
- **semantic_operations** - operation execution history
- **chain_definitions** - workflow definitions
- **chain_executions** - workflow execution history
- **traffic_log** - intercepted HTTP traffic
- **intercept_rules** - traffic matching rules
- **recon_results** - cached reconnaissance data
- **application_logs** - centralized logging (controlled by `application_logs_enabled`)

## Deployment Patterns

### Development

Single machine running everything:
- Docker Compose with service, web, and RabbitMQ
- Node running locally for testing

### Production

Separate concerns:
- Service/Web on central server
- RabbitMQ (possibly managed service)
- Nodes deployed to target systems
- PostgreSQL for the database

### Cloud (Azure)

See [Azure Deployment](../deployment/azure.md):
- Container Apps for service/web
- Managed RabbitMQ or Container Instance
- Azure Database for PostgreSQL
