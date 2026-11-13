//! Prints what the meter reads on this machine: percentages, ages and reset times, nothing else.

use agent_usage::{Freshness, Usage, WindowKind};
use std::time::{Duration, SystemTime};

fn span(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    match minutes {
        0 => format!("{}s", duration.as_secs()),
        1..=119 => format!("{minutes}m"),
        120..=2879 => format!("{}h {:02}m", minutes / 60, minutes % 60),
        _ => format!("{}d {}h", minutes / 1440, minutes / 60 % 24),
    }
}

fn main() {
    let now = SystemTime::now();
    let snapshot = agent_usage::discover(now);
    if snapshot.agents.is_empty() {
        println!("no agents found");
    }

    for found in &snapshot.agents {
        println!("{}", found.agent.title().to_uppercase());
        match &found.usage {
            Usage::NoReading => println!("  no reading yet"),
            Usage::Allowance(windows) => {
                for window in windows {
                    let name = match window.kind {
                        WindowKind::Session => "session".to_owned(),
                        WindowKind::Weekly => "weekly".to_owned(),
                        WindowKind::Other => format!("{:?} min", window.minutes),
                    };
                    let state = match window.state {
                        Freshness::Fresh => "fresh",
                        Freshness::Stale => "stale",
                        Freshness::Expired => "expired",
                    };
                    let reset = match window.resets_at.map(|r| r.duration_since(now)) {
                        Some(Ok(left)) => format!("resets in {}", span(left)),
                        Some(Err(past)) => format!("reset {} ago", span(past.duration())),
                        None => "reset unknown".to_owned(),
                    };
                    println!(
                        "  {name:<10} {:5.1}%  seen {} ago  {reset}  [{state}]",
                        window.used_percent,
                        span(window.age(now)),
                    );
                }
            }
            Usage::Totals(totals) => {
                println!(
                    "  token totals over the last {}, not an allowance",
                    span(agent_usage::TOTALS_SPAN)
                );
                println!(
                    "  replies {}  input {}  output {}  reasoning {}  cache read {}  cache write {}  cost ${:.2}",
                    totals.replies,
                    totals.input,
                    totals.output,
                    totals.reasoning,
                    totals.cache_read,
                    totals.cache_write,
                    totals.cost,
                );
                if let Some(Ok(age)) = totals.observed_at.map(|t| now.duration_since(t)) {
                    println!("  last written {} ago", span(age));
                }
            }
        }
    }
}
