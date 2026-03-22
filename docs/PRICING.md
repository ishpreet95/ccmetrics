# Embedded Pricing Reference

Source: [Anthropic Pricing](https://platform.claude.com/docs/en/about-claude/pricing)
Last verified: 2026-03-22

This file is the source of truth for the embedded pricing table in the binary.
Update this file and rebuild when Anthropic changes pricing.

## Per-Model Rates ($ per million tokens)

| Model ID | Input | Output | 5m Cache Write | 1h Cache Write | Cache Read |
|---|---|---|---|---|---|
| claude-opus-4-6 | 5.00 | 25.00 | 6.25 | 10.00 | 0.50 |
| claude-opus-4-5 | 5.00 | 25.00 | 6.25 | 10.00 | 0.50 |
| claude-opus-4-5-20251101 | 5.00 | 25.00 | 6.25 | 10.00 | 0.50 |
| claude-opus-4-1 | 15.00 | 75.00 | 18.75 | 30.00 | 1.50 |
| claude-opus-4 | 15.00 | 75.00 | 18.75 | 30.00 | 1.50 |
| claude-sonnet-4-6 | 3.00 | 15.00 | 3.75 | 6.00 | 0.30 |
| claude-sonnet-4-5 | 3.00 | 15.00 | 3.75 | 6.00 | 0.30 |
| claude-sonnet-4-5-20250929 | 3.00 | 15.00 | 3.75 | 6.00 | 0.30 |
| claude-sonnet-4 | 3.00 | 15.00 | 3.75 | 6.00 | 0.30 |
| claude-haiku-4-5 | 1.00 | 5.00 | 1.25 | 2.00 | 0.10 |
| claude-haiku-4-5-20251001 | 1.00 | 5.00 | 1.25 | 2.00 | 0.10 |
| claude-haiku-3-5 | 0.80 | 4.00 | 1.00 | 1.60 | 0.08 |

## Pricing Multipliers

| Modifier | Multiplier | Applies to | Condition |
|---|---|---|---|
| 5m cache write | 1.25x input | cache_creation (ephemeral_5m) | Default cache tier |
| 1h cache write | 2.0x input | cache_creation (ephemeral_1h) | Extended cache |
| Cache read | 0.1x input | cache_read_input_tokens | Cache hit |
| Fast mode | 6.0x all | input + output | speed: "fast", Opus 4.6 only |
| Data residency | 1.1x all | input + output + cache | inference_geo: US-only, Opus 4.6+ |
| Long context | 2.0x input, 1.5x output | All tokens when input > 200k | Sonnet 4.5/4 only |
| Batch API | 0.5x all | All token types | Not applicable to Claude Code |

## Additional Costs

| Feature | Cost | Field in JSONL |
|---|---|---|
| Web search | $10 per 1,000 searches | server_tool_use.web_search_requests |
| Web fetch | Free (token cost only) | server_tool_use.web_fetch_requests |
| Code execution | Free with web search/fetch; $0.05/hr otherwise | server_tool_use.code_execution_requests |

## How to Apply

```
cost = 0
for each unique request (deduplicated by requestId, final chunk):
    model = request.model
    rates = PRICING_TABLE[model]

    # Base costs
    input_cost   = request.input_tokens * rates.input / 1_000_000
    output_cost  = request.output_tokens * rates.output / 1_000_000

    # Cache costs (split by tier)
    cache_5m_cost = request.ephemeral_5m_input_tokens * rates.cache_write_5m / 1_000_000
    cache_1h_cost = request.ephemeral_1h_input_tokens * rates.cache_write_1h / 1_000_000
    cache_read_cost = request.cache_read_input_tokens * rates.cache_read / 1_000_000

    # Modifiers
    if request.speed == "fast":
        multiply all costs by 6.0
    if request.inference_geo == "us" and model >= opus-4-6:
        multiply all costs by 1.1
    if model in [sonnet-4-5, sonnet-4] and total_input > 200_000:
        input_cost *= 2.0
        output_cost *= 1.5

    # Server tool costs
    web_search_cost = request.web_search_requests * 10.0 / 1_000

    cost += input_cost + output_cost + cache_5m_cost + cache_1h_cost + cache_read_cost + web_search_cost
```
