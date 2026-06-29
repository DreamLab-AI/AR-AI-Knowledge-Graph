---
title: Natural Language Queries Tutorial
description: VisionClaw translates natural language questions into Cypher queries using LLM-powered semantic understanding.
category: how-to
tags:
  - tutorial
  - documentation
  - reference
  - visionclaw
updated-date: 2025-12-18
difficulty-level: intermediate
---


# Natural Language Queries Tutorial

## Overview

VisionClaw translates natural language questions into Cypher queries using LLM-powered semantic understanding.

## Quick Start

### Simple Query

```bash
POST /api/nl-query/translate
Content-Type: application/json

{
  "query": "Show me all person nodes",
  "suggestAlternatives": false
}
```

Response:
```json
{
  "translations": [{
    "originalQuery": "Show me all person nodes",
    "cypherQuery": "MATCH (n:KGNode {node_type: 'person'}) RETURN n LIMIT 50",
    "explanation": "Finds all nodes with type 'person'",
    "confidence": 0.95,
    "warnings": []
  }]
}
```

## Query Patterns

### Find Relationships

```
"What are the dependencies of Project X?"
```

Generates:
```cypher
MATCH (p:KGNode {label: 'Project X'})-[r:EDGE {relation_type: 'dependency'}]->(dep:KGNode)
RETURN dep
```

### Path Queries

```
"Show me the shortest path between Alice and Bob"
```

Generates:
```cypher
MATCH path = shortestPath(
  (a:KGNode {label: 'Alice'})-[*]-(b:KGNode {label: 'Bob'})
)
RETURN path
```

### Neighborhood Queries

```
"Find all nodes within 2 hops of Node X"
```

Generates:
```cypher
MATCH (start:KGNode {label: 'Node X'})-[*1..2]-(connected:KGNode)
RETURN DISTINCT connected LIMIT 100
```

## Advanced Features

### Multiple Suggestions

Request alternative interpretations:

```json
{
  "query": "connected nodes",
  "suggestAlternatives": true
}
```

Returns 3 different Cypher interpretations.

### Explain Cypher

Reverse translation - explain what a Cypher query does:

```bash
POST /api/nl-query/explain
Content-Type: application/json

{
  "cypher": "MATCH (n)-[r*1..3]-(m) RETURN n, m LIMIT 10"
}
```

### Validate Syntax

Check Cypher before execution:

```bash
POST /api/nl-query/validate
Content-Type: application/json

{
  "cypher": "MATCH (n:KGNode) RETURN n"
}
```

## Examples

Get curated examples:

```bash
GET /api/nl-query/examples
```

Returns:
- Person node queries
- Dependency relationship queries
- Hierarchy queries
- Path queries
- Neighborhood queries

## Tips

1. **Be specific**: "person nodes" vs "nodes"
2. **Use labels**: Mention specific node labels for better results
3. **Mention relationships**: "connected by dependency" vs "connected"
4. **Check confidence**: Low confidence? Try rephrasing
5. **Review before execution**: Always validate generated queries

---

## Related Documentation

- [Intelligent Pathfinding Guide](intelligent-pathfinding.md)
- [Physics & GPU Engine](../../explanation/physics-gpu-engine.md)
- [Contributing Guidelines](../../CONTRIBUTING.md)
- Goalie Integration - Goal-Oriented AI Research
- [VisionClaw Guides](../index.md)

## Schema Context

The service automatically uses your graph schema to generate appropriate queries. It knows:
- Available node types
- Available edge types
- Common properties
- OWL classes and properties
