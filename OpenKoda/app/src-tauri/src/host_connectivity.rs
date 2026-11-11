//! The Windows host's connectivity indication is independent of provider and executor health.
use super::{now, system_binary, CommandSpec, ProcessResult, Runner, SystemRunner};
use serde_json::{json, Value};
use std::{sync::Mutex, time::Duration};

static PROBE: Mutex<()> = Mutex::new(());
const QUERY: &str = r#"
$ErrorActionPreference = 'Stop'
$type = [type]::GetTypeFromCLSID([guid]'DCB00C01-570F-4A9B-8D69-199FDBA5723B', $true)
$manager = [Activator]::CreateInstance($type)
[int]$manager.GetConnectivity()
"#;

// NLM_CONNECTIVITY, netlistmgr.h. Flags apply across IPv4 and IPv6.
fn indication(result: Result<ProcessResult, String>) -> &'static str {
    let Some(bits) = result.ok()
        .filter(|r| r.code == 0 && !r.timed_out && !r.truncated && r.stderr.trim().is_empty())
        .and_then(|r| r.stdout.trim().parse::<u32>().ok())
    else {
        return "unavailable";
    };
    const INTERNET: u32 = 0x40 | 0x400;
    const LOCAL: u32 = 0x10 | 0x20 | 0x100 | 0x200;
    const NO_TRAFFIC: u32 = 0x1 | 0x2;
    if bits & !(INTERNET | LOCAL | NO_TRAFFIC) != 0 {
        "unavailable"
    } else if bits & INTERNET != 0 {
        "internet"
    } else if bits & LOCAL != 0 {
        "local"
    } else if bits & NO_TRAFFIC != 0 {
        "noTraffic"
    } else {
        "disconnected"
    }
}

fn observe_with(runner: &dyn Runner, gate: &Mutex<()>, payload: &Value) -> Result<Value, String> {
    if !payload.as_object().is_some_and(|input| input.is_empty()) {
        return Err("A consulta de conectividade não recebe parâmetros.".into());
    }
    let _probe = gate.try_lock().map_err(|_| "Já existe uma consulta de conectividade em andamento.")?;
    let status = if cfg!(windows) {
        indication(runner.run(&CommandSpec {
            purpose: "Windows Network List Manager",
            executable: system_binary("WindowsPowerShell\\v1.0\\powershell.exe"),
            args: vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), QUERY.into()],
            input: None,
            timeout: Duration::from_secs(5),
        }))
    } else {
        "unavailable"
    };
    // Return only this fixed metadata. Raw COM errors and network identifiers never cross IPC.
    Ok(json!({"source":"windowsNlm", "status":status, "observedAt":now()}))
}

pub(crate) fn observe(payload: &Value) -> Result<Value, String> {
    observe_with(&SystemRunner, &PROBE, payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(stdout: &str) -> ProcessResult {
        ProcessResult { code: 0, stdout: stdout.into(), stderr: String::new(), timed_out: false, truncated: false }
    }

    #[test]
    fn distinguishes_family_flags_and_mixed_networks() {
        for (bits, status) in [
            (0, "disconnected"), (1, "noTraffic"), (2, "noTraffic"), (3, "noTraffic"),
            (0x10, "local"), (0x20, "local"), (0x100, "local"), (0x200, "local"), (0x201, "local"),
            (0x40, "internet"), (0x400, "internet"), (0x42, "internet"), (0x430, "internet"),
            (0x80, "unavailable"), (0x4400, "unavailable"),
        ] {
            assert_eq!(indication(Ok(output(&bits.to_string()))), status, "mask {bits:#x}");
        }
    }

    #[test]
    fn errors_incomplete_and_malformed_output_are_unknown() {
        for result in [
            Err("private diagnostic".into()), Ok(output("")), Ok(output("64\n0")), Ok(output("-1")),
            Ok(ProcessResult { timed_out: true, ..output("64") }),
            Ok(ProcessResult { truncated: true, ..output("0") }),
            Ok(ProcessResult { code: 1, ..output("64") }),
            Ok(ProcessResult { stderr: "private diagnostic".into(), ..output("0") }),
        ] {
            assert_eq!(indication(result), "unavailable");
        }
    }

    #[test]
    fn rejects_arguments_and_concurrent_queries_before_launch() {
        struct NeverRun;
        impl Runner for NeverRun {
            fn run(&self, _: &CommandSpec) -> Result<ProcessResult, String> {
                panic!("A second or parameterized request must not launch a process");
            }
        }
        let gate = Mutex::new(());
        assert!(observe_with(&NeverRun, &gate, &json!({"endpoint": "fixture"})).is_err());
        let _held = gate.lock().unwrap();
        assert!(observe_with(&NeverRun, &gate, &json!({})).unwrap_err().contains("em andamento"));
    }

    #[cfg(windows)]
    #[test]
    fn ipc_projects_only_the_observation_even_when_com_fails() {
        struct FailedCom;
        impl Runner for FailedCom {
            fn run(&self, spec: &CommandSpec) -> Result<ProcessResult, String> {
                assert_eq!(spec.executable, system_binary("WindowsPowerShell\\v1.0\\powershell.exe"));
                assert_eq!(spec.args, ["-NoProfile", "-NonInteractive", "-Command", QUERY]);
                assert!(spec.input.is_none());
                assert_eq!(spec.timeout, Duration::from_secs(5));
                Err("private COM diagnostics".into())
            }
        }
        let value = observe_with(&FailedCom, &Mutex::new(()), &json!({})).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 3);
        assert_eq!(value["status"], "unavailable");
        assert_eq!(value["source"], "windowsNlm");
        chrono::DateTime::parse_from_rfc3339(value["observedAt"].as_str().unwrap()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "opt-in read of the Windows host's actual Network List Manager"]
    fn actual_windows_indication() {
        let value = observe(&json!({})).unwrap();
        assert_eq!(value["source"], "windowsNlm");
        assert!(["internet","local","noTraffic","disconnected"].contains(&value["status"].as_str().unwrap()));
        chrono::DateTime::parse_from_rfc3339(value["observedAt"].as_str().unwrap()).unwrap();
        println!("{value}");
    }
}
