import ctypes,hashlib,json,os,subprocess,time
from pathlib import Path
import native_metrics as n
D=Path(__file__).resolve().parent;R=D.parents[3]
exe=R/'work/candidate-binaries/steady-controller/native-metrics-abi'
c=json.loads(subprocess.check_output([str(exe)],text=True))
p={'proc_bsdshortinfo':ctypes.sizeof(n.BSDShort),'proc_taskinfo':ctypes.sizeof(n.Task),'vm_statistics64':ctypes.sizeof(n.VM),'short_pid':n.BSDShort.pbsi_pid.offset,'short_ppid':n.BSDShort.pbsi_ppid.offset,'short_comm':n.BSDShort.pbsi_comm.offset,'task_resident':n.Task.pti_resident_size.offset,'vm_compressor':n.VM.compressor_page_count.offset,'vm_compressions':n.VM.compressions.offset,'vm_decompressions':n.VM.decompressions.offset,'vm_swapins':n.VM.swapins.offset,'vm_swapouts':n.VM.swapouts.offset,'vm_max_mem':n.VM.max_mem_count.offset,'host_vm_count':ctypes.sizeof(n.VM)//4}
assert c==p,(c,p)
started=time.monotonic_ns();table=n.process_table();ended=time.monotonic_ns();vm=n.vm_statistics()
children=[subprocess.Popen(['/bin/sleep','2']) for _ in range(3)]
try:
 rows=n.process_table();child_rows={x.pid:rows.get(x.pid) for x in children}
 assert all(v and v[0]==os.getpid() for v in child_rows.values()),child_rows
 observed={'pid':os.getpid(),'self_row':rows[os.getpid()],'self_resident_bytes':n.resident_size(os.getpid()),'child_rows':child_rows,'child_rss':{x.pid:n.resident_size(x.pid) for x in children},'enumeration_failures':n.LAST_ENUMERATION_FAILURES,'initial_process_count':len(table),'capture_ns':ended-started,'vm':vm,'abi_c':c,'abi_ctypes':p,'provenance':n.provenance(),'abi_exe_sha256':hashlib.sha256(exe.read_bytes()).hexdigest()}
finally:
 for child in children:child.terminate()
 for child in children:child.wait()
(D/'native-live-probe.json').write_text(json.dumps(observed,indent=2)+'\n');print(json.dumps(observed,indent=2))
