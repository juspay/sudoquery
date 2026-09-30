use serde_json::json;

pub fn get_tools() -> serde_json::Value {
    let GET_CURRENT_DATE_TIME_TOOL: serde_json::Value = json!({
      "type": "function",
      "function": {
        "name": "get_current_datetime",
        "description": "Get the current date and time. Use this before creating queries with date filters to ensure you use appropriate date ranges. Returns current date, time, and useful date values for queries.",
        "parameters": {
          "type": "object",
          "properties": {},
          "required": [],
        },
      },
    });

    json!([GET_CURRENT_DATE_TIME_TOOL])
}
