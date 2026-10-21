"""Candidate macOS native metrics. Not installed into the active measurement.
ABI comes from local public SDK; fail closed on unknown fields or truncation.
Removes per-frame ps and periodic vm_stat child processes, not model children.
"""
import ctypes,errno,hashlib,os,re
from pathlib import Path
SDK=Path('/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include')
HEADERS=['libproc.h','sys/proc_info.h','sys/param.h','mach/vm_statistics.h','mach/host_info.h']
def constant(header,name):
 m=re.search(r'^#define\s+'+re.escape(name)+r'\s+(\d+)\b',(SDK/header).read_text(),re.M)
 if m is None:raise RuntimeError('missing public SDK constant '+name)
 return int(m[1])
def fields(header,name,types):
 text=(SDK/header).read_text();m=re.search(r'struct\s+'+name+r'\s*\{(.*?)\}',text,re.S)
 if m is None:raise RuntimeError('missing SDK struct '+name)
 body=re.sub(r'/\*.*?\*/','',m[1],flags=re.S);out=[]
 for line in body.splitlines():
  line=line.strip()
  if not line:continue
  f=re.fullmatch(r'(\w+)\s+(\w+)(?:\[(\w+)\])?;',line)
  if f is None or f[1] not in types:raise RuntimeError('unsupported SDK field '+line)
  kind=types[f[1]]
  if f[3]:
   if f[3]!='MAXCOMLEN':raise RuntimeError('unknown SDK array '+f[3])
   kind=kind*constant('sys/param.h','MAXCOMLEN')
  out.append((f[2],kind))
 return out
class BSDShort(ctypes.Structure):
 _fields_=fields('sys/proc_info.h','proc_bsdshortinfo',{'uint32_t':ctypes.c_uint32,'char':ctypes.c_char,'uid_t':ctypes.c_uint32,'gid_t':ctypes.c_uint32})
class Task(ctypes.Structure):
 _fields_=fields('sys/proc_info.h','proc_taskinfo',{'uint64_t':ctypes.c_uint64,'int32_t':ctypes.c_int32})
class VM(ctypes.Structure):
 _fields_=fields('mach/vm_statistics.h','vm_statistics64',{'natural_t':ctypes.c_uint32,'uint64_t':ctypes.c_uint64})
lib=ctypes.CDLL('/usr/lib/libproc.dylib',use_errno=True)
lib.proc_listpids.argtypes=[ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_int];lib.proc_listpids.restype=ctypes.c_int
lib.proc_pidinfo.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_uint64,ctypes.c_void_p,ctypes.c_int];lib.proc_pidinfo.restype=ctypes.c_int
system=ctypes.CDLL('/usr/lib/libSystem.B.dylib',use_errno=True)
system.mach_host_self.argtypes=[];system.mach_host_self.restype=ctypes.c_uint32
system.host_statistics64.argtypes=[ctypes.c_uint32,ctypes.c_int,ctypes.c_void_p,ctypes.POINTER(ctypes.c_uint32)];system.host_statistics64.restype=ctypes.c_int
system.mach_port_deallocate.argtypes=[ctypes.c_uint32,ctypes.c_uint32];system.mach_port_deallocate.restype=ctypes.c_int
ALL=constant('sys/proc_info.h','PROC_ALL_PIDS');SHORT=constant('sys/proc_info.h','PROC_PIDT_SHORTBSDINFO');TASK=constant('sys/proc_info.h','PROC_PIDTASKINFO');VM_INFO=constant('mach/host_info.h','HOST_VM_INFO64')
PID_CAPACITY=8192
# Explicit conservative charge for retained SDK-derived class/field/control metadata.
# Per-capture PID array/rows are allocated only while observer tracemalloc is active.
FIXED_CONTROL_CHARGE_BYTES=65536
LAST_ENUMERATION_FAILURES=[]
LAST_ENUMERATED_PIDS=set()

def confirmed_absent(pid):
 try:os.kill(pid,0)
 except ProcessLookupError:return True
 except PermissionError:return False
 return False

def process_table():
 global LAST_ENUMERATION_FAILURES,LAST_ENUMERATED_PIDS
 pids=(ctypes.c_uint32*PID_CAPACITY)();capacity=ctypes.sizeof(pids)
 used=lib.proc_listpids(ALL,0,pids,capacity)
 if used<=0 or used>=capacity or used%4:raise RuntimeError('native PID list failed/truncated '+str(used))
 rows={};short=BSDShort();failures=[];enumerated=set()
 for pid in pids[:used//4]:
  if not pid:continue
  enumerated.add(int(pid));ctypes.set_errno(0)
  got=lib.proc_pidinfo(pid,SHORT,0,ctypes.byref(short),ctypes.sizeof(short))
  if got==0:
   code=ctypes.get_errno()
   failures.append({'pid':int(pid),'errno':code,'error':errno.errorcode.get(code,'UNKNOWN_OR_EMPTY'),'confirmed_absent_after_query':confirmed_absent(pid),'ancestry':'unresolved; cannot silently remove from group completeness'})
   continue
  if got!=ctypes.sizeof(short) or short.pbsi_pid!=pid:raise RuntimeError('native BSD ABI/identity mismatch')
  name=bytes(short.pbsi_comm).split(b'\0',1)[0].decode(errors='replace')
  rows[int(pid)]=(int(short.pbsi_ppid),None,str(short.pbsi_status),name)
 if os.getpid() not in rows:raise RuntimeError('native enum omitted live observer')
 LAST_ENUMERATION_FAILURES=failures;LAST_ENUMERATED_PIDS=enumerated
 return rows

def relation(pid):
 short=BSDShort();got=lib.proc_pidinfo(pid,SHORT,0,ctypes.byref(short),ctypes.sizeof(short))
 if got==0:return None
 if got!=ctypes.sizeof(short) or short.pbsi_pid!=pid:raise RuntimeError('native relation ABI/identity mismatch')
 return (int(short.pbsi_ppid),None,str(short.pbsi_status),bytes(short.pbsi_comm).split(b'\0',1)[0].decode(errors='replace'))

def resident_size(pid):
 value=Task();got=lib.proc_pidinfo(pid,TASK,0,ctypes.byref(value),ctypes.sizeof(value))
 if got==0:return None
 if got!=ctypes.sizeof(value):raise RuntimeError('native Task ABI mismatch')
 return int(value.pti_resident_size)

def vm_statistics():
 host=system.mach_host_self()
 if not host:raise RuntimeError('native null host port')
 try:
  value=VM();count=ctypes.c_uint32(ctypes.sizeof(value)//4)
  rc=system.host_statistics64(host,VM_INFO,ctypes.byref(value),ctypes.byref(count))
  if rc:raise RuntimeError('host_statistics64 '+str(rc))
  returned=count.value*4
  if returned>ctypes.sizeof(value):raise RuntimeError('native VM returned oversized structure')
  result={name:int(getattr(value,name)) for name,kind in VM._fields_ if getattr(VM,name).offset+ctypes.sizeof(kind)<=returned}
  for required in ['free_count','wire_count','compressor_page_count','compressions','decompressions','swapins','swapouts']:
   if required not in result:raise RuntimeError('native VM missing required field '+required)
  return {'method':'host_statistics64 HOST_VM_INFO64','returned_bytes':returned,'fields':result,'units':'page counts or cumulative events as named in SDK; page size separately recorded'}
 finally:
  task=ctypes.c_uint32.in_dll(system,'mach_task_self_').value
  if system.mach_port_deallocate(task,host):raise RuntimeError('native host port deallocation failed')

def provenance():
 return {'method':'public local SDK proc_listpids/proc_pidinfo and host_statistics64','headers_sha256':{n:hashlib.sha256((SDK/n).read_bytes()).hexdigest() for n in HEADERS},'struct_sizes':{'proc_bsdshortinfo':ctypes.sizeof(BSDShort),'proc_taskinfo':ctypes.sizeof(Task),'vm_statistics64':ctypes.sizeof(VM)},'pid_capacity':PID_CAPACITY,'fixed_control_charge_bytes':FIXED_CONTROL_CHARGE_BYTES,'no_sampler_child':True,'status':'candidate; requires actual ABI and live capture validation before formal use'}
