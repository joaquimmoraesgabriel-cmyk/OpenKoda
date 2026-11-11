//! Guided Windows preparation. These probes never launch a distribution or change the host.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Checkpoint {
    started_at: String,
    initial_boot: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostFacts {
    windows_build: Option<u32>,
    architecture: Option<String>,
    hypervisor: Option<bool>,
    virtualization_firmware: Option<bool>,
    virtual_machine_platform: Option<bool>,
    wsl_client: Option<bool>,
    free_bytes: Option<u64>,
    boot: Option<String>,
}

const INVENTORY: &str = include_str!("environment_host_inventory.ps1");

impl EnvironmentManager {
    fn host_facts(&self) -> HostFacts {
        let result = self.runner.run(&CommandSpec {
            purpose: "Windows prerequisites",
            executable: system_binary("WindowsPowerShell\\v1.0\\powershell.exe"),
            args: vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), INVENTORY.into()],
            input: Some(display_path(&self.install_location()).into_bytes()),
            timeout: Duration::from_secs(15),
        });
        result.ok().filter(|r| checked(r).is_ok())
            .and_then(|r| serde_json::from_str(&r.stdout).ok()).unwrap_or_default()
    }

    pub(super) fn host_readiness(&self, start: bool) -> Result<Value, String> {
        let mut state = self.state()?;
        if state.host_checkpoint.is_none() && !start {
            return Ok(Value::Null);
        }
        let host = self.host_facts();
        let mut status_ok = None;
        let mut dedicated_install = None;
        let mut version = None;
        if host.wsl_client == Some(true) {
            status_ok = self.wsl("WSL host status", vec!["--status".into()], 10).ok()
                .filter(|r| complete_capture(r).is_ok()).map(|r| r.code == 0 && !r.stdout.trim().is_empty());
            dedicated_install = self.wsl("WSL host capabilities", vec!["--help".into()], 10).ok()
                .filter(|r| complete_capture(r).is_ok()).map(|r| installation_capabilities(&r).is_ok());
            version = self.wsl("WSL host version", vec!["--version".into()], 10).ok()
                .filter(|r| checked(r).is_ok()).and_then(|r| {
                    r.stdout.lines().next()?.split_whitespace().find(|s| {
                        s.len() <= 32 && s.split('.').count() >= 2
                            && s.split('.').all(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
                    }).map(str::to_owned)
                });
        }
        let registrations = self.inventory().ok();
        let distribution_count = registrations.as_ref().map(Vec::len);
        let step = next_step(&host, status_ok, dedicated_install, distribution_count);
        if state.host_checkpoint.is_none() {
            state.host_checkpoint = Some(Checkpoint { started_at: now(), initial_boot: host.boot.clone() });
            self.save_state(&state)?;
        }
        let checkpoint = state.host_checkpoint.as_ref().expect("checkpoint saved");
        let reboot_observed = checkpoint.initial_boot.as_ref().zip(host.boot.as_ref())
            .map(|(before, current)| before != current);
        Ok(json!({
            "startedAt": checkpoint.started_at, "checkedAt": now(), "step": step,
            "rebootObserved": reboot_observed, "windowsBuild": host.windows_build,
            "architecture": host.architecture, "hypervisor": host.hypervisor,
            "virtualizationFirmware": host.virtualization_firmware,
            "virtualMachinePlatform": host.virtual_machine_platform,
            "freeBytes": host.free_bytes, "distributionCount": distribution_count,
            "wslClient": host.wsl_client, "wslVersion": version,
            "wslStatusOk": status_ok, "dedicatedInstall": dedicated_install,
        }))
    }
}

fn next_step(host: &HostFacts, status_ok: Option<bool>, dedicated: Option<bool>, distros: Option<usize>) -> &'static str {
    if host.architecture.as_deref().is_some_and(|arch| arch != "x64") { return "architecture"; }
    if host.windows_build.is_some_and(|build| build < 19041) { return "windows_update"; }
    if host.windows_build.is_none() || host.architecture.is_none() || distros.is_none() { return "inspect"; }
    if host.hypervisor == Some(false) && host.virtualization_firmware == Some(false) { return "virtualization"; }
    if host.wsl_client == Some(false) || host.virtual_machine_platform == Some(false) {
        return if distros == Some(0) { "prepare_wsl" } else { "verify_wsl" };
    }
    if host.wsl_client != Some(true) || status_ok.is_none() || dedicated.is_none() { return "inspect"; }
    if status_ok == Some(false) {
        return if distros == Some(0) && dedicated == Some(false) { "prepare_wsl" } else { "verify_wsl" };
    }
    if dedicated == Some(false) { return "update_wsl"; }
    "review_kali"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_guidance_preserves_existing_distributions_and_distinguishes_unknowns() {
        let mut facts = HostFacts { windows_build: Some(22631), architecture: Some("x64".into()),
            hypervisor: Some(true), virtualization_firmware: Some(false), virtual_machine_platform: Some(false),
            wsl_client: Some(true), ..HostFacts::default() };
        assert_eq!(next_step(&facts, Some(false), Some(false), Some(2)), "verify_wsl");
        assert_eq!(next_step(&facts, Some(false), Some(false), Some(0)), "prepare_wsl");
        facts.virtual_machine_platform = Some(true);
        assert_eq!(next_step(&facts, Some(true), Some(false), Some(2)), "update_wsl");
        assert_eq!(next_step(&facts, None, None, Some(2)), "inspect");
        assert_eq!(next_step(&facts, Some(true), Some(true), None), "inspect");
        facts.hypervisor = Some(false);
        assert_eq!(next_step(&facts, Some(false), Some(false), Some(0)), "virtualization");
        facts.architecture = Some("arm64".into());
        assert_eq!(next_step(&facts, Some(true), Some(true), Some(0)), "architecture");
    }
}
