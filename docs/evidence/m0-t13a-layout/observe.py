#!/usr/bin/env python3
"""Bounded macOS process-tree measurement wrapper; no gameplay simulation.
Usage: observe.py NEW_RUN_DIR -- MODEL_EXECUTABLE ARGS...
All model artifacts should be under NEW_RUN_DIR/model. Never restarts a run.
"""
import ctypes,gc,hashlib,json,os,platform,re,resource,selectors,signal,subprocess,sys,time,tracemalloc
from pathlib import Path
SDK=Path('/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/sys')
body=re.search(r'struct rusage_info_v4\s*\{(.*?)\};',(SDK/'resource.h').read_text(),re.S).group(1)
fields=[]
for line in body.splitlines():
    line=line.strip()
    if not line:continue
    m=re.fullmatch(r'uint(8|64)_t\s+(\w+)(?:\[(\d+)\])?;',line)
    if not m:raise RuntimeError('unknown rusage SDK field '+line)
    kind=ctypes.c_uint8 if m[1]=='8' else ctypes.c_uint64
    fields.append((m[2],kind*int(m[3]) if m[3] else kind))
class Usage(ctypes.Structure):_fields_=fields
class Timebase(ctypes.Structure):_fields_=[('numer',ctypes.c_uint32),('denom',ctypes.c_uint32)]
class Swap(ctypes.Structure):_fields_=[('total',ctypes.c_uint64),('available',ctypes.c_uint64),('used',ctypes.c_uint64),('page_size',ctypes.c_uint32),('encrypted',ctypes.c_int)]
lib=ctypes.CDLL('/usr/lib/libproc.dylib',use_errno=True)
lib.proc_pid_rusage.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_void_p];lib.proc_pid_rusage.restype=ctypes.c_int
system=ctypes.CDLL('/usr/lib/libSystem.B.dylib',use_errno=True)
system.mach_timebase_info.argtypes=[ctypes.POINTER(Timebase)];system.mach_timebase_info.restype=ctypes.c_int
system.sysctlbyname.argtypes=[ctypes.c_char_p,ctypes.c_void_p,ctypes.POINTER(ctypes.c_size_t),ctypes.c_void_p,ctypes.c_size_t];system.sysctlbyname.restype=ctypes.c_int
base=Timebase();assert system.mach_timebase_info(ctypes.byref(base))==0 and base.denom

def file_sha(path):
    h=hashlib.sha256()
    with open(path,'rb') as f:
        while True:
            b=f.read(65536)
            if not b:break
            h.update(b)
    return h.hexdigest()

def sysctl(name,value):
    size=ctypes.c_size_t(ctypes.sizeof(value))
    if system.sysctlbyname(name.encode(),ctypes.byref(value),ctypes.byref(size),None,0):raise OSError(ctypes.get_errno(),name)
    if size.value!=ctypes.sizeof(value):raise RuntimeError('sysctl SDK size '+name)
    return value

def sysctl_text(name):
    n=ctypes.c_size_t()
    if system.sysctlbyname(name.encode(),None,ctypes.byref(n),None,0) or n.value>1024:raise RuntimeError('sysctl text size '+name)
    value=ctypes.create_string_buffer(n.value)
    if system.sysctlbyname(name.encode(),value,ctypes.byref(n),None,0):raise OSError(ctypes.get_errno(),name)
    return value.value.decode()

def usage(pid):
    v=Usage()
    if lib.proc_pid_rusage(pid,4,ctypes.byref(v)):return None
    return {'pid':pid,'start_abstime':v.ri_proc_start_abstime,'rss_bytes':v.ri_resident_size,
        'phys_footprint_bytes':v.ri_phys_footprint,'user_time_ns':v.ri_user_time*base.numer//base.denom,
        'system_time_ns':v.ri_system_time*base.numer//base.denom,'disk_bytes_read':v.ri_diskio_bytesread,'disk_bytes_written':v.ri_diskio_byteswritten}

def ps():
    child=subprocess.Popen(['/bin/ps','-axo','pid=,ppid=,rss=,stat=,comm='],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
    # Capture this short helper while alive; don't retain it for future frames.
    helper=usage(child.pid);raw=bytearray();deadline=time.monotonic()+2
    selector=selectors.DefaultSelector();selector.register(child.stdout,selectors.EVENT_READ)
    try:
        while True:
            if time.monotonic()>deadline:raise RuntimeError('observer ps timeout')
            if not selector.select(min(0.1,max(0,deadline-time.monotonic()))):continue
            b=os.read(child.stdout.fileno(),65536)
            if not b:break
            raw.extend(b)
            if len(raw)>1024*1024:raise RuntimeError('observer ps raw cap')
        if child.wait(timeout=1)!=0:raise RuntimeError('observer ps exit')
    finally:
        selector.close();child.stdout.close()
        if child.poll() is None:child.kill();child.wait(timeout=2)
    rows={}
    for line in raw.splitlines():
        parts=line.split(None,4)
        if len(parts)!=5 or len(rows)>=8192:raise RuntimeError('observer ps row limit')
        pid,ppid,rss=map(int,parts[:3]);rows[pid]=(ppid,rss*1024,parts[3].decode(),parts[4].decode(errors='replace'))
    return child.pid,helper,rows

def bounded_command(args):
    child=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    before=usage(child.pid)
    raw,error=child.communicate(timeout=3)
    if child.returncode or len(raw)>32768:raise RuntimeError('observer system command '+str(args))
    return {'command':args,'pid':child.pid,'captured_usage':before,'stdout':raw.decode(),'short_helper_peak_not_proven':True}

def disk_bytes(artifacts_root):
    total=0;files=0
    for current,dirs,names in os.walk(artifacts_root):
        for name in dirs+names:
            p=Path(current)/name
            if p.is_symlink():raise RuntimeError('observer artifact symlink')
        for name in names:
            try:total+=(Path(current)/name).stat().st_size
            except FileNotFoundError:continue
            files+=1
            if files>4096:raise RuntimeError('observer disk file cap')
    return total

def pressure():
    # sysctl returns dispatch flags (Normal=1), NOT XNU internal enum Normal=0.
    # Apple kern_memorystatus_notify.c converts before SYSCTL_OUT.
    return sysctl('kern.memorystatus_vm_pressure_level',ctypes.c_uint32()).value

def main():
    if len(sys.argv)<5 or '--' not in sys.argv:raise SystemExit(__doc__)
    split=sys.argv.index('--');flags=sys.argv[2:split];command=sys.argv[split+1:]
    root=Path(sys.argv[1]);root.mkdir();artifacts_root=root
    if flags:
        if len(flags)!=2 or flags[0]!='--artifacts-root':raise SystemExit(__doc__)
        artifacts_root=Path(flags[1]).resolve(strict=True)
    tracemalloc.start()
    samples=open(root/'host-samples.jsonl','wb',buffering=65536)
    observer_source_before=file_sha(Path(__file__));start=time.monotonic();known={};frames=0;log_bytes=0;retired=0;identities=0;max_active=0
    peak_rss=0;peak_footprint=0;peak_rss_frame=None;peak_footprint_frame=None;gap=0;last=None;max_window=0
    max_observer_hot=0;max_other_hot=0;max_total_hot=0;disk_peak=0;disk_warned=False;sampled_missing=0;swap_initial=sysctl('vm.swapusage',Swap()).used;swap_max=swap_initial
    pressure_start=None;pressure_last_poll=start;pressure_max=0;last_system=-60;next_tick=0;status='running';reason=None;post_exit=0
    executable=Path(command[0]).resolve();binary_before=file_sha(executable)
    with open(root/'model.stdout','wb') as out,open(root/'model.stderr','wb') as error:
        child=subprocess.Popen(command,stdout=out,stderr=error,start_new_session=True)
        try:
            while True:
                frame_start=time.monotonic();elapsed=frame_start-start
                # Retire Python/ctypes/subprocess cycles on a bounded schedule.
                # Collection time remains inside the measured capture window.
                if frames%16==0:gc.collect()
                rows=None  # Release the previous full host process table before building the next.
                helper_pid,helper,rows=ps();selected={os.getpid()}
                if child.poll() is None:selected.add(child.pid)
                # Keep an already observed orphan only while its actual Mach birth matches.
                for pid,birth in list(known.items()):
                    actual=usage(pid) if pid in rows else None
                    if actual and actual['start_abstime']==birth:selected.add(pid)
                    else:del known[pid];retired+=1
                while True:
                    old=len(selected)
                    for pid,row in rows.items():
                        if row[0] in selected:selected.add(pid)
                    if len(selected)==old:break
                selected.add(helper_pid);members=[];detail=[];rss=0;footprint=0;missing=0
                for pid in sorted(selected):
                    if pid not in rows and pid!=helper_pid:continue
                    row=rows.get(pid,(os.getpid(),0,'helper','ps'))
                    actual=helper if pid==helper_pid else usage(pid)
                    birth=actual['start_abstime'] if actual else None
                    role='observer' if pid==os.getpid() else ('sampler_ps' if pid==helper_pid else Path(row[3]).name)
                    rss_value=row[1] if pid==helper_pid and row[1] else (actual['rss_bytes'] if actual else row[1])
                    rss+=rss_value
                    if actual:footprint+=actual['phys_footprint_bytes']
                    else:missing+=1
                    members.append([pid,row[0],birth,rss_value,role])
                    if actual:detail.append(dict(actual,role=role))
                    if pid!=helper_pid and actual and known.get(pid)!=birth:
                        known[pid]=birth;identities+=1
                if len(known)>512:raise RuntimeError('observer live identity cap')
                sampled_missing+=missing;max_active=max(max_active,len(known))
                now=time.monotonic();elapsed=now-start
                if last is not None:gap=max(gap,now-last)
                last=now;max_window=max(max_window,now-frame_start)
                level=pressure()
                if level!=1:
                    if pressure_start is None:pressure_start=pressure_last_poll
                    pressure_max=max(pressure_max,now-pressure_start)
                else:
                    if pressure_start is not None:pressure_max=max(pressure_max,now-pressure_start)
                    pressure_start=None
                pressure_last_poll=now
                swap=sysctl('vm.swapusage',Swap());swap_max=max(swap_max,swap.used)
                model_manifest=root/'model'/'manifest.json';other_hot=0
                if model_manifest.exists():
                    raw=model_manifest.read_bytes()
                    if len(raw)>65536:raise RuntimeError('observer manifest bound')
                    other_hot=int(json.loads(raw).get('external_owned_charge_bytes',0))
                current,peak=tracemalloc.get_traced_memory();trace_overhead=tracemalloc.get_tracemalloc_memory();observer_hot=peak+trace_overhead
                max_observer_hot=max(max_observer_hot,observer_hot);max_other_hot=max(max_other_hot,other_hot);max_total_hot=max(max_total_hot,observer_hot+other_hot)
                # 2MiB observer hot data fits the L1's 4MiB metadata/IO allowance;
                # also enforce the complete 8MiB diagnostic envelope with Rust metrics.
                if observer_hot>2*1024**2 or observer_hot+other_hot>8*1024**2:raise RuntimeError('observer hot-data bound current='+str(current)+' peak='+str(peak)+' tracer='+str(trace_overhead))
                frame={'t':elapsed,'window_s':now-frame_start,'rss_bytes':rss,'phys_footprint_bytes':footprint,'missing_footprint':missing,'members':members,'pressure_dispatch_flag':level,'swap_used_bytes':swap.used,'observer_hot_bound_bytes':observer_hot,'observer_traced_current_bytes':current,'observer_traced_peak_bytes':peak,'observer_trace_metadata_bytes':trace_overhead}
                current_disk=disk_bytes(artifacts_root)
                if not root.resolve().is_relative_to(artifacts_root.resolve()):current_disk+=disk_bytes(root)
                disk_peak=max(disk_peak,current_disk);disk_warned=disk_warned or current_disk>=16*1024**3
                if current_disk>20*1024**3:raise RuntimeError('world artifact bytes exceeded 20GiB')
                if elapsed-last_system>=60:
                    frame['cpu_io_footprint']=detail
                    frame['system']={'vm_stat':bounded_command(['/usr/bin/vm_stat']),'physical_memory_bytes':sysctl('hw.memsize',ctypes.c_uint64()).value,'disk_bytes':disk_bytes(artifacts_root),'available_disk_bytes':os.statvfs(root).f_bavail*os.statvfs(root).f_frsize,'swap_total_bytes':swap.total}
                    last_system=elapsed
                if rss>peak_rss:peak_rss=rss;peak_rss_frame=frame.copy()
                if footprint>peak_footprint:peak_footprint=footprint;peak_footprint_frame=frame.copy()
                encoded=json.dumps(frame,separators=(',',':')).encode()+b'\n'
                latency_bytes=sum(p.stat().st_size for p in artifacts_root.glob('**/compute-durations.bin'))
                if log_bytes+len(encoded)+latency_bytes>256*1024**2:raise RuntimeError('combined measurement-log cap')
                samples.write(encoded);samples.flush();log_bytes+=len(encoded);frames+=1
                if pressure_max>60:raise RuntimeError('system non-normal pressure exceeded 60 seconds')
                if swap.used-swap_initial>256*1024**2:raise RuntimeError('system swap net growth exceeded 256MiB')
                available=os.statvfs(root).f_bavail*os.statvfs(root).f_frsize
                if available<2*1024**3:raise RuntimeError('actual free disk below 2GiB safety floor')
                if child.poll() is not None:
                    post_exit+=1
                    if post_exit>=3:
                        if any(pid!=os.getpid() for pid in known):raise RuntimeError('model exited with live tracked descendants')
                        break
                next_tick+=0.25;time.sleep(max(0,min(0.25,start+next_tick-time.monotonic())))
            status='passed_measurement_wrapper' if child.returncode==0 else 'model_failed'
        except Exception as e:
            status='failed';reason=str(e)
            for pid,birth in list(known.items()):
                if pid==os.getpid():continue
                actual=usage(pid)
                if actual and actual['start_abstime']==birth:
                    try:os.kill(pid,signal.SIGTERM)
                    except (ProcessLookupError,PermissionError):pass
            if child.poll() is None:
                try:os.killpg(child.pid,signal.SIGTERM)
                except (ProcessLookupError,PermissionError) as cleanup_error:reason+='; cleanup: '+str(cleanup_error)
                try:child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    try:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=5)
                    except (ProcessLookupError,PermissionError,subprocess.TimeoutExpired) as cleanup_error:reason+='; cleanup: '+str(cleanup_error)
        finally:
            samples.flush();os.fsync(samples.fileno());samples.close()
    final_usage=resource.getrusage(resource.RUSAGE_CHILDREN)
    report={'schema':'mycity-bounded-group-observer-v1','host':{'hardware_model':sysctl_text('hw.model'),'cpu':sysctl_text('machdep.cpu.brand_string'),'logical_cpus':sysctl('hw.logicalcpu',ctypes.c_uint32()).value,'physical_memory_bytes':sysctl('hw.memsize',ctypes.c_uint64()).value,'os':platform.mac_ver()[0],'architecture':platform.machine()},'observer_source_sha256':observer_source_before,'observer_source_unchanged':file_sha(Path(__file__))==observer_source_before,'status':status,'reason':reason,'command':command,'artifacts_root':str(artifacts_root),'model_exit_code':child.returncode,'actual_wall_seconds':time.monotonic()-start,'samples':frames,'sample_log_bytes':log_bytes,'maximum_sample_gap_seconds':gap,'maximum_capture_window_seconds':max_window,'peak_simultaneous_rss_bytes':peak_rss,'peak_simultaneous_footprint_bytes':peak_footprint,'peak_rss_frame':peak_rss_frame,'peak_footprint_frame':peak_footprint_frame,'missing_footprint_process_samples':sampled_missing,'peak_world_logical_bytes':disk_peak,'disk_warning_16gib_observed':disk_warned,'maximum_live_tracked_identities':max_active,'identities_recorded':identities,'identities_retired':retired,'final_live_identity_count':len(known),'observer_hot_bound_bytes':max_observer_hot,'other_diagnostic_owned_bytes':max_other_hot,'combined_diagnostic_hot_bound_bytes':max_total_hot,'swap_initial_bytes':swap_initial,'swap_max_bytes':swap_max,'maximum_observed_swap_net_growth_bytes':max(0,swap_max-swap_initial),'maximum_sampled_non_normal_pressure_seconds':pressure_max,'pressure_duration_definition':'conservative interval from preceding normal poll through next observed normal poll','pressure_semantics':'sysctl returns dispatch flags; normal=1, warning=2, critical=4, not internal enum 0..3','mach_timebase':{'numer':base.numer,'denom':base.denom},'sdk_resource_header_sha256':hashlib.sha256((SDK/'resource.h').read_bytes()).hexdigest(),'sdk_sysctl_header_sha256':hashlib.sha256((SDK/'sysctl.h').read_bytes()).hexdigest(),'binary_sha256':binary_before,'binary_unchanged':file_sha(executable)==binary_before,'cpu_children_at_end':{'user_seconds':final_usage.ru_utime,'system_seconds':final_usage.ru_stime},'short_helper_peak_completeness':'not_proven; actual helper sample windows and misses retained','sampler_scope':'observer + model process tree + observed orphans by PID/Mach birth; helper identities retired, not accumulated','memory_16gb_gate':'not_run','eight_hour_gate':'not_run','full_M0':'not_accepted'}
    (root/'observer-report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if not k.endswith('_frame')},indent=2))
    if status!='passed_measurement_wrapper':raise SystemExit(1)
if __name__=='__main__':main()
