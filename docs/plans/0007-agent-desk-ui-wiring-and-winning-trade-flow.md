# 0007 — Agent Desk UI Wiring & Winning-Trade Flow

> **Status**: Approved Design & Implementation Roadmap (docs/design/plan only — no UI code landed yet)
> **Target Scope**: `tantra-ui` (`tantra-tui`, `tantra-cli`, `tantra-ui-app` web AgentChat / Trade Lab,
> result blocks) wired to the `tantra-agent` 14-persona desk roster
> **Reference Inputs**
> - `tantra-agent/docs/plans/0012-agent-roster-culmination-prism-fincept.md` (the 14-persona desk)
> - `tantra-agent/docs/design/agent-roster-culmination.md` (persona & lens specifications)
> - `tantra-ui/docs/reference/cli-and-tui.md` (existing `--persona`, `/agent`, slash palette)
> - `tantra-ui/docs/plans/0002` (AgentChat UX reference) and `0003` (MCP result-block rendering)
> - Prism Insight winning-trade pipeline: <https://github.com/dragon1086/prism-insight> (README
>   "How PRISM Trades Now" — screen → desk analysis → buy rubric → small first buy → scenario adds →
>   leader hold → re-entry → weekly review)
> - Fincept persona-as-config convention: <https://github.com/Fincept-Corporation/FinceptTerminal>

---

## 0. Executive Summary

The agent desk roster (plan `tantra-agent` 0012) culminates 59 source agents into **14 Tantra
personas** (9 existing + 5 new: Trading Flow Analyst, Industry Analyst, Information Analyst,
Investment Strategist, Geopolitics Analyst) plus **lenses** (6 economic schools, 11 investor
mentors, 20 geopolitics modes) folded into Macro Strategist / Investment Strategist / Geopolitics
Analyst.

This plan wires that roster into every UI surface and defines the **Winning-Trade Flow**: the guided
end-to-end journey from "find a good trade" to a **risk-gated, placed, monitored and journaled**
position. The flow mirrors the proven screen → analyze → buy-gate → small-first-buy → scenario-add →
leader-hold → re-entry → review loop of Prism Insight, re-implemented on Tantra's zero-float,
SEBI-risk-gated, advisory-only stack.

**Design decisions**
1. **Personas and lenses are first-class CLI/TUI citizens.** `--persona`, `/agent` and `/create-agent`
   gain the 5 new personas; a new `--lens` / `/lens` selects mentor/school/geopolitics lenses.
2. **One command runs the desk.** `tantra ask --desk <symbol>` (CLI) and `/desk <symbol>` (TUI)
   run the multi-agent desk in sequence and render one lens card per persona (result blocks).
3. **Trade placement is always risk-gated.** The flow ends in an Order Composer that only accepts a
   Trade Plan that has passed `Decision → DecisionValidator → RiskEngine`. Paper is the default mode;
   live requires broker auth and the human-in-the-loop approval queue.
4. **Feedback closes the loop.** Closed trades feed memory/learnings and per-trigger win-rate so
   screening weights and future buy decisions improve (self-improving loop, Prism-style).

---

## 1. Roster & Lens Wiring (all surfaces)

### 1.1 Persona names (additive)

| Surface | Today (9) | After (14) |
|---------|-----------|------------|
| `tantra ask --persona <name>` | tantra-ai, analyst, quant, trader, portfolio-manager, risk-manager, macro-strategist, value-investor, devils-advocate | + `trading-flow-analyst`, `industry-analyst`, `information-analyst`, `investment-strategist`, `geopolitics-analyst` |
| TUI `/agent <name>` + palette | same 9 | same 14 |
| `/create-agent` custom personas | unchanged | unchanged (still user-defined) |
| Web AgentChat persona picker | 9 | 14 |

Aliases map folded sources to host personas (e.g. `--persona technical-analyst` → trader,
`--persona buy-specialist` → trader, `--persona market-analyst` → macro-strategist,
`--persona summary-optimizer` → tantra-ai) — resolution lives in `tantra-agent-prompt`, UI just
passes the name through.

### 1.2 Lens selection (new)

- **CLI**: `--lens <name>` on `tantra ask`/`chat`/`jobs` (e.g. `tantra ask --persona macro-strategist
  --lens keynesian "RBI repo cut impact on banks"`; `--persona investment-strategist --lens buffett
  "Nestle India"`; `--persona geopolitics-analyst --lens middle-east "crude risk"`).
- **TUI**: `/agent macro-strategist@keynesian` syntax and a `/lens <name>` command that switches the
  lens of the active persona; palette lists available lenses per persona.
- **Web**: lens dropdown on the persona picker.
- Lenses are invalid combinations (e.g. `--lens buffett` with persona `trader`) → clear CLI/TUI error.

### 1.3 Command palette & help text

- `/agent` argument list + palette hint text update; `/lens` new palette entry.
- `--persona` / `--lens` help strings in `tantra-cli` args (docs reference `cli-and-tui.md` §3.2).
- `--desk <symbol>` new command (see §2.2).

---

## 2. UI Wiring Specification

### 2.1 `tantra ask --desk <symbol>` / `/desk <symbol>`

Runs the desk in a fixed, ordered pipeline and renders one **lens card** per stage:

1. Technical read → Trader
2. Trading-flow read → Trading Flow Analyst
3. Fundamental read → Analyst
4. Industry read → Industry Analyst
5. News & catalysts → Information Analyst
6. Market read → Macro Strategist
7. Geopolitics (only when requested or when macro flags event risk) → Geopolitics Analyst
8. Synthesis → Investment Strategist (thesis card)
9. Red team → Devil's Advocate (weakest-assumption card)
10. Risk verdict → Risk Manager (downside + sizing card)

Output contract: one result block per card (`tantra/result/v1` DisplayBlocks — table / metric-grid /
chart from plan `tantra-ui` 0003), collated into a **Desk Report** saved to `/artifacts`. Flags
"**Proceed to trade plan?**" only when the desk passes minimum conviction thresholds.

### 2.2 Trade Plan card & Order Composer (TUI / web)

- The Investment Strategist thesis card ends with a **Trade Plan block** (direction, entry trigger,
  stop, target 1/2, risk-reward, sized quantity in exact `Decimal`, scenario-add and leader-hold
  rules, kill criteria).
- **Order Composer** (`/trade` in TUI, "Place" button on the web Trade Lab): pre-filled from the
  Trade Plan; every field editable; **locked unless risk-gated**.
- Risk-gate strip inside the composer shows live `tantra-risk` checks: SEBI algo-id/OTR state, price
  band, freeze limit, margin headroom, concentration limit (`/account`, `risk_inspect`).
- Modes: `paper` (default) / `backtest` (replay this plan) / `live` (requires broker auth; queues
  through `tantra jobs approvals` when `--approval manual`).
- Statutory charge preview computed with `tantra_calculate` (STT, GST, stamp duty, exchange fees).

### 2.3 Positions, monitoring & alerts (TUI / web)

- `/positions` and web Positions tab gain per-position **plan cards**: remaining scenario-add
  conditions, leader-hold status, profit-protection stop, re-entry watch window.
- Alerts (`price_alert` toolset, plan 0006) bind to the plan's stop/target/scenario levels.
- TUI monitoring rows reuse existing watchlist/portfolio panel patterns with the plan-card metadata.

### 2.4 Web (Leptos `tantra-ui-app`)

- AgentChat tab gains the 14-persona picker + lens dropdown + **Desk mode** (renders the ordered
  lens cards from §2.1) and **Trade Lab** panel (thesis card → Order Composer → risk strip →
  placement → position plan cards).
- Reuses plan 0003 result-block rendering; no new WASM primitives required beyond the Desk/Trade
  state models and their API calls (read-only `tantra-runtime-api-rest` + `jobs` queue).

### 2.5 CLI surface additions (summary)

```bash
tantra ask "<prompt>" --desk <symbol>                     # run full desk on a symbol
tantra ask "<prompt>" --persona <name> [--lens <lens>]    # single persona (+ lens)
tantra trade --plan <plan-id> --mode paper|backtest|live  # place a validated trade plan
tantra jobs approve <id>                                  # human-in-the-loop gate (live)
```

---

## 3. Winning-Trade Flow (user journey)

The flow is the UI embodiment of the Prism "screen → analyze → buy → hold → review" loop, mapped to
Tantra surfaces. Numbers in parentheses reference the surfaces above.

### Stage 0 — Entry (find candidates)
1. User asks `tantra ask "find trades in NIFTY IT"` or runs `tantra scan` / the Screener tab.
2. Screening returns candidates with **trigger quality weights** (from the last 180 days' own
   record: how often a trigger's candidates reached +20% and the average realized result).
3. User picks a candidate → `--desk RELIANCE` / `/desk RELIANCE`.

### Stage 1 — Desk analysis (lens cards)
4. The 10-stage desk renders lens cards (§2.1). Each card cites its fetched figures (as-of time) —
   no memory prices.
5. Conflict between lenses is flagged on the synthesis card ("Trader bullish on momentum, Analyst
   neutral on valuation — resolved toward …").

### Stage 2 — Thesis & trade plan
6. Investment Strategist produces the **thesis card** + **Trade Plan block** (direction, conviction,
   entry, stop, targets, risk-reward ≥ 2:1, size, add scenarios, kill criteria).
7. Optional: `--lens buffett|graham|…` re-weights the thesis under a mentor discipline.
8. Devil's Advocate + Risk Manager cards attach their verdicts to the plan (weakest assumptions,
   downside in rupees, max size).

### Stage 3 — Buy gate & risk gate
9. **Buy gate**: plan must clear the entry rubric (score ≥ regime floor on fundamentals / momentum /
   trend) and stop width within regime limits (−5%…−7% band default).
10. **Risk gate**: plan routes through `Decision → DecisionValidator → RiskEngine`; the Order
    Composer unlocks only on PASS, and the risk strip shows which checks passed.
11. User reviews the **validated order** (symbol, side, qty, price, algo_id, rationale), previews
    statutory charges, and chooses **paper** (default) / **backtest** / **live**.

### Stage 4 — Placement & sizing
12. Paper/backtest place immediately; live places via the approval queue (`--approval manual`).
13. Position starts small: **10-slot sizing**, first buy 30–80% of a slot, volatility-scaled.
14. At entry, the plan's **add scenarios** arm (breakout / recovered pullback); code buys more only
    on scenario conditions, above average cost, within the original risk budget.

### Stage 5 — Manage & monitor
15. Positions tab shows plan cards; alerts fire on stop/target/scenario levels.
16. **Leader-hold**: a stock closing +20% in 4–15 sessions without stretching above the 50-day line
    is held (sold only on a close below the 50-day line or below first-buy price) for up to 40
    sessions; fast +20% runners keep normal profit-protection stops.
17. **Re-entry watch**: stopped-out names are watched up to 60 sessions; reclaiming the key level
    near the close triggers an AI re-check, max 3 attempts per watch, 2 re-entries per market/day.

### Stage 6 — Review & journal (win-rate loop)
18. Closed trades journal per-trade rows (exit reason, hold days, realized gain); `/memory` and
    `/learnings` distill the results.
19. Weekly runner report (big-winner capture, missed winners, stop-loss cost, results by trigger)
    updates **trigger quality weights** — feeding Stage 0 screening and the buy rubric, closing the
    self-improving loop.

### Flow guardrails (binding)
- **Advisory only**: no step emits an un-gated order; every placement passes the risk gate (§Stage 3).
- **Zero floats**: all prices, sizes, P&L are exact `Decimal` (rendered via `rust_decimal`).
- **Paper by default**: live requires broker auth + approval; `--skip-permissions` never bypasses
  SEBI pre-trade risk gates.
- **Falsifiable**: every plan carries kill criteria and a "what would prove this wrong" line.

---

## 4. Implementation Phases

> **Current status**: Phase 0 only — this plan + the companion design doc land documentation.

- **Phase 0 (this plan)**: UI wiring spec + winning-trade flow + design doc
  (`docs/design/agent-desk-ui-and-winning-trade-flow.md`).
- **Phase 1 — agent surface**: depends on `tantra-agent` plan 0012 Phases 1–2 (persona enum, skills,
  alias wiring). UI then: `--persona`/`/agent`/web picker gain the 5 names + aliases; palette/help
  text updates; count-pin tests (`tantra-cli`, `tantra-tui`) updated.
- **Phase 2 — lens flag**: `--lens`/`/lens` parsing + validation (depends on `tantra-agent` plan 0012
  Phase 3 lens representation).
- **Phase 3 — desk command**: `--desk <symbol>`/`/desk <symbol>` ordered multi-persona runner +
  lens-card result blocks + Desk Report artifact (depends on plan `tantra-agent` 0012 Phase 4 and
  `tantra-ui` 0003 result-block rendering).
- **Phase 4 — trade flow**: Trade Plan card → Order Composer (risk-gate strip) → placement modes →
  position plan cards → alerts binding → journal/review loop. Native CLI (`tantra trade`) + TUI
  first, web Trade Lab second.
- **Phase 5 — verification**: `cargo clippy --workspace --all-targets --profile prod -D warnings`,
  `cargo test --workspace --profile prod`, WASM compile check; deterministic UI-flow tests in
  `tests/` (parallel test directory rule).

---

## 5. Invariants & Cleanup

- Advisory-only end-to-end; no bypass of `tantra-risk`.
- Zero floats on every displayed financial quantity.
- WASM crates stay blocking-I/O-free and `wasm32-unknown-unknown`-clean.
- When Phases 1–5 land, close this plan in `docs/plans/README.md` (Implemented table) and fold the
  design doc's settled parts into `docs/reference/cli-and-tui.md`.
- Upstream Prism/Fincept repos are read-only references; no changes pushed there.
