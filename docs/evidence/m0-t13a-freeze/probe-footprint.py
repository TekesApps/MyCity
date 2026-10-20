#!/usr/bin/env python3
"""Read-only macOS instrumentation feasibility, not a model/performance gate."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

header = Path('/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/sys/resource.h')
raw = header.read_bytes()
body = re.search(r'struct rusage_info_v4\s*\{(.*?)\};', raw.decode(), re.S).group(1)
fields = []
for line in body.splitlines():
    line = line.strip()
    if not line:
        continue
    m = re.fullmatch(r'uint(8|64)_t\s+(\w+)(?:\[(\d+)\])?;', line)
    if m is None:
        raise SystemExit('unknown SDK field: ' + line)
    kind = ctypes.c_uint8 if m[1] == '8' else ctypes.c_uint64
    fields.append((m[2], kind * int(m[3]) if m[3] else kind))

class Usage(ctypes.Structure):
    _fields_ = fields

lib = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
call = lib.proc_pid_rusage
call.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
call.restype = ctypes.c_int

def usage(pid):
    value = Usage()
    result = call(pid, 4, ctypes.byref(value))
    if result:
        raise OSError(ctypes.get_errno(), 'proc_pid_rusage')
    return {'pid': pid, 'rss_bytes': value.ri_resident_size,
            'phys_footprint_bytes': value.ri_phys_footprint,
            'lifetime_max_phys_footprint_bytes': value.ri_lifetime_max_phys_footprint,
            'user_time_raw': value.ri_user_time, 'system_time_raw': value.ri_system_time,
            'disk_bytes_read': value.ri_diskio_bytesread, 'disk_bytes_written': value.ri_diskio_byteswritten,
            'start_abstime': value.ri_proc_start_abstime}

class Timebase(ctypes.Structure):
    _fields_ = [('numer', ctypes.c_uint32), ('denom', ctypes.c_uint32)]

timebase = Timebase()
system = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
base_call = system.mach_timebase_info
base_call.argtypes = [ctypes.POINTER(Timebase)]
base_call.restype = ctypes.c_int
assert base_call(ctypes.byref(timebase)) == 0 and timebase.denom > 0
before = usage(os.getpid())
start_cpu = time.process_time_ns()
while time.process_time_ns() - start_cpu < 100_000_000:
    pass
end_cpu = time.process_time_ns()
after = usage(os.getpid())
raw_delta = (after['user_time_raw'] + after['system_time_raw']
             - before['user_time_raw'] - before['system_time_raw'])
converted_ns = raw_delta * timebase.numer // timebase.denom
assert abs(converted_ns - (end_cpu - start_cpu)) < 10_000_000, 'CPU clock units unresolved'

child = subprocess.Popen(['/bin/sleep', '2'])
try:
    time.sleep(0.1)
    rows = [usage(os.getpid()), usage(child.pid)]
finally:
    child.terminate()
    child.wait(timeout=3)
assert rows[0]['phys_footprint_bytes'] > 0
print(json.dumps({'scope': 'instrumentation_feasibility_only', 'sdk_header_sha256': hashlib.sha256(raw).hexdigest(),
                  'struct_bytes': ctypes.sizeof(Usage), 'rows': rows,
                  'mach_timebase': {'numer': timebase.numer, 'denom': timebase.denom},
                  'cpu_clock_calibration': {'rusage_raw_delta': raw_delta, 'converted_ns': converted_ns,
                                            'process_time_ns_delta': end_cpu - start_cpu},
                  'no_model_gate_claimed': True}, indent=2))
