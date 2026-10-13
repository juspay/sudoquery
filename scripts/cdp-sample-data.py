#!/usr/bin/env python3
"""Push demo customer events for one project to Kafka, for the Customers UI.

    scripts/cdp-sample-data.py <org_id> <project_id> [--customers N] [--brokers HOST:PORT]

Writes to the project's topic, events.{org_id}.{project_id}, in the shape
events-collector publishes. The project must already exist.

The CDP only counts events and metrics declared for the organization (in the
Customers setup page). Declare them before pushing; events sent within about
a minute of declaring are not counted:

  Event types
    page_viewed
    product_viewed
    plan_changed
    order_completed    dedup on properties.order_id

  Metrics
    page_views       count of page_viewed
    last_seen_at     time of page_viewed
    recent_products  recent values of properties.product  (product_viewed)
    plan             latest value of properties.plan      (plan_changed)
    order_count      count of order_completed
    total_spend      sum of properties.amount             (order_completed)
    last_order_at    time of order_completed

Customers are a mix of anonymous visitors, signed-up browsers, buyers
(some on two devices), and lapsed buyers whose last order is months old.
Every run adds new customers. Needs `kafka-python`.
"""

import argparse
import json
import random
import uuid
from datetime import datetime, timedelta, timezone

from kafka import KafkaProducer

FIRST = ["Aarav", "Priya", "Rohan", "Ananya", "Vikram", "Meera", "Arjun", "Diya",
         "Kabir", "Isha", "Nikhil", "Sara", "Dev", "Tara", "Karan", "Riya"]
LAST = ["Shah", "Iyer", "Reddy", "Kapoor", "Nair", "Mehta", "Rao", "Das"]
PRODUCTS = ["runner-shoes", "trail-shoes", "tee", "hoodie", "cap", "socks",
            "backpack", "water-bottle", "watch", "jacket"]
PAGES = ["/", "/shoes", "/apparel", "/sale", "/pricing", "/cart"]
PLANS = ["free", "starter", "pro"]

NOW = datetime.now(timezone.utc)


def at(days_ago):
    t = NOW - timedelta(days=days_ago)
    return t.isoformat(timespec="milliseconds").replace("+00:00", "Z")


class Feed:
    def __init__(self, org_id, project_id):
        self.org_id, self.project_id = org_id, project_id
        self.messages = []

    def event(self, name, device, days_ago, properties, identifiers=None):
        properties = dict(properties)
        event = {
            "envelop_version": "1.0",
            "id": str(uuid.uuid4()),
            "name": name,
            "org_id": self.org_id,
            "project_id": self.project_id,
            "anon_id": device,
            "occured_at": at(days_ago),
            "properties": properties,
        }
        if identifiers:
            # A logged-in customer, as the collector's bearer-token route
            # publishes it.
            properties["identifiers"] = identifiers
            event["authenticated"] = True
        self.messages.append((days_ago, device, event))


def browse(feed, device, start, visits, identifiers=None):
    """`visits` visits spread over the `start` days before now."""
    for _ in range(visits):
        day = random.uniform(0.01, start)
        for page in random.sample(PAGES, random.randint(1, 3)):
            feed.event("page_viewed", device, day, {"path": page}, identifiers)
        if random.random() < 0.7:
            feed.event("product_viewed", device, day - 0.001,
                       {"product": random.choice(PRODUCTS)}, identifiers)


def customer(feed, run, i):
    first, last = random.choice(FIRST), random.choice(LAST)
    handle = f"{first}.{last}.{run}{i}".lower()
    device = f"device-{run}-{i}"
    ids = {"user_id": f"user-{run}-{i}", "email": f"{handle}@example.com", "cookie": device}
    if random.random() < 0.3:
        ids["phone"] = "+9198" + "".join(random.choices("0123456789", k=8))

    kind = random.choices(["anonymous", "browser", "buyer", "lapsed"], [3, 2, 4, 1])[0]
    if kind == "anonymous":
        browse(feed, device, 14, random.randint(1, 4))
        return
    # Browses anonymously first, then signs up on the same device.
    signup = random.uniform(60, 120) if kind == "lapsed" else random.uniform(1, 60)
    browse(feed, device, signup, random.randint(1, 2))
    feed.event("plan_changed", device, signup, {"plan": "free"}, ids)
    if kind == "browser":
        browse(feed, device, signup, random.randint(1, 5), ids)
        return

    if random.random() < 0.4:
        feed.event("plan_changed", device, signup / 2, {"plan": random.choice(PLANS[1:])}, ids)
    # Some buyers also use a second device, logged in with the same email.
    devices = [device]
    if random.random() < 0.4:
        devices.append(f"device-{run}-{i}-b")
    # A lapsed customer's last order is at least 45 days old.
    earliest = 45 if kind == "lapsed" else 0.01
    for n in range(random.randint(1, 2) if kind == "lapsed" else random.randint(1, 6)):
        on = random.choice(devices)
        id_set = {**ids, "cookie": on}
        day = random.uniform(earliest, signup)
        product = random.choice(PRODUCTS)
        feed.event("product_viewed", on, day + 0.001, {"product": product}, id_set)
        feed.event("order_completed", on, day,
                   {"order_id": f"order-{run}-{i}-{n}", "amount": random.randrange(299, 4999, 100),
                    "currency": "INR", "product": product}, id_set)
    if kind == "buyer":
        browse(feed, random.choice(devices), 7, random.randint(1, 3), ids)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("org_id")
    parser.add_argument("project_id")
    parser.add_argument("--customers", type=int, default=25)
    parser.add_argument("--brokers", default="127.0.0.1:19092", help="sudoquery's Redpanda")
    args = parser.parse_args()

    run = uuid.uuid4().hex[:4]
    feed = Feed(args.org_id, args.project_id)
    for i in range(args.customers):
        customer(feed, run, i)

    topic = f"events.{args.org_id}.{args.project_id}"
    producer = KafkaProducer(bootstrap_servers=args.brokers)
    # Oldest first, so each customer's history arrives in order.
    for _, device, event in sorted(feed.messages, key=lambda m: -m[0]):
        producer.send(topic, key=device.encode(), value=json.dumps(event).encode())
    producer.flush()
    print(f"pushed {len(feed.messages)} events for {args.customers} customers to {topic}")


if __name__ == "__main__":
    main()
