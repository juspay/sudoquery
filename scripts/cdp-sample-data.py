#!/usr/bin/env python3
"""Push sample customer data for one project into the CDP, then check it.

    scripts/cdp-sample-data.py <org_id> <project_id>

The project must already be provisioned (it has a CDP workspace and its
`events.{org_id}.{project_id}` topic). The script:

1. Declares the sample vocabulary on the organization's CDP tenant: four
   event types and eight metrics covering every metric kind (count, number
   sum, rolling window, timestamp, latest string, recent strings). Existing
   declarations are left alone.
2. If anything was new, waits for the CDP to pick it up (see "Waiting").
3. Produces events for five customers to the project's topic, shaped as
   events-collector publishes them:
   - anon:    an anonymous visitor (bare device id, no login).
   - signup:  browses anonymously, then signs up: the device joins the
              account.
   - buyer:   a repeat buyer on two devices with a mixed-case email, four
              orders (one 40 days ago, one delivered twice), six products
              viewed, and a plan change.
   - lapsed:  one order 60 days ago, nothing since.
   - phone:   identified by phone number only, one order.
   Plus an undeclared event, which the CDP skips.
4. Creates nine segments using every predicate form (comparisons, between,
   exists/missing, within_days/beyond_days, all/any/not).
5. Polls each customer's profile until its metrics and segments match what
   the events imply, and prints a pass/fail table.

Each run makes fresh customers (a random suffix on every identifier), so it
can be rerun against the same project. Segments are created once per
project and reused.

Waiting: the CDP caches its configuration. New metrics reach it within about
a minute, and events sent before that are never counted toward them. New
event types reach the ingestor when its cached tenant config expires (up to
about 5.5 minutes) unless the ingestor's admin endpoint is reachable to flush
it, which it is only on the CDP's own host (127.0.0.1:8086). See the CDP
repo's docs/known-bugs.md.

Talks to the CDP control plane directly as the platform gateway (the same
credentials dashboard-server uses) and to sudoquery's Redpanda. Needs
`kafka-python` and `requests`.
"""

import argparse
import json
import os
import sys
import time
import uuid
from datetime import datetime, timedelta, timezone

import requests
from kafka import KafkaConsumer, KafkaProducer

DEFAULT_CDP_URL = "http://localhost:8082/cdp/control-plane"
DEFAULT_CDP_TOKEN = "local-sudoquery-platform-token"
DEFAULT_BROKERS = "127.0.0.1:19092"
DEFAULT_INGESTOR_ADMIN = "http://127.0.0.1:8086"

METRIC_REFRESH = 65  # resolver + metric fold re-read definitions every 60s
INGEST_CONFIG_TTL = 335  # ingestor: 30s in process + 300s in Redis
VERIFY_TIMEOUT = 180  # resolve, merge, fold, then segment evaluation
POLL = 2

EVENT_TYPES = {
    "page_viewed": {},
    "product_viewed": {},
    "plan_changed": {},
    "order_completed": {
        "dedup_entity": "order",
        "dedup_key_paths": ["properties.order_id"],
    },
}

METRICS = [
    {"metric_name": "page_views", "event_name": "page_viewed",
     "value_kind": "count", "display_name": "Page views"},
    {"metric_name": "last_seen_at", "event_name": "page_viewed",
     "value_kind": "occurred_at_epoch", "display_name": "Last seen"},
    {"metric_name": "recent_products", "event_name": "product_viewed",
     "value_kind": "property_string", "value_path": "properties.product",
     "aggregation": "recent", "keep": 5, "display_name": "Recently viewed"},
    {"metric_name": "plan", "event_name": "plan_changed",
     "value_kind": "property_string", "value_path": "properties.plan",
     "aggregation": "latest", "display_name": "Plan"},
    {"metric_name": "order_count", "event_name": "order_completed",
     "value_kind": "count", "display_name": "Orders"},
    {"metric_name": "total_spend", "event_name": "order_completed",
     "value_kind": "property_number", "value_path": "properties.amount",
     "display_name": "Total spend"},
    {"metric_name": "spend_30d", "event_name": "order_completed",
     "value_kind": "property_number", "value_path": "properties.amount",
     "window_days": 30, "display_name": "Spend, last 30 days"},
    {"metric_name": "last_order_at", "event_name": "order_completed",
     "value_kind": "occurred_at_epoch", "display_name": "Last order"},
]

# id → (display name, predicate)
SEGMENTS = {
    "big-spenders": ("Big spenders",
                     {"metric": "total_spend", "op": "gte", "value": 500}),
    "repeat-buyers": ("Repeat buyers",
                      {"metric": "order_count", "op": "gte", "value": 2}),
    "mid-spenders": ("Mid spenders",
                     {"metric": "total_spend", "op": "between", "value": [100, 499]}),
    "lapsed-buyers": ("Lapsed buyers",
                      {"metric": "last_order_at", "op": "beyond_days", "value": 30}),
    "active-7d": ("Active this week",
                  {"metric": "last_seen_at", "op": "within_days", "value": 7}),
    "window-shoppers": ("Window shoppers", {"all": [
        {"metric": "page_views", "op": "gte", "value": 1},
        {"metric": "order_count", "op": "missing"},
    ]}),
    "plan-holders": ("On a plan", {"metric": "plan", "op": "exists"}),
    "vip": ("VIP", {"any": [
        {"metric": "total_spend", "op": "gte", "value": 1000},
        {"all": [
            {"metric": "order_count", "op": "gte", "value": 3},
            {"metric": "last_order_at", "op": "within_days", "value": 7},
        ]},
    ]}),
    "engaged-non-buyers": ("Engaged, never bought", {"all": [
        {"metric": "page_views", "op": "gte", "value": 2},
        {"not": {"metric": "order_count", "op": "exists"}},
    ]}),
}


def ago(days=0, minutes=0):
    t = datetime.now(timezone.utc) - timedelta(days=days, minutes=minutes)
    return t.isoformat(timespec="milliseconds").replace("+00:00", "Z")


class Cdp:
    """The control plane, as the platform gateway, acting as one tenant."""

    def __init__(self, url, token, tenant):
        self.base = f"{url.rstrip('/')}/v1/tenants/{tenant}"
        self.url = url.rstrip("/")
        self.session = requests.Session()
        self.session.headers.update({
            "Authorization": f"Bearer {token}",
            "X-CDP-Tenant": tenant,
        })

    def request(self, method, path, **kwargs):
        response = self.session.request(method, self.base + path, timeout=10, **kwargs)
        try:
            body = response.json()
        except ValueError:
            body = response.text
        return response.status_code, body

    def must(self, method, path, **kwargs):
        status, body = self.request(method, path, **kwargs)
        if status >= 300:
            sys.exit(f"{method} {path}: HTTP {status}: {body}")
        return body


def declare_vocabulary(cdp):
    """Declares what's missing; returns (new event types?, new metrics?)."""
    existing = {e["event_name"]: e for e in cdp.must("GET", "/event-types")["event_types"]}
    new_events = False
    for name, body in EVENT_TYPES.items():
        current = existing.get(name)
        if current and current.get("status") == "active" and \
                current.get("dedup_key_paths") == body.get("dedup_key_paths"):
            continue
        cdp.must("PUT", f"/event-types/{name}", json=body)
        print(f"  declared event type {name}")
        new_events = True

    defined = {m["metric_name"] for m in cdp.must("GET", "/metric-definitions")["metric_definitions"]}
    new_metrics = False
    for metric in METRICS:
        if metric["metric_name"] in defined:
            continue
        cdp.must("PUT", "/metric-definitions", json={"workspace_id": "*", **metric})
        print(f"  defined metric {metric['metric_name']}")
        new_metrics = True
    return new_events, new_metrics


def flush_ingestor_config(admin_url, tenant):
    try:
        response = requests.post(f"{admin_url}/internal/config/invalidate",
                                 json={"tenant_id": tenant}, timeout=3)
        return response.ok
    except requests.RequestException:
        return False


def wait(seconds, why):
    print(f"waiting {seconds}s: {why}")
    end = time.monotonic() + seconds
    while (left := end - time.monotonic()) > 0:
        print(f"  {int(left):3d}s left", end="\r", flush=True)
        time.sleep(min(5, left))
    print(" " * 20, end="\r")


def create_segments(cdp, project_id):
    """Creates the sample segments in this project; returns name → segment id.

    Segment ids are per organization and segments are immutable: an existing
    one in this project is reused, and one another project took gets a
    project-suffixed id."""
    ids = {}
    for name, (display_name, predicate) in SEGMENTS.items():
        for segment_id in (name, f"{name}-{project_id}"):
            status, body = cdp.request("PUT", f"/segments/{segment_id}", json={
                "workspace_id": project_id,
                "display_name": display_name,
                "predicate": predicate,
            })
            if status < 300:
                print(f"  created segment {segment_id}")
                break
            if status != 409:
                sys.exit(f"creating segment {segment_id}: HTTP {status}: {body}")
            status, existing = cdp.request("GET", f"/segments/{segment_id}/definition",
                                           params={"workspace_id": project_id})
            if status == 200 and existing.get("workspace_id", project_id) == project_id:
                break
        else:
            sys.exit(f"segment ids {name} and {name}-{project_id} are both taken")
        ids[name] = segment_id
    return ids


class Feed:
    """Events for one project, shaped as events-collector publishes them."""

    def __init__(self, org_id, project_id):
        self.org_id, self.project_id = org_id, project_id
        self.messages = []  # (key, bytes)

    def event(self, name, device, when, properties=None, identifiers=None):
        properties = dict(properties or {})
        event = {
            "envelop_version": "1.0",
            "id": str(uuid.uuid4()),
            "name": name,
            "org_id": self.org_id,
            "project_id": self.project_id,
            "anon_id": device,
            "occured_at": when,
            "properties": properties,
        }
        if identifiers:
            # The collector's bearer-token route: a logged-in customer.
            properties["identifiers"] = identifiers
            event["authenticated"] = True
        self.add(device, event)
        return event

    def add(self, key, event):
        self.messages.append((key, json.dumps(event).encode()))


def build_customers(feed, run):
    """Queues every customer's events; returns what each should end up as."""
    customers = {}

    # anon: three pages and two products, never logs in.
    device = f"anon-{run}"
    for i, path in enumerate(["/", "/shoes", "/shoes/runner"]):
        feed.event("page_viewed", device, ago(minutes=30 - i), {"path": path})
    for i, product in enumerate(["runner-blue", "runner-red"]):
        feed.event("product_viewed", device, ago(minutes=21 - i), {"product": product})
    # Not in the vocabulary: skipped by the CDP.
    feed.event("button_clicked", device, ago(minutes=19), {"button": "add-to-cart"})
    customers["anon"] = {
        "lookup": ("cookie", device),
        "same_profile": [],
        "metrics": {"page_views": "3", "order_count": None},
        "recent_products": ["runner-red", "runner-blue"],
        "plan": None,
        "in": {"active-7d", "window-shoppers", "engaged-non-buyers"},
    }

    # signup: two anonymous pages, then signs up on the same device.
    device = f"signup-dev-{run}"
    email = f"signup-{run}@example.com"
    feed.event("page_viewed", device, ago(minutes=50), {"path": "/"})
    feed.event("page_viewed", device, ago(minutes=49), {"path": "/pricing"})
    feed.event("plan_changed", device, ago(minutes=48), {"plan": "free"},
               {"user_id": f"u-signup-{run}", "email": email, "cookie": device})
    customers["signup"] = {
        "lookup": ("email", email),
        "same_profile": [("cookie", device), ("user_id", f"u-signup-{run}")],
        "metrics": {"page_views": "2", "order_count": None},
        "recent_products": None,
        "plan": "free",
        "in": {"active-7d", "window-shoppers", "plan-holders", "engaged-non-buyers"},
    }

    # buyer: laptop + phone, mixed-case email, four orders (one twice).
    laptop, phone = f"buyer-laptop-{run}", f"buyer-phone-{run}"
    email = f"buyer-{run}@example.com"
    shouting = f"  Buyer-{run}@Example.COM "
    ids_laptop = {"user_id": f"u-buyer-{run}", "email": email, "cookie": laptop}
    ids_phone = {"email": shouting, "cookie": phone}
    feed.event("plan_changed", laptop, ago(days=45), {"plan": "starter"}, ids_laptop)
    feed.event("order_completed", laptop, ago(days=40),
               {"order_id": f"B1-{run}", "amount": 300}, ids_laptop)
    second = feed.event("order_completed", laptop, ago(days=5),
                        {"order_id": f"B2-{run}", "amount": 450}, ids_laptop)
    feed.add(laptop, second)  # redelivered as-is: counted once
    for i, product in enumerate(["tee", "cap", "socks", "jacket", "bag", "watch"]):
        feed.event("product_viewed", phone, ago(minutes=40 - i), {"product": product}, ids_phone)
    feed.event("page_viewed", phone, ago(minutes=30), {"path": "/checkout"}, ids_phone)
    feed.event("order_completed", phone, ago(minutes=29),
               {"order_id": f"B3-{run}", "amount": 250}, ids_phone)
    feed.event("plan_changed", laptop, ago(minutes=10), {"plan": "pro"}, ids_laptop)
    customers["buyer"] = {
        "lookup": ("email", email),
        "same_profile": [("cookie", laptop), ("cookie", phone), ("user_id", f"u-buyer-{run}")],
        "metrics": {"order_count": "3", "total_spend": "1000", "spend_30d": "700",
                    "page_views": "1"},
        "recent_products": ["watch", "bag", "jacket", "socks", "cap"],
        "plan": "pro",
        "in": {"big-spenders", "repeat-buyers", "vip", "active-7d", "plan-holders"},
    }

    # lapsed: one order and one visit, 60 days ago.
    device = f"lapsed-dev-{run}"
    email = f"lapsed-{run}@example.com"
    ids = {"email": email, "cookie": device}
    feed.event("page_viewed", device, ago(days=60, minutes=5), {"path": "/"}, ids)
    feed.event("order_completed", device, ago(days=60),
               {"order_id": f"L1-{run}", "amount": 200}, ids)
    customers["lapsed"] = {
        "lookup": ("email", email),
        "same_profile": [("cookie", device)],
        "metrics": {"order_count": "1", "total_spend": "200", "page_views": "1"},
        "recent_products": None,
        "plan": None,
        "in": {"mid-spenders", "lapsed-buyers"},
    }

    # phone: known only by phone number, one order.
    device = f"phone-dev-{run}"
    number = "+9198" + str(uuid.uuid4().int)[:8]
    feed.event("order_completed", device, ago(minutes=15),
               {"order_id": f"P1-{run}", "amount": 120}, {"phone": number, "cookie": device})
    customers["phone"] = {
        "lookup": ("phone", number),
        "same_profile": [("cookie", device)],
        "metrics": {"order_count": "1", "total_spend": "120", "spend_30d": "120",
                    "page_views": None},
        "recent_products": None,
        "plan": None,
        "in": {"mid-spenders"},
    }
    return customers


def produce(brokers, topic, messages):
    consumer = KafkaConsumer(bootstrap_servers=brokers)
    exists = topic in consumer.topics()
    consumer.close()
    if not exists:
        sys.exit(f"topic {topic} doesn't exist on {brokers}: is the project provisioned?")
    producer = KafkaProducer(bootstrap_servers=brokers)
    for key, value in messages:
        producer.send(topic, key=key.encode(), value=value)
    producer.flush()
    producer.close()


def metric(profile, name):
    for m in profile.get("metrics", []):
        if m["metric"] == name:
            return m
    return None


def number(m):
    """A metric's value as a canonical decimal string ("1000.0" → "1000")."""
    if m is None or m.get("value") is None:
        return None
    text = str(m["value"])
    return text.rstrip("0").rstrip(".") if "." in text else text


def check(cdp, project_id, expected, segment_ids):
    """Problems with one customer right now (empty = all good)."""
    kind, value = expected["lookup"]
    ws = f"/workspaces/{project_id}/profiles/lookup"
    status, profile = cdp.request("GET", ws, params={"type": kind, "value": value})
    if status != 200:
        return [f"no profile for {kind}={value} (HTTP {status})"]
    problems = []
    for other_kind, other in expected["same_profile"]:
        status, body = cdp.request("GET", ws, params={"type": other_kind, "value": other})
        if status != 200 or body.get("profile_id") != profile["profile_id"]:
            problems.append(f"{other_kind}={other} isn't on this profile")
    for name, want in expected["metrics"].items():
        got = number(metric(profile, name))
        if got != want:
            problems.append(f"{name} is {got}, expected {want}")
    if expected["recent_products"] is not None:
        m = metric(profile, "recent_products")
        got = [i["value"] for i in (m or {}).get("items", [])]
        if got != expected["recent_products"]:
            problems.append(f"recent_products is {got}, expected {expected['recent_products']}")
    got_plan = (metric(profile, "plan") or {}).get("text")
    if got_plan != expected["plan"]:
        problems.append(f"plan is {got_plan}, expected {expected['plan']}")
    entered = {s["segment_id"] for s in profile.get("segments", []) if s.get("state") == "entered"}
    for name, segment_id in segment_ids.items():
        should = name in expected["in"]
        if should and segment_id not in entered:
            problems.append(f"not in segment {name}")
        elif not should and segment_id in entered:
            problems.append(f"unexpectedly in segment {name}")
    return problems


def main():
    parser = argparse.ArgumentParser(
        description=__doc__.split("\n\n")[0],
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("org_id")
    parser.add_argument("project_id")
    parser.add_argument("--cdp-url", default=os.environ.get("CDP_CONTROL_PLANE_URL") or DEFAULT_CDP_URL,
                        help="CDP control plane (env CDP_CONTROL_PLANE_URL)")
    parser.add_argument("--cdp-token", default=os.environ.get("CDP_PLATFORM_TOKEN") or DEFAULT_CDP_TOKEN,
                        help="platform token (env CDP_PLATFORM_TOKEN)")
    parser.add_argument("--brokers", default=os.environ.get("KAFKA_BROKER_URL") or DEFAULT_BROKERS,
                        help="sudoquery's Redpanda (env KAFKA_BROKER_URL)")
    parser.add_argument("--ingestor-admin", default=DEFAULT_INGESTOR_ADMIN,
                        help="CDP ingestor admin API, to flush its tenant config")
    parser.add_argument("--no-wait", action="store_true",
                        help="don't wait for new declarations to reach the CDP")
    parser.add_argument("--no-verify", action="store_true",
                        help="push the data and stop")
    args = parser.parse_args()
    org_id, project_id = args.org_id, args.project_id

    cdp = Cdp(args.cdp_url, args.cdp_token, org_id)
    status, body = cdp.request("GET", "/workspaces")
    if status != 200:
        sys.exit(f"CDP tenant {org_id} at {args.cdp_url}: HTTP {status}: {body}")
    if project_id not in [w["workspace_id"] for w in body["workspaces"]]:
        sys.exit(f"project {project_id} has no CDP workspace: provision it first")

    print(f"vocabulary for {org_id}")
    new_events, new_metrics = declare_vocabulary(cdp)
    delay = 0
    if new_events and flush_ingestor_config(args.ingestor_admin, org_id):
        print("  flushed the ingestor's cached config")
    elif new_events:
        delay = INGEST_CONFIG_TTL
        why = "new event types reach the ingestor when its cached config expires"
    if new_metrics and delay < METRIC_REFRESH:
        delay = METRIC_REFRESH
        why = "new metrics reach the CDP within a minute"
    if delay and not args.no_wait:
        wait(delay, why)

    print(f"segments for {project_id}")
    segment_ids = create_segments(cdp, project_id)

    run = uuid.uuid4().hex[:6]
    topic = f"events.{org_id}.{project_id}"
    feed = Feed(org_id, project_id)
    customers = build_customers(feed, run)
    produce(args.brokers, topic, feed.messages)
    print(f"produced {len(feed.messages)} events to {topic} (run {run})")
    for name, expected in customers.items():
        kind, value = expected["lookup"]
        print(f"  {name:7s} {kind}={value}")

    if args.no_verify:
        return
    print(f"checking (up to {VERIFY_TIMEOUT}s)")
    deadline = time.monotonic() + VERIFY_TIMEOUT
    pending = dict(customers)
    problems = {}
    while pending:
        for name in list(pending):
            problems[name] = check(cdp, project_id, pending[name], segment_ids)
            if not problems[name]:
                print(f"  ok    {name}")
                del pending[name]
        if not pending or time.monotonic() > deadline:
            break
        time.sleep(POLL)
    for name in pending:
        print(f"  FAIL  {name}")
        for problem in problems[name]:
            print(f"          {problem}")
    sys.exit(1 if pending else 0)


if __name__ == "__main__":
    main()
