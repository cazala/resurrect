# Reference deployment files

These systemd units reproduce the hosted Resurrect seed topology described in
[`docs/hosted-services.md`](../docs/hosted-services.md). They contain public
configuration only.

Before enabling them, create these root-owned files with mode `0600`:

- `/etc/resurrect/seed.env`, containing `RESURRECT_RPC_URL` and
  `RESURRECT_ETHEREUM_PRIVATE_KEY`; and
- `/etc/cloudflared/token`, containing the dedicated remotely managed Tunnel
  token.

Install a verified `resurrect-node` release binary at
`/usr/local/bin/resurrect-node`, install `cloudflared` at
`/usr/bin/cloudflared`, copy the units into `/etc/systemd/system`, reload
systemd, and enable both units. Do not copy secrets into this directory or the
unit definitions.

The Cloudflare Tunnel configuration is remote-managed. Its first ingress maps
`resurrect-ws.caza.la` to `http://127.0.0.1:4002`; the final ingress returns
HTTP 404. DNS uses a proxied CNAME to the dedicated tunnel target. Routine
configuration and validation are documented in the hosted-services runbook.
