//! macbattery — live MacBook battery health dashboard.
//!
//! Run with no arguments for the TUI, or `--once` for a plain-text report.

#[cfg(not(target_os = "macos"))]
compile_error!("macbattery only supports macOS: the battery service is read via IOKit.");

mod battery;
mod ui;

const HELP: &str = "\
macbattery — MacBook battery health dashboard

USAGE:
    macbattery             live TUI dashboard
    macbattery --once      print a one-shot report and exit (-1)
    macbattery --version   print the version and exit (-V)
    macbattery --help      show this help (-h)

KEYS (TUI):
    q / Esc / Ctrl+C   quit
    r                  refresh now
    p                  pause / resume auto-refresh
";

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => {}
        Some("--help" | "-h") => {
            print!("{HELP}");
            return;
        }
        Some("--version" | "-V") => {
            println!("macbattery {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--once" | "-1") => {
            if let Some(extra) = args.next() {
                eprintln!("error: unexpected argument '{extra}' after '--once'\n");
                eprint!("{HELP}");
                std::process::exit(2);
            }
            print!("{}", text_report(&battery::read()));
            return;
        }
        Some(other) => {
            eprintln!("error: unknown argument '{other}'\n");
            eprint!("{HELP}");
            std::process::exit(2);
        }
    }
    if let Err(err) = ui::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn text_report(info: &battery::BatteryInfo) -> String {
    if !info.present {
        return "No battery detected on this machine.".into();
    }

    let time = if let Some(min) = info.time_to_full_min {
        format!("{} until full", battery::fmt_duration(min))
    } else if let Some(min) = info.time_to_empty_min {
        format!("{} remaining", battery::fmt_duration(min))
    } else {
        "unavailable".into()
    };

    let mut out = String::new();
    out.push_str("MacBook Battery Report\n");
    out.push_str(&format!(
        "  {:<14} {:.0}%  ({} / {} mAh)  {}\n",
        "Charge",
        info.charge_percent,
        info.current_capacity,
        info.full_charge_capacity,
        info.state()
    ));
    out.push_str(&format!("  {:<14} {}\n", "Time", time));
    out.push_str(&format!(
        "  {:<14} {:.1}%  ({} mAh of {} mAh design)\n",
        "Health", info.health_percent, info.full_charge_capacity, info.design_capacity
    ));
    out.push_str(&format!(
        "  {:<14} {} / {}\n",
        "Cycles", info.cycle_count, info.max_cycle_count
    ));
    out.push_str(&format!(
        "  {:<14} {:.3} V  {}  {:.1} W  {:.1} °C\n",
        "Power",
        info.voltage_mv as f64 / 1000.0,
        battery::fmt_amperage(info.amperage_ma),
        info.watts,
        info.temperature_c
    ));
    out.push_str(&format!(
        "  {:<14} {}\n",
        "Adapter",
        info.adapter_watts
            .map(|w| format!("{w} W"))
            .unwrap_or_else(|| "—".into())
    ));
    out.push_str(&format!(
        "  {:<14} {}\n",
        "Condition",
        info.condition.as_str()
    ));
    out.push_str(&format!(
        "  {:<14} {}\n",
        "Serial",
        info.serial.clone().unwrap_or_else(|| "—".into())
    ));
    out.push_str(&format!(
        "  {:<14} {}{}\n",
        "Manufacturer",
        info.manufacturer.clone().unwrap_or_else(|| "—".into()),
        info.device_name
            .as_ref()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "  {:<14} {}\n",
        "Manufactured",
        info.manufacture_date.clone().unwrap_or_else(|| "—".into())
    ));
    out
}
