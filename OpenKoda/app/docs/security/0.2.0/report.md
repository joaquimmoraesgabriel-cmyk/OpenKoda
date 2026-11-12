# Security Review: OpenKoda-code

## Scope

No validated exploitable vulnerabilities were established in the 39 distinct source paths fully reviewed. This is partial coverage of the repository-wide scan: the SDK estimates 252 in-scope files, leaving 213 unreviewed.

- Scan mode: repository
- Target kind: git\_revision
- Target ID: target\_sha256\_d886f909a6bf154fe9655d34666997fd19987ce66516598a738cc54c6c56ac48
- Revision: 58fa65e48886defed56dc6e9569b0531ffc04acf
- Included paths: .
- Excluded paths: none
- Scan context: Analise a segurança do código do próprio OpenKoda. Priorize fronteiras IPC, gestão de processos do motor e tratamento de credenciais. Não edite arquivos. Produza o relatório com cobertura e evidências. Repository-wide scan of revision 58fa65e48886defed56dc6e9569b0531ffc04acf.

Limitations and exclusions:
- Coverage is partial: 39 distinct fully reviewed source paths out of the SDK estimate of 252; the remaining 213 paths were not reviewed, so no repository-wide clean conclusion is claimed.
- The checkout has no scripts directory, although Rust compile-time includes reference scripts/environment-helper.py, scripts/kali-executor.py, scripts/kali-bootstrap.py, scripts/kali-archive-inspect.py, and scripts/kali-restore-marker.py. The Linux bootstrap, executor, and archive helper behavior could not be reviewed.
- No Tauri config, capability, or permission files are present in the checkout; packaged WebView CSP and navigation restrictions could not be confirmed.
- The pinned @openai/codex-security implementation is not present in the reviewed source. Whether its child tools inherit or filter the provider API key remains unresolved.
- Root and reviewed nested SECURITY.md resolution returned no policy content. No knowledge-base file, target-path inventory, or reviewer feedback file was supplied.

### Scan Summary

| Field | Value |
| --- | --- |
| Scan outcome | completed |
| Reportable findings | 0 |
| Severity mix | none |
| Confidence mix | none |
| Validation mode | Manual source tracing and control/counterevidence review. No exploit was reproduced. |

Canonical artifacts: `scan-manifest.json` and `findings.json`. This report is a deterministic projection of those files.

## Threat Model

OpenKoda is a local Tauri desktop app. `src-tauri/src/lib.rs:71-82` defines the single Tauri `request` IPC command and checks that the caller is the `main` window; line 448 registers that command. Native services manage project data, provider credentials, provider requests, WSL/Kali work, and the local Codex Security runtime. The code uses Windows DPAPI for provider-key storage (`src-tauri/src/ai.rs:745-754`) and fixed provider endpoints (`src-tauri/src/ai_provider.rs:24-61`). The repository does not establish a public remote API. Untrusted project/evidence/repository content can enter local workflows, so the security review traced those paths to sensitive operations. The scanner key is passed to the local SDK runtime and set in its worker environment (`src-tauri/src/security_scan.rs:361-365`, `security-runtime/worker.mjs:35-40`); the downstream SDK child-process boundary is unavailable here.

### Assets

- Provider API credentials and their intended provider-use authority
- Local project, assessment, evidence, and workflow data
- Host files and managed WSL/Kali environment state
- Authority to issue provider requests or perform local engine operations

### Trust Boundaries

- Local WebView to Rust through Tauri IPC
- Rust services to local persistent storage and Windows DPAPI
- Rust process supervisors to WSL/Kali and the Node SDK runtime
- Security-scanner SDK tools to attacker-controlled repository content
- Application to fixed external AI-provider endpoints

### Attacker Capabilities

- Influence data submitted through the local WebView IPC or imported into project/evidence workflows
- Supply untrusted repository content that the local security scanner analyzes
- Interact with a lower-trust local workload only if a source-backed interface permits it

### Security Objectives

- Validate and authorize each IPC action against the exact values used by sensitive operations
- Keep credential bytes unavailable to the WebView and untrusted workflows except for intended provider requests
- Constrain engine process authority and treat uncertain termination as unresolved
- Prevent untrusted imported or returned content from crossing host and data boundaries

### Assumptions

- The application runs with the desktop user's local privileges.
- No external unauthenticated attacker path is assumed without source evidence.
- The checkout omits packaged Tauri configuration, embedded WSL helper scripts, and pinned SDK internals; corresponding deployment and child-process properties are unverified.

## Findings

### No findings

No findings were reported.
