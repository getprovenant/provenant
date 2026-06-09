# Security Policy

## Supported Versions

Security fixes are targeted at the latest released version and the current `main` branch.

## Reporting a Vulnerability

Please do not disclose suspected security issues in public issues, discussions, or pull requests first.

- Prefer GitHub's private vulnerability reporting flow or a GitHub Security Advisory draft when that option is available on the repository.
- If a private reporting path is not available, open a minimal public issue that requests a secure contact channel without including exploit details, proof-of-concept code, or sensitive target information.

When reporting, include as much of the following as you can:

- affected version or commit
- operating system and environment details
- reproduction steps or a minimized sample
- expected behavior vs. actual behavior
- impact assessment if known

We will triage reports as quickly as practical and coordinate on disclosure timing for confirmed vulnerabilities.

## Serve Mode Security Posture

`provenant serve` exposes the scanner over HTTP. It has **no built-in authentication or authorization** by design: any client that can reach the listening port can submit scans and read results. The current service has no concept of users, API keys, or per-request credentials.

Access control is delegated entirely to where and how you bind the service:

- **Default loopback bind.** `provenant serve` binds to `127.0.0.1:8080` by default. On a loopback bind the service is only reachable from the same host, which is the intended deployment for same-host use.
- **Privileged-input gate.** Local-path, remote-URL, and repository inputs are _privileged inputs_: they make the service host read local files, fetch URLs, or run `git fetch` on behalf of the caller. On a loopback bind these are enabled. When the service is bound beyond loopback, they are **disabled** unless the operator explicitly passes `--allow-privileged-inputs`. Upload input always remains available because the caller supplies the content to scan rather than directing the host to access something.

Because there is no request-level authn/authz, the bind address and the privileged-input gate are the _only_ protections the service provides on its own.

### Operator guidance

- Treat any non-loopback `provenant serve` deployment as unauthenticated and place it behind your own access controls: a reverse proxy that enforces authentication, a network policy, a firewall, or an equivalent perimeter control.
- Use `--allow-privileged-inputs` only for trusted deployments that already have their own network access controls. Enabling it on an exposed bind lets any reachable caller make the host read local paths, fetch arbitrary HTTP(S) URLs, and clone arbitrary repositories. Treat repository and URL ingestion as sensitive, server-side-effecting operations.
- Do not expose the privileged-input modes to untrusted callers. If callers are not already trusted to perform host-side filesystem, network, and `git` access, prefer upload input on a restricted bind.

The service does enforce bounded request, upload, remote-download, and archive-extraction sizes, but these are denial-of-service mitigations, not access control.

### Future work

An optional bearer-token (or similar request-credential) gate for deployments that expose the port has been considered but is intentionally **not implemented** today. Until such a feature exists, operators must rely on the bind address, the privileged-input gate, and an external perimeter as described above.

See the [Serve API Guide](docs/SERVE_API_GUIDE.md#security-posture) for the concrete bind, input-mode, and `--allow-privileged-inputs` behavior this posture is built on.
