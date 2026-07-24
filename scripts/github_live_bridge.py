#!/usr/bin/env python3
"""
Always-on ingest bridge: V1 events → V2 graph → ensure person twins.

Does NOT Slack-DM. Status delivery is owned by twin-api's scheduled notify
loop (STATUS_WINDOW / NOTIFY_INTERVAL) so high-volume GitHub webhooks never
spam developers (ADR-014).

Hardening (permanent graph reliability):
- Gate on V2 /healthz before projecting (no stampede when V2 is wedged).
- Exponential backoff when V2 is down / timing out.
- Poison-skip events that fail repeatedly so one bad payload cannot stall the map.
- Periodic re-project when embedded V2 restarts empty (clears seen state).
"""
from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request
from collections import defaultdict

V1 = os.environ.get("V1_BASE_URL", "http://127.0.0.1:18080").rstrip("/")
V2 = os.environ.get("V2_BASE_URL", "http://127.0.0.1:18082").rstrip("/")
TWIN = os.environ.get("TWIN_BASE_URL", "http://127.0.0.1:18083").rstrip("/")
TENANT = os.environ.get("TENANT_ID", "ten_github")
STATE = os.environ.get("STATE_FILE", f"/tmp/ai_manager_bridge_seen_{TENANT}.txt")
POLL = float(os.environ.get("POLL_SECS", "5"))
# Cap work per tick so embedded graph-api is not flooded (avoids hangs on small VPS).
MAX_PER_TICK = int(os.environ.get("BRIDGE_MAX_PER_TICK", "3"))
PROJECT_PAUSE = float(os.environ.get("BRIDGE_PROJECT_PAUSE_SECS", "0.4"))
PROJECT_TIMEOUT = float(os.environ.get("BRIDGE_PROJECT_TIMEOUT_SECS", "12"))
HEALTH_TIMEOUT = float(os.environ.get("BRIDGE_HEALTH_TIMEOUT_SECS", "2.5"))
MAX_EVENT_FAILURES = int(os.environ.get("BRIDGE_MAX_EVENT_FAILURES", "5"))
EMPTY_GRAPH_CHECK_SECS = float(os.environ.get("BRIDGE_EMPTY_GRAPH_CHECK_SECS", "45"))
READER_PROVIDER = os.environ.get("BRIDGE_READER_PROVIDER_ID", "bridge_reader")
DEFAULT_SLACK = os.environ.get("SLACK_TEST_USER_ID", "").strip()
DEFAULT_CHANNEL = os.environ.get("SLACK_TEST_CHANNEL_ID", "").strip()
DEFAULT_NAME = os.environ.get("DEFAULT_DISPLAY_NAME", "Engineer").strip() or "Engineer"
# provider_user_id:slack_uid,login:slack_uid,global_user_id:slack_uid
RAW_MAP = os.environ.get("SLACK_USER_MAP", "")
# How often to refresh multi-person map from twin-api team admin (M6).
TEAM_MAP_REFRESH = float(os.environ.get("TEAM_MAP_REFRESH_SECS", "60"))


def parse_slack_map(raw: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for part in raw.split(","):
        part = part.strip()
        if not part or ":" not in part:
            continue
        k, v = part.split(":", 1)
        k, v = k.strip(), v.strip()
        if k and v:
            out[k] = v
    return out


# Env map is base; twin team API overlays (admin UI) for multi-person beta.
SLACK_MAP: dict[str, str] = parse_slack_map(RAW_MAP)
_LAST_TEAM_FETCH = 0.0
_LAST_EMPTY_CHECK = 0.0
_EVENT_FAILS: dict[str, int] = defaultdict(int)
_V2_BACKOFF = 0.0  # seconds to sleep when V2 unhealthy
_V2_DOWN_SINCE: float | None = None


def refresh_team_map(force: bool = False) -> None:
    """Merge GET /v3/tenants/{t}/team bridge_slack_map into SLACK_MAP (never DMs)."""
    global SLACK_MAP, _LAST_TEAM_FETCH
    if not TWIN:
        return
    now = time.time()
    if not force and (now - _LAST_TEAM_FETCH) < TEAM_MAP_REFRESH:
        return
    _LAST_TEAM_FETCH = now
    try:
        data = get(f"{TWIN}/v3/tenants/{TENANT}/team", timeout=10)
        overlay = data.get("bridge_slack_map") or {}
        if isinstance(overlay, dict):
            added = 0
            for k, v in overlay.items():
                ks, vs = str(k).strip(), str(v).strip()
                if ks and vs and SLACK_MAP.get(ks) != vs:
                    SLACK_MAP[ks] = vs
                    added += 1
            if added:
                print(
                    f"team map merge +{added} keys total={len(SLACK_MAP)} "
                    f"multi_person={data.get('multi_person_ready')}",
                    flush=True,
                )
    except Exception as e:
        print(f"team map refresh warn: {e}", flush=True)


def get(url: str, timeout: float = 15):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read().decode())


def post(url: str, obj: dict, timeout: float = 20):
    req = urllib.request.Request(
        url,
        data=json.dumps(obj).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            body = r.read().decode()
            return json.loads(body) if body else {}
    except urllib.error.HTTPError as e:
        err = e.read().decode(errors="replace")
        raise RuntimeError(f"HTTP {e.code} {url}: {err[:300]}") from e


def load_seen() -> set[str]:
    if not os.path.exists(STATE):
        os.makedirs(os.path.dirname(STATE) or ".", exist_ok=True)
        open(STATE, "a").close()
        return set()
    return {ln.strip() for ln in open(STATE) if ln.strip() and not ln.startswith("#")}


def mark_seen(eid: str, seen: set[str]) -> None:
    with open(STATE, "a") as f:
        f.write(eid + "\n")
    seen.add(eid)


def clear_seen(seen: set[str]) -> None:
    seen.clear()
    os.makedirs(os.path.dirname(STATE) or ".", exist_ok=True)
    open(STATE, "w").close()


def ensure_reader() -> str:
    """Seed V1 membership so we can ACL-read private events (grp_eng)."""
    out = post(
        f"{V1}/v1/tenants/{TENANT}/users",
        {
            "provider_user_id": READER_PROVIDER,
            "display_name": "Bridge Reader",
            "groups": ["grp_eng", "grp_default"],
        },
        timeout=10,
    )
    gid = out.get("global_user_id") or ""
    if not gid:
        raise RuntimeError(f"bridge reader seed failed: {out}")
    return gid


def wait_health(url: str, tries: int = 60) -> None:
    for _ in range(tries):
        try:
            get(f"{url}/healthz", timeout=3)
            return
        except Exception:
            time.sleep(1)
    raise RuntimeError(f"timeout waiting for {url}/healthz")


def v2_healthy() -> bool:
    try:
        get(f"{V2}/healthz", timeout=HEALTH_TIMEOUT)
        return True
    except Exception:
        return False


def v2_node_count() -> int | None:
    """Return node count or None if V2 unreachable."""
    try:
        stats = get(f"{V2}/v2/tenants/{TENANT}/stats", timeout=HEALTH_TIMEOUT + 1)
        return int(stats.get("nodes") or 0)
    except Exception:
        return None


def note_v2_down() -> None:
    global _V2_DOWN_SINCE, _V2_BACKOFF
    now = time.time()
    if _V2_DOWN_SINCE is None:
        _V2_DOWN_SINCE = now
        print("V2 unhealthy — pausing projections until /healthz recovers", flush=True)
    # Exponential backoff 2s → 60s
    if _V2_BACKOFF <= 0:
        _V2_BACKOFF = 2.0
    else:
        _V2_BACKOFF = min(60.0, _V2_BACKOFF * 1.5)


def note_v2_up() -> None:
    global _V2_DOWN_SINCE, _V2_BACKOFF
    if _V2_DOWN_SINCE is not None:
        dur = time.time() - _V2_DOWN_SINCE
        print(f"V2 healthy again (was down ~{dur:.0f}s) — resuming projections", flush=True)
    _V2_DOWN_SINCE = None
    _V2_BACKOFF = 0.0


def maybe_reproject_empty_graph(seen: set[str]) -> None:
    """If embedded V2 restarted empty but we already saw events, clear seen once."""
    global _LAST_EMPTY_CHECK
    now = time.time()
    if now - _LAST_EMPTY_CHECK < EMPTY_GRAPH_CHECK_SECS:
        return
    _LAST_EMPTY_CHECK = now
    if not seen:
        return
    if not v2_healthy():
        return
    n = v2_node_count()
    if n is None:
        return
    if n == 0:
        print(
            f"V2 graph empty (nodes=0) with {len(seen)} seen ids — "
            "clearing seen to re-project already-ingested signals",
            flush=True,
        )
        clear_seen(seen)
        _EVENT_FAILS.clear()


def is_bot_actor(actor: dict) -> bool:
    login = (actor.get("display_name") or "").strip().lower()
    pu = str(actor.get("provider_user_id") or "").strip().lower()
    if "[bot]" in login or login.endswith("bot") or login.endswith("[bot]"):
        return True
    if "bot" in login and login not in {"", "neel"}:
        return True
    if pu.endswith("[bot]"):
        return True
    return False


def slack_for_actor(actor: dict) -> str | None:
    """Resolve Slack user id for a canonical actor.

    Prefer explicit SLACK_USER_MAP keys (provider id, login, global id),
    merged with twin-api team admin map (multi-person).
    Only fall back to SLACK_TEST_USER_ID when the map is empty (single-dev demos).
    Never map bots.
    """
    if is_bot_actor(actor):
        return None
    refresh_team_map()
    gu = (actor.get("global_user_id") or "").strip()
    pu = str(actor.get("provider_user_id") or "").strip()
    login = (actor.get("display_name") or "").strip()
    for key in (gu, pu, login):
        if key and key in SLACK_MAP:
            return SLACK_MAP[key]
    if not SLACK_MAP and DEFAULT_SLACK:
        return DEFAULT_SLACK
    return None


def ensure_twin(actor: dict) -> None:
    if not TWIN:
        return
    gu = (actor.get("global_user_id") or "").strip()
    if not gu or is_bot_actor(actor):
        return
    slack = slack_for_actor(actor)
    if not slack:
        return
    name = (actor.get("display_name") or "").strip() or DEFAULT_NAME
    body = {
        "twin_kind": "person",
        "subject_id": gu,
        "display_name": name,
        "slack_user_id": slack,
    }
    if DEFAULT_CHANNEL:
        body["channel_id"] = DEFAULT_CHANNEL
    try:
        post(f"{TWIN}/v3/tenants/{TENANT}/twins", body, timeout=12)
        print(f"twin upsert subject={gu} slack={slack} name={name}", flush=True)
    except Exception as e:
        print(f"twin upsert fail subject={gu}: {e}", flush=True)


def project_event(ev: dict) -> None:
    actor = ev.get("actor") or {}
    gu = (actor.get("global_user_id") or "").strip() or "unknown"
    if gu and gu != "unknown":
        try:
            post(
                f"{V2}/v2/tenants/{TENANT}/users",
                {"global_user_id": gu, "groups": ["grp_eng"]},
                timeout=min(8.0, PROJECT_TIMEOUT),
            )
        except Exception:
            pass
    out = post(f"{V2}/v2/project", ev, timeout=PROJECT_TIMEOUT)
    print(
        f"ingest {ev.get('event_id')} {ev.get('event_type')} → V2 {out.get('status')} actor={gu}",
        flush=True,
    )
    ensure_twin(actor)


def main() -> None:
    print(
        f"bridge start tenant={TENANT} poll={POLL}s v1={V1} v2={V2} twin={TWIN} "
        f"slack_map={len(SLACK_MAP)} default_slack={'set' if DEFAULT_SLACK else 'none'} "
        f"max_per_tick={MAX_PER_TICK} project_timeout={PROJECT_TIMEOUT}s",
        flush=True,
    )
    wait_health(V1)
    wait_health(V2)
    try:
        wait_health(TWIN, tries=30)
    except Exception as e:
        print(f"twin not ready yet ({e}); will retry twin upserts later", flush=True)

    reader = ensure_reader()
    print(f"bridge reader global_user_id={reader}", flush=True)
    refresh_team_map(force=True)
    seen = load_seen()
    maybe_reproject_empty_graph(seen)
    # Force check immediately at boot
    global _LAST_EMPTY_CHECK
    _LAST_EMPTY_CHECK = 0.0
    maybe_reproject_empty_graph(seen)

    while True:
        try:
            # --- V2 health gate ---
            if not v2_healthy():
                note_v2_down()
                time.sleep(max(POLL, _V2_BACKOFF or 2.0))
                continue
            note_v2_up()

            # Embedded V2 wipe recovery
            maybe_reproject_empty_graph(seen)

            try:
                reader = ensure_reader()
            except Exception as e:
                print(f"reader seed warn: {e}", flush=True)
            refresh_team_map()

            data = get(
                f"{V1}/v1/tenants/{TENANT}/events?user_id={reader}&limit=100",
                timeout=20,
            )
            events = data.get("events") or []
            # Oldest first
            events = list(reversed(events))
            done = 0
            for ev in events:
                if done >= MAX_PER_TICK:
                    break
                eid = ev.get("event_id")
                if not eid or eid in seen:
                    continue
                # Poison skip: never stall the whole map on one bad event
                if _EVENT_FAILS[eid] >= MAX_EVENT_FAILURES:
                    print(
                        f"poison skip {eid} after {_EVENT_FAILS[eid]} failures "
                        f"type={ev.get('event_type')}",
                        flush=True,
                    )
                    mark_seen(eid, seen)
                    continue
                if not v2_healthy():
                    note_v2_down()
                    break
                try:
                    project_event(ev)
                    mark_seen(eid, seen)
                    _EVENT_FAILS.pop(eid, None)
                    done += 1
                    if PROJECT_PAUSE > 0:
                        time.sleep(PROJECT_PAUSE)
                except Exception as e:
                    _EVENT_FAILS[eid] += 1
                    print(
                        f"project fail {eid} (try {_EVENT_FAILS[eid]}/{MAX_EVENT_FAILURES}): {e}",
                        flush=True,
                    )
                    # V2 likely wedged or slow — back off; do not mark seen
                    note_v2_down()
                    time.sleep(max(POLL, _V2_BACKOFF or 5.0))
                    break
        except Exception as e:
            print(f"loop err: {e}", flush=True)
            time.sleep(max(POLL, 3.0))
            continue
        time.sleep(POLL)


if __name__ == "__main__":
    main()
