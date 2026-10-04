//! `cargo xtask ps` / `cargo xtask setup` — docker compose stack tooling.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::process::Command;

pub(crate) fn ps() {
    crate::run("sudo", &["docker", "compose", "ps", "-a"]);
}

pub(crate) fn setup(requested: &[String]) {
    let available = compose_services();
    if available.is_empty() {
        eprintln!("no services found in docker-compose.yml");
        std::process::exit(1);
    }
    let selected = if requested.is_empty() {
        select_services(&available)
    } else {
        parse_selection(&requested.join(" "), &available).unwrap_or_else(|message| fail(message))
    };
    println!("starting: {}", selected.join(", "));
    let mut args = vec!["docker", "compose", "up", "-d"];
    args.extend(selected.iter().map(String::as_str));
    crate::run("sudo", &args);
}

/// Service names from docker-compose.yml, sorted — `config` is client-side,
/// so this needs no daemon and no sudo.
fn compose_services() -> Vec<String> {
    let output = Command::new("docker")
        .args(["compose", "config", "--services"])
        .current_dir(crate::workspace_root())
        .output()
        .expect("failed to spawn docker");
    if !output.status.success() {
        eprintln!(
            "failed to list compose services: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::process::exit(1);
    }
    let mut services: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .map(str::to_owned)
        .filter(|service| !service.is_empty())
        .collect();
    services.sort_unstable();
    services
}

fn select_services(available: &[String]) -> Vec<String> {
    println!("docker compose services:");
    for (index, service) in available.iter().enumerate() {
        println!("  {}. {service}", index + 1);
    }
    print!("\nstart which? (numbers or names, space/comma separated, empty = all): ");
    io::stdout().flush().expect("failed to flush stdout");

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("failed to read selection");
    parse_selection(&input, available).unwrap_or_else(|message| fail(message))
}

/// Resolve a selection (`"2 postgres"`, `"1,3"`, `""`, …) to service names,
/// deduplicated and in `available` order. Empty input or `all` means all.
fn parse_selection(input: &str, available: &[String]) -> Result<Vec<String>, String> {
    let tokens: Vec<&str> = input
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() || tokens.contains(&"all") {
        return Ok(available.to_vec());
    }
    let mut chosen: BTreeSet<usize> = BTreeSet::new();
    for token in tokens {
        match token.parse::<usize>() {
            Ok(number) => {
                if number == 0 || number > available.len() {
                    return Err(format!(
                        "service number {number} out of range (1..={})",
                        available.len()
                    ));
                }
                chosen.insert(number - 1);
            }
            Err(_) => match available.iter().position(|service| service == token) {
                Some(position) => {
                    chosen.insert(position);
                }
                None => {
                    return Err(format!(
                        "unknown service {token:?} (available: {})",
                        available.join(", ")
                    ));
                }
            },
        }
    }
    Ok(chosen
        .into_iter()
        .map(|position| available[position].clone())
        .collect())
}

fn fail(message: String) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::parse_selection;

    fn available() -> Vec<String> {
        ["keycloak", "minio", "postgres", "redpanda"]
            .map(str::to_owned)
            .to_vec()
    }

    fn names(services: &[String]) -> Vec<&str> {
        services.iter().map(String::as_str).collect()
    }

    #[test]
    fn selection_when_input_is_empty_returns_all() {
        assert_eq!(
            names(&parse_selection("", &available()).unwrap()),
            ["keycloak", "minio", "postgres", "redpanda"]
        );
    }

    #[test]
    fn selection_when_input_is_all_returns_all() {
        assert_eq!(
            names(&parse_selection("  all\n", &available()).unwrap()),
            ["keycloak", "minio", "postgres", "redpanda"]
        );
    }

    #[test]
    fn selection_by_numbers_maps_and_dedupes() {
        assert_eq!(
            names(&parse_selection("3 1,3", &available()).unwrap()),
            ["keycloak", "postgres"]
        );
    }

    #[test]
    fn selection_by_names_and_mixed_tokens_dedupes_in_available_order() {
        assert_eq!(
            names(&parse_selection("postgres 1 keycloak", &available()).unwrap()),
            ["keycloak", "postgres"]
        );
    }

    #[test]
    fn selection_when_name_is_unknown_is_rejected() {
        assert!(parse_selection("postgres kafka", &available()).is_err());
    }

    #[test]
    fn selection_when_number_is_out_of_range_is_rejected() {
        assert!(parse_selection("5", &available()).is_err());
        assert!(parse_selection("0", &available()).is_err());
    }
}
