# Next upgrades shipped (2026-08-08)

## Done

1. **@mention → person map** — Slack `<@U…>` and free-text names resolve via team directory (`GET …/people/directory`). Commitments show human labels (`promiser_label` / `promisee_label`).
2. **Morning commitment digest** — preview + send APIs; background loop at `COMMITMENT_DIGEST_HOUR_UTC` (default 13) posts to `COMMITMENT_DIGEST_CHANNEL` once/day.
3. **Linear one-way export** — `POST …/commitments/{id}/export_linear` if `LINEAR_API_KEY` + `LINEAR_TEAM_ID` set. Commitment remains source of truth.
4. **Simple / Technical mode** — toggle like light/dark; Simple hides graph, lab, heat, technical ledger, Neon ops; defaults to Simple.

## Env (optional)

| Env | Role |
|-----|------|
| `COMMITMENT_DIGEST_CHANNEL` | Slack channel id for morning digest |
| `COMMITMENT_DIGEST_HOUR_UTC` | Hour to send (default 13) |
| `COMMITMENT_DIGEST_ENABLED` | `true`/`1` (default true if channel set) |
| `LINEAR_API_KEY` | Linear personal/API key |
| `LINEAR_TEAM_ID` | Linear team UUID |

## What more we can do later

- Close commitment when Linear issue closes (webhook)
- Per-person DM of “you owe” list (not only channel digest)
- Emoji reactions as commit/done (✅ on promise message)
- Calendar due-date extraction (“by Friday”)
- Export to Jira same pattern as Linear
- Mobile-friendly “My commitments” employee page
- Confidence training from mark-done vs auto-detect
