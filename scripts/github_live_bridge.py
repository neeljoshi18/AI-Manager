#!/usr/bin/env python3
"""
Always-on ingest bridge: V1 events → V2 graph → ensure person twins.

Does NOT Slack-DM. Status delivery is owned by twin-api's scheduled notify
loop (STATUS_WINDOW / NOTIFY_INTERVAL) so high-volume GitHub webhooks never
spam developers (ADR-014).

Reads events via a seeded ACL reader (grp_eng) so private-repo exhaust is
visible. Upserts twins + Slack map for known actors when SLACK_USER_MAP /
SLACK_TEST_USER_ID are set.
"""
from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request

V1 = os.environ.get("V1_BASE_URL", "http://127.0.0.1:18080").rstrip("/")
V2 = os.environ.get("V2_BASE_URL", "http://127.0.0.1:18082").rstrip("/")
TWIN = os.environ.get("TWIN_BASE_URL", "http://127.0.0.1:18083").rstrip("/")
TENANT = os.environ.get("TENANT_ID", "ten_github")
STATE = os.environ.get("STATE_FILE", f"/tmp/ai_manager_bridge_seen_{TENANT}.txt")
POLL = float(os.environ.get("POLL_SECS", "5"))
# Cap work per tick so embedded graph-api is not flooded (avoids hangs on small VPS).
MAX_PER_TICK = int(os.environ.get("BRIDGE_MAX_PER_TICK", "3"))
PROJECT_PAUSE = float(os.environ.get("BRIDGE_PROJECT_PAUSE_SECS", "0.4"))
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


def ensure_reader() -> str:
    """Seed V1 membership so we can ACL-read private events (grp_eng)."""
    out = post(
        f"{V1}/v1/tenants/{TENANT}/users",
        {
            "provider_user_id": READER_PROVIDER,
            "display_name": "Bridge Reader",
            "groups": ["grp_eng", "grp_default"],
        },
    )
    gid = out.get("global_user_id") or ""
    if not gid:
        raise RuntimeError(f"bridge reader seed failed: {out}")
    return gid


def wait_health(url: str, tries: int = 60) -> None:
    for i in range(tries):
        try:
            get(f"{url}/healthz", timeout=3)
            return
        except Exception:
            time.sleep(1)
    raise RuntimeError(f"timeout waiting for {url}/healthz")


def is_bot_actor(actor: dict) -> bool:
    login = (actor.get("display_name") or "").strip().lower()
    pu = str(actor.get("provider_user_id") or "").strip().lower()
    if "[bot]" in login or login.endswith("bot") or login.endswith("[bot]"):
        return True
    if "bot" in login and login not in {"", "neel"}:
        # vercel[bot], dependabot, etc.
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
        post(f"{TWIN}/v3/tenants/{TENANT}/twins", body, timeout=15)
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
                timeout=10,
            )
        except Exception:
            pass
    out = post(f"{V2}/v2/project", ev, timeout=25)
    print(
        f"ingest {ev.get('event_id')} {ev.get('event_type')} → V2 {out.get('status')} actor={gu}",
        flush=True,
    )
    ensure_twin(actor)


def main() -> None:
    print(
        f"bridge start tenant={TENANT} poll={POLL}s v1={V1} v2={V2} twin={TWIN} "
        f"slack_map={len(SLACK_MAP)} default_slack={'set' if DEFAULT_SLACK else 'none'}",
        flush=True,
    )
    wait_health(V1)
    wait_health(V2)
    # twin optional at start (may boot later)
    try:
        wait_health(TWIN, tries=30)
    except Exception as e:
        print(f"twin not ready yet ({e}); will retry twin upserts later", flush=True)

    reader = ensure_reader()
    print(f"bridge reader global_user_id={reader}", flush=True)
    refresh_team_map(force=True)
    seen = load_seen()

    # Embedded V2 loses graph on container recreate; replay V1 exhaust so Graph UI is full.
    # Only wipe once per process start when the graph is empty (not when only unmapped
    # event types were seen — those leave nodes=0 but still need re-projection of PRs).
    try:
        stats = get(f"{V2}/v2/tenants/{TENANT}/stats", timeout=10)
        v2_nodes = int(stats.get("nodes") or 0)
        if v2_nodes == 0 and seen:
            print(
                f"V2 graph empty (nodes=0) but bridge has {len(seen)} seen ids — "
                "clearing seen state to re-project already-ingested signals",
                flush=True,
            )
            seen = set()
            open(STATE, "w").close()
    except Exception as e:
        print(f"v2 stats check warn: {e}", flush=True)

    while True:
        try:
            # Re-seed reader periodically (embedded V1 loses membership on restart)
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
            # Process oldest first so graph order is sensible
            events = list(reversed(events))
            done = 0
            for ev in events:
                if done >= MAX_PER_TICK:
                    break
                eid = ev.get("event_id")
                if not eid or eid in seen:
                    continue
                try:
                    project_event(ev)
                    mark_seen(eid, seen)
                    done += 1
                    if PROJECT_PAUSE > 0:
                        time.sleep(PROJECT_PAUSE)
                except Exception as e:
                    print(f"project fail {eid}: {e}", flush=True)
                    # do not mark seen — retry next loop; back off hard if V2 wedged
                    time.sleep(max(POLL, 5.0))
                    break
        except Exception as e:
            print(f"loop err: {e}", flush=True)
        time.sleep(POLL)


if __name__ == "__main__":
    main()
