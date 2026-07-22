#!/usr/bin/env python3
"""
Ingest bridge only: V1 events → V2 graph.

Does NOT compile or Slack-DM. Status delivery is owned by twin-api's
scheduled notify loop (STATUS_WINDOW / NOTIFY_INTERVAL) so high-volume
GitHub webhooks never spam developers.
"""
import json
import os
import time
import urllib.request

V1 = os.environ.get("V1_BASE_URL", "http://127.0.0.1:18080")
V2 = os.environ.get("V2_BASE_URL", "http://127.0.0.1:18082")
TENANT = os.environ.get("TENANT_ID", "ten_github")
STATE = os.environ.get("STATE_FILE", f"/tmp/ai_manager_bridge_seen_{TENANT}.txt")
POLL = float(os.environ.get("POLL_SECS", "4"))
WATCH = set(filter(None, os.environ.get("WATCH_USERS", "").split(",")))


def get(url):
    with urllib.request.urlopen(url, timeout=10) as r:
        return json.loads(r.read().decode())


def post(url, obj):
    req = urllib.request.Request(
        url,
        data=json.dumps(obj).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read().decode())


def load_state():
    if not os.path.exists(STATE):
        open(STATE, "a").close()
        return set(), set()
    lines = open(STATE).read().splitlines()
    seen = {ln for ln in lines if not ln.startswith("user:")}
    users = {ln.replace("user:", "") for ln in lines if ln.startswith("user:")}
    return seen, users


def mark(eid, actor, seen):
    with open(STATE, "a") as f:
        f.write(eid + "\n")
        f.write("user:" + actor + "\n")
    seen.add(eid)


def main():
    seen, users = load_state()
    users |= WATCH
    seeds = {
        "gu_d11c3177-d61b-4dd3-8fd9-e8881397895d",
        "gu_9cf2a501-b7d0-41ea-9c72-bf0f8364d1eb",
        "gu_alice",
    }
    print(
        f"bridge (ingest-only) tenant={TENANT} poll={POLL}s — no Slack; twin-api schedules DMs",
        flush=True,
    )

    while True:
        try:
            for seed in list(users | seeds):
                try:
                    data = get(f"{V1}/v1/tenants/{TENANT}/events?user_id={seed}&limit=50")
                except Exception:
                    continue
                for ev in data.get("events") or []:
                    eid = ev.get("event_id")
                    if not eid or eid in seen:
                        continue
                    actor = (ev.get("actor") or {}).get("global_user_id") or seed
                    users.add(actor)
                    try:
                        post(
                            f"{V2}/v2/tenants/{TENANT}/users",
                            {"global_user_id": actor, "groups": ["grp_eng"]},
                        )
                    except Exception:
                        pass
                    try:
                        out = post(f"{V2}/v2/project", ev)
                        print(
                            f"ingest {eid} → V2 {out.get('status')} actor={actor}",
                            flush=True,
                        )
                    except Exception as e:
                        print(f"project fail {eid}: {e}", flush=True)
                        continue
                    mark(eid, actor, seen)
        except Exception as e:
            print("loop err", e, flush=True)
        time.sleep(POLL)


if __name__ == "__main__":
    main()
