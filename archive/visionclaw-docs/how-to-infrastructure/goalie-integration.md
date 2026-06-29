---
title: Goalie Integration - Goal-Oriented AI Research
description: Goalie is integrated as an MCP service providing goal-oriented AI research with anti-hallucination features using the Perplexity API.
category: how-to
tags:
  - tutorial
  - api
  - documentation
  - reference
  - visionclaw
updated-date: 2025-12-18
difficulty-level: advanced
---


# Goalie Integration - Goal-Oriented AI Research

Goalie is integrated as an MCP service providing goal-oriented AI research with anti-hallucination features using the Perplexity API.

## Features

- **GOAP Planning**: Goal-Oriented Action Planning with A* pathfinding
- **Deep Research**: Multi-step research with 20-30 sources per query
- **Anti-Hallucination**: Citation tracking, Ed25519 signatures, validation
- **Advanced Reasoning**: Chain-of-thought, self-consistency, multi-agent verification
- **Cost Effective**: $0.006 per query, $0.02-0.10 for complex research

## Configuration

### 1. Get Perplexity API Key

Visit https://www.perplexity.ai/settings/api and get your API key.

### 2. Add to .env

```bash
# Perplexity Configuration for Goalie
PERPLEXITY-API-KEY=pplx-your-key-here
GOAP-MAX-REPLANS=3
GOAP-CACHE-TTL=3600
```

### 3. Goalie MCP Server

The Goalie MCP server runs automatically via supervisord on port 9504.

## Available Tools

### goalie-search

Deep research with GOAP planning and anti-hallucination.

**Parameters:**
- `query` (required): Research question or topic
- `mode` (optional): 'web' or 'academic' (default: 'web')
- `maxResults` (optional): Maximum results (default: 10)
- `domains` (optional): Comma-separated domain list
- `verify` (optional): Enable Ed25519 verification (default: false)

**Example:**
```json
{
  "name": "goalie-search",
  "arguments": {
    "query": "What are the legal requirements for starting a food truck in California?",
    "mode": "web",
    "maxResults": 15,
    "domains": "ca.gov,sba.gov"
  }
}
```

### goalie-query

Quick search without full GOAP planning.

**Parameters:**
- `query` (required): Quick search query
- `limit` (optional): Limit results (default: 5)

**Example:**
```json
{
  "name": "goalie-query",
  "arguments": {
    "query": "What is an LLC?",
    "limit": 5
  }
}
```

### goalie-reasoning

Advanced reasoning methods.

**Parameters:**
- `query` (required): Query for reasoning
- `method` (required): 'chain-of-thought', 'self-consistency', 'anti-hallucination', or 'agentic'

**Example:**
```json
{
  "name": "goalie-reasoning",
  "arguments": {
    "query": "Analyze the impact of AI on employment",
    "method": "chain-of-thought"
  }
}
```

## CLI Usage

Access Goalie directly via CLI:

```bash
# Deep research
goalie search "Your research question"

# Quick query
goalie query "Quick question"

# Advanced reasoning
goalie reasoning chain-of-thought "Complex question"

# Academic research
goalie search "Your question" --mode academic

# Domain-specific
goalie search "FDA regulations" --domains "fda.gov,nih.gov"

# With verification
goalie search "Financial data" --verify
```

## Service Management

```bash
# Check status
supervisorctl status goalie-mcp

# View logs
tail -f /app/mcp-logs/goalie-mcp.log

# Restart
supervisorctl restart goalie-mcp
```

## Use Cases

### Legal Research
```bash
goalie search "Legal requirements for Delaware C-Corp with foreign investors" \
  --mode web \
  --max-results 20 \
  --domains "delaware.gov,sec.gov,irs.gov"
```

### Medical Research
```bash
goalie search "Latest Type 2 diabetes treatment options and effectiveness" \
  --mode academic \
  --max-results 15
```

### Market Analysis
```bash
goalie search "Tesla financial health and competitive position" \
  --max-results 20 \
  --verify
```

## Cost Comparison

| Task | Human | Goalie |
|------|-------|--------|
| Legal research (2 hours) | $100-300 | $0.02-0.05 |
| Market analysis | $500-1500 | $0.10-0.20 |
| Medical literature review | $200-500 | $0.05-0.10 |

## Output Organization

Results are saved to `.research/` directory:

```
.research/
├── your-query-slug/
│   ├── summary.md           # Executive summary
│   ├── full-report.md       # Detailed findings
│   ├── sources.json         # All citations
│   └── raw-data.json        # Original API responses
```

## Reasoning Plugins

### Chain-of-Thought
Explores 3+ reasoning paths with 85-95% confidence scoring.

### Self-Consistency
Runs 3+ independent samples with 90%+ agreement rates.

### Anti-Hallucination
100% citation requirement with low/medium/high risk scoring.

### Agentic Research
5+ specialized agents: Explorer, Validator, Synthesizer, Critic, Fact-checker.

## Performance Metrics

- **Sources**: 20-30 per complex query
- **Time**: 15-40 seconds for deep research
- **Confidence**: 89.5% average accuracy
- **Cost**: $0.006 average per query

## Troubleshooting

### API Key Not Set
```
ERROR: PERPLEXITY-API-KEY environment variable is required
```
Add `PERPLEXITY-API-KEY` to `.env` file.

### Service Not Running
```bash
supervisorctl status goalie-mcp
supervisorctl start goalie-mcp
```

### High API Costs
Set `GOAP-CACHE-TTL` higher (default: 3600 seconds) to cache results longer.

## References

- [Goalie NPM Package](https://www.npmjs.com/package/goalie)
- [Perplexity API](https://www.perplexity.ai/settings/api)
- [GOAP Planning Theory](https://en.wikipedia.org/wiki/GOAP)

---

## Related Documentation

- [Contributing Guidelines](../../CONTRIBUTING.md)
- [Semantic Forces User Guide](../../explanation/physics-gpu-engine.md)
- [Troubleshooting Guide](troubleshooting.md)
- [Natural Language Queries Tutorial](../features/natural-language-queries.md)
- [Intelligent Pathfinding Guide](../features/intelligent-pathfinding.md)

## Support

For issues or questions:
- Check logs: `/app/mcp-logs/goalie-mcp.log`
- Supervisord status: `supervisorctl status goalie-mcp`
