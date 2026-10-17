# P6: Claude's usage readings (2026-10-17)

The owner, of R5: "Thats fine but if there are any important findings with reading endpoints consider bypassing."

## The endpoint, read once

`GET https://api.anthropic.com/api/oauth/usage` with Claude Code's OAuth token, read into memory from
`~/.claude/.credentials.json` and sent once; nothing printed or written but the response. HTTP 200 at 21:31Z.

What it holds beyond `claude -p /usage`:

| Field | Value | Use to herdr |
|---|---|---|
| `five_hour`, `seven_day` | `utilization` 34.0 and 36.0, `resets_at` to the microsecond | the same whole percents as `/usage` |
| `limits[]` | session 34, weekly_all 36, weekly_scoped "Fable" 0; each with `severity` and `is_active` (which limit binds now: the week) | `is_active` names the binding window; herdr computes the same from the numbers |
| `weekly_scoped_shares[0]` | Fable `used_percent_of_weekly` 0, `allowance_percent_of_weekly` 50 | **answers P1's Fable question**: Fable spends the all-models week, capped at half of it |
| `seven_day_breakdown.rows` | Claude Code 81%, Cowork 18%, Chats 1% of the week | use from other surfaces, which the local `/usage` notes cannot see |
| `seven_day_opus`, `seven_day_sonnet`, others | null | no per-model weekly window on this plan |
| `extra_usage`, `spend` | off; credits off | none |

**Verdict:** nothing that changes a seating. `/usage` gives the same windows at no cost and with no credential.
herdr does not read the endpoint (R5 stands at "no"). The two facts it gave are written into plan 12 instead.

## The statusline

Not run: it needs an interactive Claude turn. Claude Code's statusline input documents `rate_limits` with
`five_hour` and `seven_day`, `used_percentage` and `resets_at`; Nirvana Claude's statusline reads them in O4 and the
O4 live check records them.

## A note on this session's own use

Between the first `/usage` read of this design (49% of a window that reset at 19:50Z) and this read, the new session
window reached 34% and the week went from 28% to 36%. Most of it is this session's research agents and probes.
