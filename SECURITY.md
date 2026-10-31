# Security policy

## Reporting a vulnerability

Please do not open a public issue for a security problem. Use
[GitHub private vulnerability reporting](https://github.com/matthart1983/kernwatch/security/advisories/new).

You should get an acknowledgement within 72 hours and a triage verdict within 7
days. Please allow time for a fix before public disclosure. Reports are credited
in the advisory and release notes unless you prefer otherwise.

## Supported versions

kernwatch is pre-1.0. Fixes target the latest release only, so please reproduce
on it before reporting.

## What matters most

kernwatch is often run as root or with BPF capabilities, and it reads names that
other users on the machine control.

1. Anything that lets an unprivileged local user use kernwatch's privileges,
   including its BPF probes and its task, IRQ, RPS and cgroup changes.
2. A host change applied without its preview, target identity check or journal
   entry.
3. Terminal escape injection from process, thread, module, cgroup, unit or journal
   text, in the terminal or in exported reports.
4. Crashes or unbounded memory use from a crafted `.kwr` recording, baseline state
   file or kernel-visible name.
5. Writes outside the report and state directories.

kernwatch does not elevate itself. A user who already has root on the machine is
outside this policy.
