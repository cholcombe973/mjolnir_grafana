#[macro_use]
extern crate serde_derive;

use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::process;

use mjolnir_api::{plugin_list, Alert, Discover, Remediation, RemediationResult};
use serde_json::{self, Value};

#[cfg(test)]
mod tests {
    use super::Incoming;

    fn it_parses_grafana_json() {
        let json = r#"
        {
          "title": "My alert",
          "ruleId": 1,
          "ruleName": "Load peaking!",
          "ruleUrl": "http://url.to.grafana/db/dashboard/my_dashboard?panelId=2",
          "state": "alerting",
          "imageUrl": "http://s3.image.url",
          "message": "Load is peaking. Make sure the traffic is real and spin up more webfronts",
          "evalMatches": [
              {
                  "metric": "requests",
                  "tags": {},
                  "value": 122
              }
          ]
        }"#;
        let alert: Incoming = serde_json::from_str(&json).unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct Eval {
    metric: String,
    tags: Value,
    value: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Alerting,
    NoData,
    Ok,
    Paused,
    Pending,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Incoming {
    title: String,
    rule_id: String,
    rule_name: String,
    rule_url: String,
    state: State,
    image_url: String,
    message: String,
    eval_matches: Vec<Eval>,
}

// What does your plugin look like?
fn generate_usage() -> Discover {
    Discover::new("grafana")
        .with_author("Chris Holcombe <xfactor973@gmail.com>")
        .with_version("0.0.1")
        .with_alerts(generate_alerts())
        .with_remediations(generate_actions())
        .webhook()
}

// you can plug in actions and alerts below
fn generate_alerts() -> Vec<Alert> {
    // Your alerts here
    vec![Alert::new("grafana")]
}

fn generate_actions() -> Vec<Remediation> {
    // Your actions here
    vec![]
}

fn list_plugins() -> HashMap<String, fn(HashMap<String, String>) -> RemediationResult> {
    // Insert your plugins here!
    plugin_list!("grafana" => grafana)
}

// Your plugins should be functions with this signature
fn grafana(args: HashMap<String, String>) -> RemediationResult {
    let body: String = if let Some(body) = args.get("body") {
        if body.len() > 0 {
            body.clone()
        } else {
            return RemediationResult::new().err(format!("Empty Body"));
        }
    } else {
        return RemediationResult::new().err(format!("Missing required argument: Body"));
    };
    let incoming: Incoming = match serde_json::from_str(&body) {
        Ok(a) => a,
        Err(e) => return RemediationResult::new().err(format!("Failed to parse json: {:?}", e)),
    };
    let mut alert = Alert::new("alertmanager");
    alert = alert.with_arg(format!("raw={:?}", incoming));
    alert = alert.with_name(incoming.title);
    alert = alert.with_arg(format!("state={:?}", incoming.state));
    for e in &incoming.eval_matches {
        alert = alert.with_arg(format!("{}={}", e.metric, e.value));
    }
    RemediationResult::new().ok().with_alerts(vec![alert])
}

// Don't touch anything below here!
fn main() {
    let plugins = list_plugins();

    let mut arg_list = get_args();

    let plugin = arg_list.remove("plugin").unwrap_or_else(|| {
        println!("Could not find a requested plugin");
        process::exit(1);
    });

    let f = plugins.get(&plugin).unwrap_or_else(|| {
        println!(
            "{} is not a registered plugin, available plugins are: {:?}",
            plugin,
            plugins.keys()
        );
        process::exit(1);
    });

    let res = f(arg_list);

    io::stdout().write(&res.write_to_bytes().unwrap()).unwrap();
}

fn get_args() -> HashMap<String, String> {
    let mut args = env::args();
    if args.len() == 1 {
        // This is the usage directions to Mjolnir
        io::stdout()
            .write(&generate_usage().write_to_bytes().unwrap())
            .unwrap();
        process::exit(0);
    } else {
        let mut arg_list: HashMap<String, String> = HashMap::new();
        let _ = args.next();
        for arg in args {
            let mut parts = arg.split("=");
            let name = parts.next().unwrap().replace("--", "");
            let mut value = parts.next();
            if value.is_none() {
                value = Some("");
            }
            arg_list.insert(name.into(), value.unwrap().into());
        }
        return arg_list;
    }
}
