# agentcache

A production-grade agentic LLM proxy in Rust. Not just semantic caching — deterministic tool normalization, agent state reuse, and cross-provider orchestration for pipelines that need to skip the model entirely when it's safe to do so.

## The Problem

Native provider caching (Anthropic, OpenAI) handles exact prefix matching well. But agentic pipelines break it constantly:

- Tool definitions serialized with randomized key order → prefix cache miss every time
- System timestamps or random IDs in tool calls → guaranteed cache invalidation
- Same planning objective expressed differently across turns → no reuse
- Multiple API keys or workspaces → Anthropic's February 2026 workspace isolation blocks shared cache

The result: you pay for full inference on requests that are semantically identical to something you already computed.

`agentcache` sits between your agent and the LLM provider and fixes this.

## What It Does

```
Agent → agentcache → [normalize + hash + semantic lookup]
                   ↓ hit              ↓ miss
              Cached state       [provider-native cache hints]
                                       ↓
                              Anthropic / OpenAI / vLLM
```

**1. Deterministic Tool Normalization**
Intercepts outgoing requests and normalizes tool definitions before they hit the provider: alphabetical JSON key sorting, stripping dynamic variables (timestamps, random IDs, session tokens), enforcing schema formatting stability. This alone guarantees near-100% prefix cache hit rate on Anthropic and OpenAI for requests that would otherwise miss due to serialization non-determinism.

**2. Agentic State Caching**
Doesn't cache raw prompt text → raw response text. Caches agent planning steps, tool-routing decisions, and structured reasoning paths based on semantic similarity of the *objective*, not the surface text. Reuses intermediate states across turns when the goal is equivalent.

**3. Cross-Workspace / Cross-Provider Bridging**
Overcomes Anthropic's workspace-level cache isolation. Shares tool definitions, system context, and cached states across API keys, workspaces, or business units. Also routes across providers (Anthropic, OpenAI, local vLLM) with a unified cache layer.

**4. Cache Safety Layer**
Not all cache hits are good. `agentcache` applies TTL and freshness rules by request type: mathematical/structural queries get long TTLs, time-sensitive or user-specific queries bypass the cache entirely. Prevents the stale-answer problem that makes naive semantic caches dangerous in production.

**5. Full Observability**
Every request emits: hit/miss, avoided tokens, latency saved, similarity score, provider used. OpenTelemetry spans for distributed tracing. Audit trail for cache hits — you can always see what was returned and why.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     agentcache proxy                        │
│                    (Axum, async Rust)                       │
│                                                             │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────────┐  │
│  │    Tool     │   │   Semantic   │   │   Freshness &   │  │
│  │ Normalizer  │→  │    Lookup    │→  │  Safety Rules   │  │
│  │ (JSON sort, │   │ (embeddings, │   │  (TTL, bypass,  │  │
│  │  strip IDs) │   │  cosine sim) │   │   audit log)    │  │
│  └─────────────┘   └──────────────┘   └─────────────────┘  │
│                           │                                  │
│              hit ─────────┴──────── miss                    │
│               │                        │                    │
│        Return cached            Add cache-control           │
│           state                 hints → provider            │
└─────────────────────────────────────────────────────────────┘
```

## Stack

| Component | Technology |
|---|---|
| Language | Rust (async, tokio) |
| HTTP server | Axum |
| Cache backend | Redis / Valkey |
| Embeddings | Candle (local) or remote API |
| Vector search | faiss-rs or Qdrant bindings |
| Observability | OpenTelemetry + tracing |
| Providers | Anthropic, OpenAI, vLLM (pluggable) |
| Config | TOML + environment variables |

## Why Rust

This proxy sits in the critical path of every agent call. It needs to be faster than the network overhead it's saving — sub-2ms per request. Rust gives us predictable latency, no GC pauses, and safe concurrency across the async request pipeline. A Python proxy here would defeat the purpose.

## Milestones

- [x] Project scaffold — Axum proxy, basic passthrough to Anthropic API
- [x] Request/response logging with structured tracing
- [ ] Tool definition normalizer — JSON key sorting, dynamic variable stripping
- [ ] Redis integration — cache store and retrieval
- [ ] Embedding layer — prompt vectorization via Candle
- [ ] Semantic similarity search — cosine distance with configurable threshold
- [ ] Freshness rules — TTL by request type, bypass rules for time-sensitive queries
- [ ] Cross-provider adapter — OpenAI + vLLM support
- [ ] Observability — hit rate, avoided tokens, latency saved, bad-hit audits
- [ ] Docker image + deployment guide
- [ ] Benchmarks — measured cost and latency reduction on real agent workloads

## Configuration

```toml
[server]
port = 8080

[providers]
default = "anthropic"

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-6"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
model = "gpt-4o"

[cache]
backend = "redis"
redis_url = "redis://localhost:6379"
similarity_threshold = 0.92

[cache.ttl]
structural = 2592000   # 30 days — math, schemas, tool routing
conversational = 3600  # 1 hour — general agent states
time_sensitive = 0     # bypass — weather, prices, current events

[normalization]
sort_json_keys = true
strip_timestamps = true
strip_random_ids = true
```

## Usage

```bash
# Run the proxy
cargo run --release

# Point your agent at agentcache instead of the provider directly
export ANTHROPIC_BASE_URL=http://localhost:8080

# Your agent code doesn't change
client = anthropic.Anthropic()
response = client.messages.create(...)
```

## What Native Provider Caching Doesn't Cover

| Problem | Anthropic/OpenAI native | agentcache |
|---|---|---|
| Exact prefix match | ✅ | ✅ |
| Non-deterministic tool serialization | ❌ breaks cache | ✅ normalizes |
| Semantic similarity across turns | ❌ | ✅ |
| Cross-workspace sharing | ❌ isolated | ✅ |
| Cross-provider unified cache | ❌ | ✅ |
| Freshness / safety rules | ❌ | ✅ |
| Agent state reuse | ❌ | ✅ |
| Observability on cache decisions | ❌ | ✅ |

## Target Performance

| Scenario | Without agentcache | With agentcache |
|---|---|---|
| Tool normalization boost | ~40% prefix cache hit | ~95% prefix cache hit |
| Semantic hit (agent state) | Full inference ~800ms | <5ms |
| Expected token cost reduction | baseline | 40–70% |

## Status

🚧 Active development. Proxy scaffold and tracing done. Tool normalizer next.

## License

MIT
