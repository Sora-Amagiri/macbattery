use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Render a frame to text so assertions can inspect the visible screen.
fn render(info: &BatteryInfo, paused: bool, w: u16, h: u16) -> String {
    let app = App {
        info: info.clone(),
        paused,
        last_update: Instant::now(),
    };
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw(f, &app)).unwrap();

    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        out.push('\n');
    }
    out
}

fn charging() -> BatteryInfo {
    BatteryInfo {
        present: true,
        charge_percent: 42.0,
        is_charging: true,
        fully_charged: false,
        on_ac: true,
        time_to_empty_min: None,
        time_to_full_min: Some(46),
        cycle_count: 81,
        max_cycle_count: 1000,
        design_capacity: 4381,
        full_charge_capacity: 3842,
        current_capacity: 3700,
        health_percent: 87.7,
        voltage_mv: 12689,
        amperage_ma: 937,
        watts: 11.9,
        temperature_c: 31.4,
        condition: battery::Condition::Normal,
        adapter_watts: Some(45),
        manufacturer: Some("DSY".into()),
        device_name: Some("bq20z451".into()),
        serial: Some("F5D01752TC4N4F3AF".into()),
        manufacture_date: Some("2023-04-28".into()),
    }
}

#[test]
fn charging_dashboard_shows_all_panels() {
    let screen = render(&charging(), false, 80, 24);
    for expected in [
        "MacBook Battery",
        "Charging",
        "42%",
        "until full",
        "46 min",
        "Cycles",
        "81",
        "Health",
        "Normal",
        "Voltage",
        "Temperature",
        "F5D01752TC4N4F3AF",
        "2023-04-28",
        "quit",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}:\n{screen}");
    }
}

#[test]
fn discharging_shows_drain_estimate() {
    let mut info = charging();
    info.is_charging = false;
    info.on_ac = false;
    info.time_to_full_min = None;
    info.time_to_empty_min = Some(106); // 1 h 46 min
    let screen = render(&info, false, 80, 24);
    assert!(screen.contains("On Battery"), "{screen}");
    assert!(screen.contains("1 h 46 min"), "{screen}");
    assert!(screen.contains("remaining"), "{screen}");
    assert!(!screen.contains("until full"), "{screen}");
}

#[test]
fn on_ac_without_charge_estimate() {
    let mut info = charging();
    info.is_charging = false;
    info.on_ac = true;
    info.time_to_full_min = None;
    info.time_to_empty_min = None;
    let screen = render(&info, false, 80, 24);
    assert!(screen.contains("On AC Power"), "{screen}");
    assert!(screen.contains("Time estimate unavailable"), "{screen}");
    assert!(!screen.contains("remaining"), "{screen}");
}

#[test]
fn fully_charged_state() {
    let mut info = charging();
    info.charge_percent = 100.0;
    info.is_charging = false;
    info.fully_charged = true;
    info.time_to_full_min = None;
    let screen = render(&info, false, 80, 24);
    assert!(screen.contains("Fully Charged"), "{screen}");
    assert!(!screen.contains("until full"), "{screen}");
}

#[test]
fn no_battery_renders_clean_screen() {
    let screen = render(&BatteryInfo::absent(), false, 80, 24);
    assert!(screen.contains("No battery detected"), "{screen}");
    // Regression: the zero-filled panels must not render without a battery.
    assert!(!screen.contains("Cycles"), "{screen}");
    assert!(!screen.contains("Serial"), "{screen}");
}

#[test]
fn small_terminal_shows_hint() {
    let screen = render(&charging(), false, 40, 10);
    assert!(screen.contains("Terminal too small"), "{screen}");
}

#[test]
fn footer_shows_pause_state() {
    assert!(render(&charging(), true, 80, 24).contains("paused"));
    assert!(!render(&charging(), false, 80, 24).contains("paused"));
}

#[test]
fn one_shot_report_handles_no_battery() {
    let report = crate::text_report(&BatteryInfo::absent());
    assert_eq!(report, "No battery detected on this machine.");
}

#[test]
fn one_shot_report_contains_key_fields() {
    let report = crate::text_report(&charging());
    for expected in [
        "42%",
        "46 min until full",
        "87.7%",
        "81 / 1000",
        "2023-04-28",
    ] {
        assert!(report.contains(expected), "missing {expected:?}:\n{report}");
    }
}
