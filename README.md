# Order Matching Engine (Rust)

## Overview

This project is a high-performance in-memory order matching engine built in Rust.
It supports real-time order processing, WebSocket streaming, and a **Primary–Replica architecture** for distributed scaling.

The system follows a **single-writer design**, where all matching happens on a PRIMARY server, while multiple REPLICA servers maintain synchronized state and serve API requests.

---

## Configuration

### Symbol Configuration

The engine supports multiple trading symbols (e.g., BTC, ETH, SOL). Symbols are configured via `config.json`:

```json
{
  "symbols": ["BTC", "ETH", "SOL", "XRP"]
}
```

You can override the config file location with the `CONFIG_PATH` environment variable:

```bash
CONFIG_PATH=/path/to/config.json cargo run
```

Each symbol has its own isolated orderbook, and orders are matched within their respective symbol's orderbook.

---

## Core Features

* In-memory matching engine
* Price-time priority (FIFO)
* REST API for order submission and orderbook queries
* WebSocket streaming for real-time fill events
* Primary–Replica architecture for multi-instance support
* Snapshot + delta synchronization for replication

---

## API Endpoints

### POST /orders

Submit a new order

**Request:**
```json
{
  "symbol": "BTC",
  "side": "buy",
  "price": 100,
  "qty": 50
}
```

**Response:**
```json
{
  "order_id": 1,
  "fills": []
}
```

---

### GET /orderbook

Returns the current orderbook for a specific symbol

**Query Parameters:**
- `symbol`: The trading symbol (e.g., "BTC", "ETH")

**Example:**
```
GET /orderbook?symbol=BTC
```

**Response:**
```json
{
  "symbol": "BTC",
  "bids": [{ "price": 100, "qty": 50 }],
  "asks": []
}
```

---

### GET /ws

WebSocket endpoint for external clients

**Streams:**
* Real-time fill events

---

### GET /internal/sync

WebSocket endpoint used by REPLICA servers

**Streams:**
* Initial snapshot of orderbook
* Continuous orderbook updates (deltas)
* Fill events

---

## Running the Project

**Start PRIMARY server:**
```bash
IS_PRIMARY=true PORT=3000 cargo run
```

**Start REPLICA server:**
```bash
IS_PRIMARY=false PRIMARY_URL="http://localhost:3000" PORT=3001 cargo run
```

---

## Testing

**Place order on PRIMARY server:**
```bash
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{"symbol":"BTC","side":"buy","price":100,"qty":50}'
```

**Place order via REPLICA server (forwarded to PRIMARY server):**
```bash
curl -X POST http://localhost:3001/orders \
  -H "Content-Type: application/json" \
  -d '{"symbol":"ETH","side":"sell","price":100,"qty":30}'
```

**Check orderbook for a specific symbol:**
```bash
curl "http://localhost:3000/orderbook?symbol=BTC"
```

---

## Matching Engine

* Implements price-time priority:
  * Buy orders match lowest ask
  * Sell orders match highest bid
* Partial fills are supported
* Matching is done synchronously inside a mutex to ensure correctness under concurrent requests

---

## Design Decisions & Requirements Explanation

### Handling Multiple API Server Instances

The system uses a **single-writer architecture** to avoid double matching.

* Only the PRIMARY server processes and matches orders
* REPLICA servers forward all order requests to the PRIMARY server via HTTP
* REPLICA servers do not perform matching locally

This guarantees:
* No duplicate matching
* No race conditions across servers
* A single source of truth

Replication is handled via WebSocket:
* REPLICA server connects to PRIMARY server
* Receives snapshot on connect
* Applies continuous updates (deltas + fills)

This allows multiple API servers to run safely while maintaining consistency.

---

### 1. Orderbook Data Structure

The orderbook uses:
* BTreeMap for price levels
* VecDeque for per-price FIFO queues

**Structure:**
* `Engine`: Contains `HashMap<String, OrderBook>` - one orderbook per symbol
* Each `OrderBook` has:
  * `bids`: BTreeMap<price, VecDeque<Order>>
  * `asks`: BTreeMap<price, VecDeque<Order>>

**Reasoning:**

* Engine with HashMap:
  * Isolates orderbooks by symbol
  * Allows independent matching per symbol
  * Easy to add new symbols

* BTreeMap:
  * Maintains sorted order
  * Efficient access to best bid and ask

* VecDeque:
  * Efficient FIFO operations
  * Preserves time priority within a price level

This combination naturally supports price-time priority matching with symbol isolation.

---

### 2. Concurrency Handling

* A shared async mutex protects the orderbook
* Only one order is processed at a time
* Prevents race conditions during matching

**Important detail:**
* Lock is held only during matching
* Released before broadcasting updates

This ensures correctness while reducing contention.

---

### 3. WebSocket Feed

**External clients (`/ws`):**
* Receive fill events in real time

**Internal sync (`/internal/sync`):**
* Used by REPLICA servers
* Receives:
  * Snapshot
  * Orderbook updates (deltas)
  * Fill events

This enables real-time data streaming and replication.

---

### What Breaks First in Production?

The main limitations under real load would be:

1. Single PRIMARY server
   * All writes go through one node
   * Limits horizontal scalability

2. Global mutex on orderbook
   * Becomes a bottleneck under high throughput

3. Broadcast channels
   * Slow clients may drop messages

4. No persistence
   * Data is lost on restart

5. No sequencing in sync
   * Potential inconsistency if updates are missed

---

### What I Would Build Next (Next 4 Hours)

1. Add persistence layer
   * Store orders and trades (Postgres or RocksDB)

2. Add comprehensive unit and integration test cases
   * Validate matching logic (price-time priority, partial fills)
   * Test edge cases (empty book, large orders, exact matches)
   * Ensure correctness under concurrent order submissions

3. Improve concurrency
   * Sharded or lock-free orderbook

4. Add order cancellation and modification

5. Replace broadcast with durable messaging (Kafka)

---

## Summary

This project focuses on correctness and system design clarity.

It demonstrates:
* Matching engine implementation
* Safe concurrency in Rust
* Real-time streaming via WebSockets
* Distributed system tradeoffs using a single-writer model

---