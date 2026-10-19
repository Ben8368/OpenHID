---
paths:
  - "crates/openhid-agent-core/**"
  - "crates/openhid-agent/**"
  - "crates/openhid-core/**"
  - "crates/openhid-hid/**"
---

# Serde types here ride the IPC wire

The append-only wire-format contract — method order, enum variant order,
`PROTOCOL_VERSION`, the golden tests — lives with the contract itself in
`crates/openhid-ipc/AGENTS.md`. Read it before changing any serde type that
crosses the agent↔GUI boundary.
