#!/usr/bin/env python3
"""Load test for sink-opensearch.

Several sink processes share one topic while instances join, crash and leave,
and a share of events is sent twice, the way clients retry. At the end it
checks that OpenSearch holds every unique event exactly once, that bad events
are in the DLQ and every offset is committed, and prints what happened: a
rebalance timeline and how many writes OpenSearch already had.

Needs the local stack and a release build:

    docker compose -f tests/docker-compose.yml up -d redpanda opensearch opensearch-init
    cargo build --release -p sink-opensearch
    python3 scripts/loadtest-opensearch-sink.py           # e.g. --events 300000 --partitions 12

It uses its own topics, consumer group and `events-lt-<run>-*` indexes, and
keeps them afterwards so you can look at the data; --clean-up deletes them.
It never touches anything it didn't create. Sink logs and CAC files stay in
target/loadtest/<run>/.
"""

import argparse
import json
import os
import random
import signal
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
KAFKA = "localhost:19092"
OPENSEARCH = "http://localhost:9200"
TENANTS = [f"tenant-{i:02d}" for i in range(20)]
BAD_TENANT = "Bad-Tenant"  # uppercase can't be part of an index name
BASE_PORT = 9480
CONSUMED = "sink_records_consumed_total"
CREATED = 'sink_docs_written_total{result="created"}'
ALREADY = 'sink_docs_written_total{result="already_written"}'


def rpk(*args, stdin=None):
    result = subprocess.run(
        ["docker", "exec", "-i", "redpanda", "rpk", *args],
        input=stdin,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"rpk {' '.join(args)}: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout


def opensearch(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(
        f"{OPENSEARCH}/{path}", data=data, method=method, headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def group_offsets(group):
    """Per-partition (committed, end) offsets for a consumer group."""
    offsets = {}
    for line in rpk("group", "describe", group).splitlines():
        fields = line.split()
        if len(fields) >= 6 and fields[1].isdigit():
            committed = int(fields[2]) if fields[2] != "-" else None
            offsets[int(fields[1])] = (committed, int(fields[4]))
    return offsets


def topic_size(topic):
    total = 0
    for line in rpk("topic", "describe", topic, "-p").splitlines():
        fields = line.split()
        if fields and fields[0].isdigit():
            total += int(fields[-1])
    return total


def dead_letter_sources(topic, count):
    """The (source partition, source offset) header pair of each dead letter."""
    if count == 0:
        return []
    output = rpk("topic", "consume", topic, "-n", str(count), "-f", "json")
    decoder, position, sources = json.JSONDecoder(), 0, []
    while True:
        while position < len(output) and output[position].isspace():
            position += 1
        if position >= len(output):
            return sources
        record, position = decoder.raw_decode(output, position)
        headers = {header["key"]: header["value"] for header in record.get("headers") or []}
        sources.append((headers.get("dlq.source.partition"), headers.get("dlq.source.offset")))


def make_events(count, bad_ratio, rng):
    start = datetime.now(timezone.utc) - timedelta(hours=1)
    events = []
    for i in range(count):
        bad = rng.random() < bad_ratio
        anon = f"anon-{rng.randrange(5000)}"
        event = {
            "envelop_version": "1.0",
            "id": str(uuid.UUID(int=rng.getrandbits(128), version=4)),
            "name": rng.choice(["page_viewed", "checkout_viewed", "payment_initiated"]),
            "tenant_id": BAD_TENANT if bad else rng.choice(TENANTS),
            "anon_id": anon,
            "occured_at": (start + timedelta(milliseconds=i)).isoformat().replace("+00:00", "Z"),
            # Properties change shape between events on purpose.
            "properties": rng.choice([{"amount": i}, {"amount": f"{i} INR"}, {"cart": {"items": i % 7}}, {}]),
        }
        events.append((anon, json.dumps(event, separators=(",", ":")), bad))
    return events


def plan_chunks(events, duplicate_ratio, chunk_count, rng):
    """Splits events into chunks sent over time. Resent copies of valid events
    go into the chunk after their original, like a client retrying late."""
    size = -(-len(events) // chunk_count)
    chunks = [[(key, value) for key, value, _ in events[i : i + size]] for i in range(0, len(events), size)]
    candidates = [i for i, (_, _, bad) in enumerate(events) if not bad]
    resent = rng.sample(candidates, int(len(candidates) * duplicate_ratio))
    for i in resent:
        key, value, _ = events[i]
        chunks[min(i // size + 1, len(chunks) - 1)].append((key, value))
    return chunks, len(resent)


def write_cac(path, names, port):
    path.write_text(
        f"""[default-configs]
"kafka.topics" = {{ value = ["{names['topic']}"], schema = {{ type = "array" }} }}
"kafka.group_id" = {{ value = "{names['group']}", schema = {{ type = "string" }} }}
"kafka.client_config" = {{ value = {{ "bootstrap.servers" = "{KAFKA}", "session.timeout.ms" = "10000", "heartbeat.interval.ms" = "2000" }}, schema = {{ type = "object" }} }}
"opensearch.url" = {{ value = "{OPENSEARCH}", schema = {{ type = "string" }} }}
"opensearch.index" = {{ value = "{names['index']}", schema = {{ type = "string" }} }}
"batch.linger_ms" = {{ value = 200, schema = {{ type = "integer" }} }}
"dlq.topic" = {{ value = "{names['dlq']}", schema = {{ type = "string" }} }}
"shutdown.grace_ms" = {{ value = 20000, schema = {{ type = "integer" }} }}
"server.addr" = {{ value = "127.0.0.1:{port}", schema = {{ type = "string" }} }}

[dimensions]
tenant_id = {{ position = 1, schema = {{ type = "string" }} }}
workspace_id = {{ position = 2, schema = {{ type = "string" }} }}
"""
    )


class Sink:
    """One sink process, with its last scraped metrics."""

    def __init__(self, name, binary, cac, port, log_path):
        self.name, self.binary, self.cac, self.port, self.log_path = name, binary, cac, port, log_path
        self.process = None
        self.metrics = {}

    def start(self):
        env = {"PATH": os.environ["PATH"], "HOME": os.environ.get("HOME", ""), "SINK_CONFIG": str(self.cac), "RUST_LOG": "info"}
        self.log = open(self.log_path, "w")
        self.process = subprocess.Popen([str(self.binary)], env=env, cwd=ROOT, stdout=self.log, stderr=subprocess.STDOUT)

    @property
    def running(self):
        return self.process is not None and self.process.poll() is None

    def scrape(self):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{self.port}/metrics", timeout=2) as response:
                text = response.read().decode()
        except (urllib.error.URLError, OSError):
            return
        metrics = {}
        for line in text.splitlines():
            if line.startswith("sink_") and " " in line:
                key, value = line.rsplit(" ", 1)
                metrics[key] = float(value)
        self.metrics = metrics

    def kill(self):
        self.scrape()
        self.process.send_signal(signal.SIGKILL)
        self.process.wait()

    def stop(self):
        self.scrape()
        self.process.send_signal(signal.SIGTERM)
        return self.process.wait(timeout=60)

    def total(self, prefix):
        return sum(value for key, value in self.metrics.items() if key.startswith(prefix))


def timeline(sinks, actions, t0):
    interesting = {
        "partitions assigned": "assigned {partitions} partitions",
        "partitions revoked": "revoked {partitions} partitions",
        "committed offsets on revoke": "committed on revoke ({partitions} partitions)",
        "committed final offsets": "committed final offsets ({partitions} partitions)",
        "stopped cleanly": "stopped cleanly",
    }
    rows = list(actions)
    for sink in sinks:
        for line in Path(sink.log_path).read_text().splitlines():
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            fields = entry.get("fields", {})
            template = interesting.get(fields.get("message"))
            if template:
                at = datetime.fromisoformat(entry["timestamp"].replace("Z", "+00:00")).timestamp()
                rows.append((at, sink.name, template.format(partitions=fields.get("partitions", "?"))))
    for at, who, what in sorted(rows):
        print(f"  {at - t0:6.1f}s  {who:<8} {what}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--events", type=int, default=100_000, help="unique events to send")
    parser.add_argument("--partitions", type=int, default=6)
    parser.add_argument("--duplicates", type=float, default=0.05, help="share of valid events sent twice")
    parser.add_argument("--bad", type=float, default=0.001, help="share of events with an invalid tenant")
    parser.add_argument("--pace", type=float, default=6.0, help="seconds between instance changes")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/sink-opensearch")
    parser.add_argument("--seed", type=int, default=None)
    parser.add_argument("--clean-up", action="store_true", help="delete this run's topics and indexes afterwards")
    args = parser.parse_args()

    if not args.binary.exists():
        sys.exit(f"{args.binary} not found; run: cargo build --release -p sink-opensearch")

    run = uuid.uuid4().hex[:8]
    rng = random.Random(args.seed)
    names = {
        "topic": f"lt-{run}",
        "dlq": f"lt-{run}.dlq",
        "group": f"lt-{run}",
        "index": f"events-lt-{run}-{{tenant_id}}",
    }
    workdir = ROOT / "target" / "loadtest" / run
    workdir.mkdir(parents=True, exist_ok=True)
    print(f"run {run}: {args.events:,} events, {args.partitions} partitions, workdir {workdir.relative_to(ROOT)}")

    rpk("topic", "create", names["topic"], "-p", str(args.partitions))
    rpk("topic", "create", names["dlq"], "-p", "1")

    events = make_events(args.events, args.bad, rng)
    chunks, resent = plan_chunks(events, args.duplicates, 60, rng)
    bad_count = sum(1 for _, _, bad in events if bad)
    sent = sum(len(chunk) for chunk in chunks)
    print(f"sending {sent:,} messages: {args.events - bad_count:,} valid unique, {resent:,} resent, {bad_count:,} with an invalid tenant\n")

    sinks = []
    for n in range(4):
        name = f"sink-{n + 1}"
        cac = workdir / f"{name}.toml"
        write_cac(cac, names, BASE_PORT + n)
        sinks.append(Sink(name, args.binary, cac, BASE_PORT + n, workdir / f"{name}.log"))

    t0 = time.time()
    actions = []

    def act(who, what):
        actions.append((time.time(), who, what))

    producer_error = []

    def produce():
        pause = args.pace * 6 / len(chunks)
        try:
            for chunk in chunks:
                rpk("topic", "produce", names["topic"], "-f", "%k %v\n", stdin="".join(f"{k} {v}\n" for k, v in chunk))
                time.sleep(pause)
            act("producer", "finished sending")
        except Exception as error:  # reported by the main thread
            producer_error.append(error)

    stop_scraping = threading.Event()

    def scrape_loop():
        while not stop_scraping.is_set():
            for sink in sinks:
                if sink.running:
                    sink.scrape()
            time.sleep(1)

    producer = threading.Thread(target=produce)
    scraper = threading.Thread(target=scrape_loop, daemon=True)
    exit_codes = {}
    try:
        sinks[0].start()
        act("sink-1", "started")
        producer.start()
        scraper.start()
        time.sleep(args.pace)
        sinks[1].start()
        act("sink-2", "started")
        time.sleep(args.pace)
        sinks[2].start()
        act("sink-3", "started")
        time.sleep(args.pace)
        act("sink-2", "KILLED (kill -9, like a crash)")
        sinks[1].kill()
        exit_codes["sink-2"] = "killed"
        time.sleep(args.pace * 2)  # the group notices after session.timeout.ms (10s)
        act("sink-3", "SIGTERM (graceful stop)")
        exit_codes["sink-3"] = sinks[2].stop()
        time.sleep(args.pace / 2)
        sinks[3].start()
        act("sink-4", "started")

        producer.join()
        if producer_error:
            raise producer_error[0]
        deadline = time.time() + 300
        while time.time() < deadline:
            try:
                offsets = group_offsets(names["group"])
            except RuntimeError:
                offsets = {}
            if len(offsets) == args.partitions and all(c == e for c, e in offsets.values() if e > 0):
                break
            time.sleep(1)
        act("group", "lag 0: every offset committed")
        drained_at = time.time()

        for sink in (sinks[0], sinks[3]):
            exit_codes[sink.name] = sink.stop()
    finally:
        stop_scraping.set()
        for sink in sinks:
            if sink.running:
                sink.process.kill()

    print("timeline")
    timeline(sinks, actions, t0)

    opensearch("POST", f"events-lt-{run}-*/_refresh")
    indexes = opensearch("GET", f"_cat/indices/events-lt-{run}-*?format=json&h=index,docs.count") or []
    docs = sum(int(index["docs.count"]) for index in indexes)
    dead = topic_size(names["dlq"])
    dead_unique = len(set(dead_letter_sources(names["dlq"], dead)))
    offsets = group_offsets(names["group"])
    uncommitted = sum(e - (c or 0) for c, e in offsets.values())
    consumed = sum(sink.total(CONSUMED) for sink in sinks)
    already = sum(sink.total(ALREADY) for sink in sinks)

    print("\nduplicates")
    print(f"  records read again after a partition moved: ~{max(0, consumed - sent):,.0f}")
    print("    (a moved partition's unwritten buffer is dropped and re-read by its new owner)")
    print(f"  writes OpenSearch already had (409, nothing duplicated): ~{already:,.0f}")
    print(f"    = {resent:,} resent by the 'client' + re-reads of records already written")
    print(f"  dead letters written twice: {dead - dead_unique:,} (the DLQ is at-least-once too; replaying it is still safe)")
    print("  (~: sink-2's last counters were scraped just before it was killed)")
    for sink in sinks:
        print(
            f"  {sink.name}: read {sink.total(CONSUMED):>9,.0f}   created {sink.total(CREATED):>9,.0f}"
            f"   already written {sink.total(ALREADY):>7,.0f}   exit {exit_codes.get(sink.name)}"
        )

    expected_docs = args.events - bad_count
    checks = [
        (f"OpenSearch holds each valid event exactly once: {docs:,} of {expected_docs:,}", docs == expected_docs),
        (f"one index per tenant: {len(indexes)} of {len(TENANTS)}", len(indexes) == len(TENANTS)),
        (f"each invalid-tenant event is in the DLQ: {dead_unique:,} of {bad_count:,}", dead_unique == bad_count),
        (f"every offset committed: {uncommitted:,} uncommitted across {len(offsets)} partitions",
         uncommitted == 0 and len(offsets) == args.partitions),
        ("graceful stops exited 0: " + ", ".join(f"{n}={exit_codes.get(n)}" for n in ("sink-1", "sink-3", "sink-4")),
         all(exit_codes.get(n) == 0 for n in ("sink-1", "sink-3", "sink-4"))),
    ]
    print(f"\ndrained {sent:,} messages in {drained_at - t0:.1f}s ({sent / (drained_at - t0):,.0f} msg/s, limited by the send rate)")
    print("\nchecks")
    for label, ok in checks:
        print(f"  {'PASS' if ok else 'FAIL'}  {label}")

    if args.clean_up:
        for index in indexes:
            opensearch("DELETE", index["index"])
        rpk("topic", "delete", names["topic"], names["dlq"])
        print("\ndeleted this run's topics and indexes")
    else:
        print(f"\nkept: topics {names['topic']} and {names['dlq']}, consumer group {names['group']}, indexes events-lt-{run}-*")
        print("to delete them:")
        print(f"  curl -X DELETE 'localhost:9200/events-lt-{run}-*'")
        print(f"  docker exec redpanda rpk topic delete {names['topic']} {names['dlq']}")
        print(f"  docker exec redpanda rpk group delete {names['group']}")
    print(f"logs: {workdir.relative_to(ROOT)}/sink-*.log")
    sys.exit(0 if all(ok for _, ok in checks) else 1)


if __name__ == "__main__":
    main()
