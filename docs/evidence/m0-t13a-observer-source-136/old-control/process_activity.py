"""Identify active cargo/rustc processes without signaling or stopping them."""
import os
import subprocess

COMPILER_NAMES = {"cargo", "rustc", "rustc_driver"}


def is_compiler(comm, args=""):
    name = os.path.basename(comm.strip())
    if name in COMPILER_NAMES:
        return True
    first = args.strip().split(maxsplit=1)[0] if args.strip() else ""
    return os.path.basename(first) in COMPILER_NAMES


def parse_process_listing(output, exclude_pids=()):
    excluded = {int(pid) for pid in exclude_pids}
    active = []
    for line in output.splitlines():
        fields = line.strip().split(maxsplit=3)
        if len(fields) < 3:
            continue
        try:
            pid, ppid = int(fields[0]), int(fields[1])
        except ValueError:
            continue
        comm = fields[2]
        args = fields[3] if len(fields) == 4 else comm
        if pid not in excluded and is_compiler(comm, args):
            active.append({"pid": pid, "ppid": ppid, "comm": comm, "args": args[:512]})
    return active


def active_compilers():
    output = subprocess.check_output(
        ["/bin/ps", "-axo", "pid=,ppid=,comm=,args="], text=True
    )
    return parse_process_listing(output, exclude_pids=(os.getpid(),))


def observer_compilers(process_rows, path_lookup):
    """Resolve only truncated/path-like names using libproc; spawn no sampler child."""
    active = []
    for pid, row in process_rows.items():
        if pid == os.getpid():
            continue
        comm = row[3]
        path = comm if is_compiler(comm) else None
        if path is None and (comm.startswith("/") or len(comm.encode()) >= 16):
            path = path_lookup(pid)
        if path and is_compiler(path):
            active.append({"pid": pid, "ppid": row[0], "comm": comm, "executable_path": path})
    return active
