use serde_json::{Value, json};

struct ChartAxisConfig {
    x_axis: Option<&'static str>,
    y_axis: Option<&'static str>,
    label_axis: Option<&'static str>,
    value_axis: Option<&'static str>,
}

struct ChartConfig {
    bar_chart: Option<ChartAxisConfig>,
    line_chart: Option<ChartAxisConfig>,
    pie_chart: Option<ChartAxisConfig>,
    funnel_chart: Option<ChartAxisConfig>,
    sankey_chart: Option<ChartAxisConfig>,
}

struct GoldenQuery {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    use_case: &'static str,
    query: &'static str,
    tags: &'static [&'static str],
    chart_config: Option<ChartConfig>,
}

fn chart_axis_to_json(c: &ChartAxisConfig) -> Value {
    let mut v = json!({});
    if let Some(x) = c.x_axis {
        v["xAxis"] = json!(x);
    }
    if let Some(y) = c.y_axis {
        v["yAxis"] = json!(y);
    }
    if let Some(l) = c.label_axis {
        v["labelAxis"] = json!(l);
    }
    if let Some(va) = c.value_axis {
        v["valueAxis"] = json!(va);
    }
    v
}

fn chart_config_to_json(cc: &ChartConfig) -> Value {
    let mut charts = vec![];
    if let Some(c) = &cc.bar_chart {
        let mut v = chart_axis_to_json(c);
        v["chartType"] = json!("bar-chart");
        charts.push(v);
    }
    if let Some(c) = &cc.line_chart {
        let mut v = chart_axis_to_json(c);
        v["chartType"] = json!("line-chart");
        charts.push(v);
    }
    if let Some(c) = &cc.pie_chart {
        let mut v = chart_axis_to_json(c);
        v["chartType"] = json!("pie-chart");
        charts.push(v);
    }
    if let Some(c) = &cc.funnel_chart {
        let mut v = chart_axis_to_json(c);
        v["chartType"] = json!("funnel-chart");
        charts.push(v);
    }
    if let Some(c) = &cc.sankey_chart {
        let mut v = chart_axis_to_json(c);
        v["chartType"] = json!("sankey-chart");
        charts.push(v);
    }
    json!({ "charts": charts })
}

const GOLDEN_QUERIES: &[GoldenQuery] = &[
    GoldenQuery {
        id: "gq-001",
        name: "Daily Event Volume",
        description: "Count of events aggregated by day within a date range",
        use_case: "When user asks for daily event trends, event volume over time, or time-series event counts",
        query: r#"SELECT
    toDate(event_timestamp) AS date,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
GROUP BY date
ORDER BY date"#,
        tags: &["events", "daily", "volume", "time-series", "trend"],
        chart_config: Some(ChartConfig {
            line_chart: Some(ChartAxisConfig {
                x_axis: Some("date"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("date"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-002",
        name: "Event Distribution by Name",
        description: "Top events by count within a date range",
        use_case: "When user asks for event breakdown, what events are happening, or event popularity",
        query: r#"SELECT
    event_name,
    count() AS count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
GROUP BY event_name
ORDER BY count DESC
LIMIT 20"#,
        tags: &["events", "distribution", "top-events", "breakdown"],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("event_name"),
                y_axis: Some("count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("event_name"),
                value_axis: Some("count"),
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-003",
        name: "Events by Platform",
        description: "Event counts grouped by platform",
        use_case: "When user asks about platform distribution, platform usage, or breakdown by platform",
        query: r#"SELECT
    toString(properties.platform) AS platform,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND platform != ''
GROUP BY platform
ORDER BY event_count DESC"#,
        tags: &["platform", "distribution", "breakdown"],
        chart_config: Some(ChartConfig {
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("platform"),
                value_axis: Some("event_count"),
            }),
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("platform"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-004",
        name: "Events by Device Type",
        description: "Event counts grouped by device type",
        use_case: "When user asks about device breakdown, mobile vs desktop, or device type usage",
        query: r#"SELECT
    toString(properties.device_type) AS device_type,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND device_type != ''
GROUP BY device_type
ORDER BY event_count DESC"#,
        tags: &["device", "device-type", "distribution", "breakdown"],
        chart_config: Some(ChartConfig {
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("device_type"),
                value_axis: Some("event_count"),
            }),
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("device_type"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-005",
        name: "Events by Browser",
        description: "Event counts grouped by browser",
        use_case: "When user asks about browser distribution, browser usage, or which browsers are used",
        query: r#"SELECT
    toString(properties.browser) AS browser,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND browser != ''
GROUP BY browser
ORDER BY event_count DESC
LIMIT 10"#,
        tags: &["browser", "distribution", "breakdown"],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("browser"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("browser"),
                value_axis: Some("event_count"),
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-006",
        name: "Events by Country",
        description: "Event counts grouped by country",
        use_case: "When user asks about geographic distribution, country breakdown, or where users are located",
        query: r#"SELECT
    country,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND country IS NOT NULL
    AND country != ''
GROUP BY country
ORDER BY event_count DESC
LIMIT 20"#,
        tags: &["country", "geography", "location", "distribution"],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("country"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            line_chart: None,
            pie_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-007",
        name: "Hourly Event Distribution",
        description: "Event counts by hour of day",
        use_case: "When user asks about hourly patterns, time of day activity, or hourly distribution",
        query: r#"SELECT
    toHour(event_timestamp) AS hour,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
GROUP BY hour
ORDER BY hour"#,
        tags: &["hourly", "time", "pattern", "distribution"],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("hour"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            line_chart: Some(ChartAxisConfig {
                x_axis: Some("hour"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-008",
        name: "Weekly Event Trend",
        description: "Event counts by week",
        use_case: "When user asks for weekly trends, weekly overview, or week-over-week comparison",
        query: r#"SELECT
    toStartOfWeek(event_timestamp) AS week,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
GROUP BY week
ORDER BY week"#,
        tags: &["weekly", "trend", "time-series"],
        chart_config: Some(ChartConfig {
            line_chart: Some(ChartAxisConfig {
                x_axis: Some("week"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("week"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-009",
        name: "Monthly Event Trend",
        description: "Event counts by month",
        use_case: "When user asks for monthly trends, monthly overview, or month-over-month comparison",
        query: r#"SELECT
    formatDateTime(event_timestamp, '%Y-%m') AS month,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
GROUP BY month
ORDER BY month"#,
        tags: &["monthly", "trend", "time-series"],
        chart_config: Some(ChartConfig {
            line_chart: Some(ChartAxisConfig {
                x_axis: Some("month"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("month"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-010",
        name: "Specific Event Count",
        description: "Count of a specific event by name",
        use_case: "When user asks about a specific event count, how many times an event occurred",
        query: r#"SELECT
    count() AS event_count
FROM events_v2
WHERE event_name = '{event_name}'
    AND event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'"#,
        tags: &["specific-event", "count", "event-name"],
        chart_config: None,
    },
    GoldenQuery {
        id: "gq-011",
        name: "Unique Users Count",
        description: "Count of unique users within a date range",
        use_case: "When user asks for unique users, distinct users, or how many users",
        query: r#"SELECT
    uniqExact(actor_id) AS unique_users
FROM user_events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND actor_id != ''"#,
        tags: &["users", "unique", "distinct", "count"],
        chart_config: None,
    },
    GoldenQuery {
        id: "gq-012",
        name: "Daily Active Users",
        description: "Daily count of unique active users",
        use_case: "When user asks for DAU, daily active users, or users per day",
        query: r#"SELECT
    toDate(event_timestamp) AS date,
    uniqExact(actor_id) AS unique_users
FROM user_events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND actor_id != ''
GROUP BY date
ORDER BY date"#,
        tags: &["dau", "users", "daily", "active"],
        chart_config: Some(ChartConfig {
            line_chart: Some(ChartAxisConfig {
                x_axis: Some("date"),
                y_axis: Some("unique_users"),
                label_axis: None,
                value_axis: None,
            }),
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("date"),
                y_axis: Some("unique_users"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-013",
        name: "Event Funnel",
        description: "User progression through sequential events using windowFunnel",
        use_case: "When user asks for funnel, conversion steps, or sequential event analysis. Tracks how far users progress through a defined sequence of events.",
        query: r#"
WITH
    map(
        1, 'product_view',
        2, 'add_to_cart',
        3, 'purchase'
    ) AS level_mapping,
    cumulative_count AS (
        SELECT
            level,
            sum(count()) OVER (ORDER BY level DESC) AS count
        FROM (
            SELECT
                windowFunnel(1800)(
                    toDateTime(event_timestamp),
                    event_name = 'product_view',
                    event_name = 'add_to_cart',
                    event_name = 'purchase'
                ) AS level
            FROM default.user_events_v2
            GROUP BY actor_id
        )
        WHERE level > 0
        GROUP BY level
    )
SELECT
    level_mapping[level] AS event,
    count
FROM cumulative_count
ORDER BY level ASC;
"#,
        tags: &["funnel", "conversion", "sequential", "windowFunnel"],
        chart_config: Some(ChartConfig {
            funnel_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("event"),
                value_axis: Some("count"),
            }),
            bar_chart: None,
            line_chart: None,
            pie_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-014",
        name: "Top Cities by Events",
        description: "Top cities by event count",
        use_case: "When user asks about top cities, city breakdown, or where events are coming from",
        query: r#"SELECT
    toString(properties.city) AS city,
    count() AS event_count
FROM events_v2
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND city != ''
GROUP BY city
ORDER BY event_count DESC
LIMIT 20"#,
        tags: &["city", "location", "top", "distribution"],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("city"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            line_chart: None,
            pie_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-015",
        name: "Available Event Names",
        description: "List of distinct event names in the database",
        use_case: "When user wants to know what events are available, event types, or event catalog",
        query: r#"SELECT DISTINCT event_name
FROM events_v2
ORDER BY event_name
LIMIT 100"#,
        tags: &["events", "catalog", "list", "available"],
        chart_config: None,
    },
    GoldenQuery {
        id: "gq-016",
        name: "Event Property Distribution",
        description: "Distribution of values for a specific property of a given event",
        use_case: "When user asks about property values distribution, breakdown of an event property, or what values a property has",
        query: r#"SELECT
    toString(properties.{property_name}) AS property_value,
    count() AS count
FROM events_v2
WHERE event_name = '{event_name}'
    AND event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
    AND properties.{property_name} IS NOT NULL
GROUP BY property_value
ORDER BY count DESC
LIMIT 30"#,
        tags: &[
            "events",
            "property",
            "distribution",
            "breakdown",
            "properties",
        ],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("property_value"),
                y_axis: Some("count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("property_value"),
                value_axis: Some("count"),
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-017",
        name: "Mean Reciprocal Rank",
        description: "Calculate reciprocal rank for a specific event",
        use_case: "When user asks about ranking, reciprocal rank, or position of an event in a ranking",
        query: r#"SELECT avg(1.0 / properties.{property_name}::Float64) AS mrr FROM events_v2 WHERE event_name = '{event_name}' AND properties.{property_name} IS NOT NULL AND event_timestamp >= '{start_date}' AND event_timestamp <= '{end_date}'"#,
        tags: &["ranking", "reciprocal-rank"],
        chart_config: None,
    },
    GoldenQuery {
        id: "gq-018",
        name: "Consecutive Event Time Gap Distribution",
        description: "Distribution of time gaps between consecutive events of the same type per user",
        use_case: "When user asks about time between events, session gaps, repeat behavior, or dwell time between consecutive actions",
        query: r#"SELECT
    multiIf(
        time_gap < 10, '<10s',
        time_gap < 30, '10-30s',
        time_gap < 60, '30-60s',
        time_gap < 300, '1-5min',
        '5min+'
    ) AS gap_bucket,
    count() AS pair_count
FROM (
    SELECT
        (lead(event_timestamp) OVER w) - event_timestamp AS time_gap
    FROM user_events_v2
    WHERE event_name = '{event_name}' AND project_id = '{project_id}'
    WINDOW w AS (PARTITION BY actor_id ORDER BY event_timestamp)
)
WHERE time_gap IS NOT NULL
GROUP BY gap_bucket
ORDER BY gap_bucket"#,
        tags: &[
            "consecutive",
            "time-gap",
            "window",
            "lead",
            "session",
            "dwell-time",
            "distribution",
        ],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("gap_bucket"),
                y_axis: Some("pair_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("gap_bucket"),
                value_axis: Some("pair_count"),
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-019",
        name: "Consecutive Event Text Similarity Distribution",
        description: "Distribution of edit distance between text properties of consecutive events per user",
        use_case: "When user asks about query reformulation, search refinement, typo correction, or text similarity between consecutive user actions",
        query: r#"SELECT
    multiIf(
        edit_dist = 0, 'identical',
        edit_dist <= 2, 'minor_change',
        edit_dist <= 5, 'moderate_change',
        'major_change'
    ) AS similarity_bucket,
    count() AS pair_count
FROM (
    SELECT
        editDistance(properties.{property_name}, lead(properties.{property_name}) OVER w) AS edit_dist
    FROM user_events_v2
    WHERE event_name = '{event_name}' AND project_id = '{project_id}'
    WINDOW w AS (PARTITION BY actor_id ORDER BY event_timestamp)
)
WHERE edit_dist IS NOT NULL
GROUP BY similarity_bucket
ORDER BY similarity_bucket"#,
        tags: &[
            "consecutive",
            "similarity",
            "edit-distance",
            "text",
            "window",
            "lead",
            "reformulation",
            "distribution",
        ],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("similarity_bucket"),
                y_axis: Some("pair_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("similarity_bucket"),
                value_axis: Some("pair_count"),
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-020",
        name: "Reciprocal Position Distribution",
        description: "Distribution of reciprocal position (1/rank) showing how often users select top-ranked results",
        use_case: "When user asks about reciprocal rank distribution, position weighting, or how often top vs lower ranked items are selected",
        query: r#"SELECT
    multiIf(
        inv_position >= 0.5, 'top_2',
        inv_position >= 0.2, 'top_5',
        inv_position >= 0.1, 'top_10',
        'beyond_10'
    ) AS position_bucket,
    count() AS event_count
FROM (
    SELECT 1 / properties.{property_name} AS inv_position
    FROM user_events_v2
    WHERE event_name = '{event_name}' AND project_id = '{project_id}'
        AND properties.{property_name} IS NOT NULL
)
WHERE inv_position IS NOT NULL
GROUP BY position_bucket
ORDER BY position_bucket"#,
        tags: &[
            "ranking",
            "reciprocal",
            "position",
            "inverse",
            "weight",
            "distribution",
        ],
        chart_config: Some(ChartConfig {
            bar_chart: Some(ChartAxisConfig {
                x_axis: Some("position_bucket"),
                y_axis: Some("event_count"),
                label_axis: None,
                value_axis: None,
            }),
            pie_chart: Some(ChartAxisConfig {
                x_axis: None,
                y_axis: None,
                label_axis: Some("position_bucket"),
                value_axis: Some("event_count"),
            }),
            line_chart: None,
            funnel_chart: None,
            sankey_chart: None,
        }),
    },
    GoldenQuery {
        id: "gq-021",
        name: "Boolean Cross-Tabulation",
        description: "2x2 contingency table counting rows by two boolean conditions",
        use_case: "When user asks for a 2x2 matrix, cross-tab, confusion matrix, or grouping by two boolean conditions on the same dataset",
        query: r#"SELECT
    {condition_a} AS dim_a,
    {condition_b} AS dim_b,
    count() AS cnt
FROM (
    {subquery}
)
WHERE {filter_conditions}
GROUP BY dim_a, dim_b"#,
        tags: &[
            "cross-tab",
            "2x2",
            "matrix",
            "boolean",
            "contingency",
            "grouping",
        ],
        chart_config: None,
    },
];

pub fn list_golden_queries() -> Value {
    let queries: Vec<Value> = GOLDEN_QUERIES
        .iter()
        .map(|q| {
            json!({
                "id": q.id,
                "name": q.name,
                "description": q.description,
                "useCase": q.use_case,
                "tags": q.tags,
            })
        })
        .collect();
    json!({ "golden_queries": queries })
}

pub fn fetch_golden_query(identifier: &str) -> Value {
    let lower = identifier.to_lowercase();
    let query = GOLDEN_QUERIES
        .iter()
        .find(|q| q.name.to_lowercase() == lower || q.id == identifier);

    match query {
        Some(q) => {
            let mut result = json!({
                "id": q.id,
                "name": q.name,
                "description": q.description,
                "useCase": q.use_case,
                "query": q.query,
                "tags": q.tags,
            });
            if let Some(cc) = &q.chart_config {
                result["chartConfig"] = chart_config_to_json(cc);
            }
            result
        }
        None => json!({ "error": format!("Golden query '{}' not found", identifier) }),
    }
}
