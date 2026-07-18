# Daemon Protocol v2 and Process Ownership

Wayscriber publishes daemon protocol v2 as one closed production mode. The CLI, config,
environment, runtime files, and incoming requests cannot select a protocol implementation. The
legacy implementation remains only for mixed-version parsing, the independently compiled v1 test
fixture, the empty-request `SIGUSR1` visibility fallback, and a source-level rollback compatibility
selector.

## Route map

| Route | Publication and wake | Durable owner | Terminal proof |
|---|---|---|---|
| Typed CLI request | Atomic rename into `commands/v2/queue` and inotify | `ClientCommand` plus caller lease | Typed response digest and acknowledgement |
| Empty legacy visibility request | `SIGUSR1` only | Daemon visibility intent | Current-generation child state |
| Tray/global shortcut | PID-free bounded intent plus eventfd wake | Daemon controller | Controller transition result |
| Hide ready overlay | Committed command decision | Daemon child owner | Broker wait/reap result |
| Show stopped overlay | Committed command decision | Generation and pidfd child owner | Readiness record matching PID start identity |
| Ready-overlay action | Eligible ordered action journal entry plus `SIGUSR2` | Overlay event-loop action applier | Applied/abandoned journal state and typed response |
| Start-and-deliver action | Prepared action made eligible by command commit | Command control plus action journal | Child application or explicit indeterminate result |
| Anonymous tray action | Eligible ordered action journal entry | Action journal | Applied/abandoned tombstone |
| Runtime helper | Bounded `SOCK_SEQPACKET` broker request | Pre-lock process broker | Exit status, timeout, explicit output-overflow failure, or exec acceptance |
| Restart recovery | Capped queue/control/journal scans | New daemon generation | Prior open work rejected; committed work preserved as indeterminate |
| Terminal collection | Admission, decision, and caller-lease locks | Command GC owner | Response, disposition, reconciliation, report, and lease predicates |

The runtime record carries protocol versions, boot identity, time/PID namespace identities,
process start ticks, and a CSPRNG instance token. Clients validate that identity against procfs and
a pidfd before publishing work. Canonical JSON records reject unknown fields, non-canonical
encoding, symlinks, oversized inputs, and cross-generation identities. Records containing v2
discriminator fields never fall back to the permissive legacy parser when strict v2 parsing fails.

## Process ownership

Daemon and active-overlay runs create an authenticated process broker before acquiring a
Wayscriber singleton lock. Runtime code uses closed helper kinds with bounded arguments,
environment changes, input, output, and deadlines. The broker alone creates and reaps runtime
children. The daemon retains an opaque broker handle, generation, display-only PID, and pidfd for
each overlay child; tray and shortcut producers never carry a PID.

Each broker also owns an out-of-band shutdown socket. Closing or signaling that channel preempts
an active bounded helper, kills its process group, and reaps owned children without waiting for the
request/response exchange lock. The broker runs in its own process group so terminating an overlay
group leaves the broker alive long enough to perform that cleanup. Complete helper output that
exceeds its declared cap fails the operation; the explicit `wl-paste` prefix mode instead returns a
bounded sample and stops its helper as soon as the requested prefix is complete. Screenshot helpers
use a larger but still bounded 256 MiB cap so large valid compositor captures are not returned as
truncated PNG data. Retained `wl-copy` publication uses that same cap, and both its input writer and
retained provider group are cancelled when broker shutdown begins.

Overlay startup uses a CSPRNG generation passed only to the candidate. After the candidate wins the
overlay singleton lock it publishes a private readiness record containing its generation, PID, and
process-start identity. The daemon does not mark that child ready until all three match. Overlay
exit is watched through the pidfd, while signals, tray intents, shortcut intents, and typed queue
renames have owned wake descriptors; the daemon lifecycle has no periodic discovery tick.

The enforced process-site inventory is `tools/check-process-sites.py`. Direct process creation is
limited to the broker, pre-runtime systemd setup, the separate configurator process, standalone
About clipboard integration, and named test fixtures. The same check audits the raw-clone child
stub: before `execve` it may reach only the fixed `fcntl`, `dup3`, `setpgid`, `close_range`,
`execve`, and `exit_group` syscall set over prebuilt buffers.

## Compatibility and rollback

- A v2 client against a v1 daemon uses the strict legacy parser and v1 request path.
- A frozen v1 client against a v2 daemon rejects typed requests because no v1 token exists. Its
  explicitly empty visibility signal remains supported.
- V1 cleanup removes only exact v1 request/response artifacts and never recursively removes the v2
  root.
- Restart recovery rejects prior-generation open commands with a durable no-effect response and
  records authorized commands without terminal proof as indeterminate. Foreign-generation journal
  entries are abandoned rather than replayed.
- Rollback is a source-level release decision using the v2-aware compatibility selector. It is not
  user-selectable and must preserve the complete versioned v2 tree; an arbitrary pre-v2 binary is
  not a supported rollback target.
