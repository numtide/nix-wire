# Recording sessions

How-to recipes for recording Nix daemon wire protocol sessions in various
configurations.

## Recording ssh-ng:// remote sessions

To record wire protocol sessions on a remote machine accessed via
`ssh-ng://`, configure Nix to use `nix-wire-record` as the remote program
wrapper:

```
nix build --store 'ssh-ng://user@host?remote-program=nix-wire-record -- nix-daemon' -f ...
```

This tells Nix to run `nix-wire-record -- nix-daemon --stdio` on the remote
host (Nix appends `--stdio` automatically). The recorder runs in command
mode, wrapping the daemon process and capturing the session.

Recordings are written to `/nix/var/nix/nix-wire/` on the remote machine by
default. To customize the output directory:

```
nix build --store 'ssh-ng://user@host?remote-program=nix-wire-record --output-dir /tmp/recordings -- nix-daemon'
```

Copy the recordings back and decode them locally:

```
scp 'user@host:/nix/var/nix/nix-wire/*.nixwire' ./recordings/
nix-wire-decode --recording ./recordings/0000.nixwire
```

Replaying against a remote daemon works the same way:

```
nix-wire-replay --recording 0000.nixwire -- ssh user@host nix-daemon --stdio
```

## Recording with a custom store root

If your Nix store is not at the default `/nix`, use the `--store` flag:

```
nix-wire-record --store /custom/nix
```

The recorder derives the socket path (`<store>/var/nix/daemon-socket/socket`)
and the default output directory (`<store>/var/nix/nix-wire/`) from this
root.

## Managing recording output

### Default output directory

Recordings are written to `<store>/var/nix/nix-wire/` by default (typically
`/nix/var/nix/nix-wire/`).

### Custom output directory

Override the output directory with `--output-dir`:

```
nix-wire-record --output-dir /tmp/my-recordings
```

### File naming

Recording files are named `NNNN.nixwire` with a zero-padded sequence number
(minimum 4 digits). The recorder scans the output directory for existing
files and picks the next available number. Concurrent sessions use an atomic
counter to avoid collisions within the same recorder process.

Examples: `0000.nixwire`, `0001.nixwire`, `0042.nixwire`.
