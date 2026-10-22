"""Independent SDK ctypes oracle; not a formal performance run or memory credit."""
import ctypes,errno,hashlib,importlib.util,json,os,re,resource,subprocess,sys,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];P=D.parent/'m0-t13a-typed-workspace';sys.path.insert(0,str(P));os.chdir(R)
import native_metrics as native
spec=importlib.util.spec_from_file_location('old_observer',P/'observe.py');old=importlib.util.module_from_spec(spec);spec.loader.exec_module(old)
info=json.loads((D/'source.json').read_text());binary=info['binary'];W=R/'work/native-reader-probe-01';assert not W.exists();W.mkdir()
SDK=Path('/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include');header=(SDK/'sys/proc_info.h').read_text();body=re.search(r'struct\s+proc_bsdinfo\s*\{(.*?)\}',header,re.S).group(1);body=re.sub(r'/\*.*?\*/','',body,flags=re.S)
types={'uint32_t':ctypes.c_uint32,'uint64_t':ctypes.c_uint64,'int32_t':ctypes.c_int32,'uid_t':ctypes.c_uint32,'gid_t':ctypes.c_uint32,'char':ctypes.c_char};fields=[]
for line in body.splitlines():
 line=line.strip()
 if not line:continue
 m=re.fullmatch(r'(\w+)\s+(\w+)(?:\[([^\]]+)\])?;',line);assert m and m[1] in types,line;kind=types[m[1]]
 if m[3]:
  count=m[3].replace(' ','');assert count in ['MAXCOMLEN','2*MAXCOMLEN'];kind=kind*(native.constant('sys/param.h','MAXCOMLEN')*(2 if count.startswith('2') else 1))
 fields.append((m[2],kind))
class BSD(ctypes.Structure):_fields_=fields

def independent(pid):
 began=time.monotonic_ns();usage=old.Usage();ctypes.set_errno(0);ret=old.lib.proc_pid_rusage(pid,4,ctypes.byref(usage));e=ctypes.get_errno();bsd=BSD();ctypes.set_errno(0);got=native.lib.proc_pidinfo(pid,native.constant('sys/proc_info.h','PROC_PIDTBSDINFO'),0,ctypes.byref(bsd),ctypes.sizeof(bsd));be=ctypes.get_errno()
 return dict(begin=began,end=time.monotonic_ns(),rc=ret,errno=e,bsd_returned_bytes=got,bsd_errno=be,birth=usage.ri_proc_start_abstime,resident_bytes=usage.ri_resident_size,footprint_bytes=usage.ri_phys_footprint,user_time_raw=usage.ri_user_time,system_time_raw=usage.ri_system_time,parent_pid=bsd.pbi_ppid,pid=bsd.pbi_pid)

def start(name):
 p=subprocess.Popen([binary,'native-metrics-probe',str(W/name)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env={**os.environ,'MYCITY_HEAP_LOG':str(W/(name+'-heap.jsonl'))});ready=json.loads(p.stdout.readline());(D/(name+'-ready.json')).write_text(json.dumps(ready,indent=2)+'\n');return p,ready

def request(p,n,pid,expected):
 p.stdin.write(json.dumps(dict(sequence=n,pid=pid,expected=expected))+'\n');p.stdin.flush();return json.loads(p.stdout.readline())
p,ready=start('live');identity=ready['identity'];assert identity['pid']==p.pid
abi=ready['abi'];expected_abi={'rusage_size':ctypes.sizeof(old.Usage),'rusage_offsets':{name:getattr(old.Usage,name).offset for name in abi['rusage_offsets']},'bsd_size':ctypes.sizeof(BSD),'bsd_offsets':{name:getattr(BSD,name).offset for name in abi['bsd_offsets']}}
assert all(abi[k]==v for k,v in expected_abi.items());(D/'abi-oracle.json').write_text(json.dumps(dict(expected=expected_abi,actual=abi,sdk_sha256={n:hashlib.sha256((SDK/n).read_bytes()).hexdigest() for n in ['sys/proc_info.h','sys/resource.h','sys/param.h']},usage_fields=[(n,str(t)) for n,t in old.Usage._fields_],bsd_fields=[(n,str(t)) for n,t in BSD._fields_]),indent=2)+'\n')
rows=[];durations=[]
for n in range(1,241):
 before=independent(p.pid);reply=request(p,n,p.pid,identity);after=independent(p.pid);assert reply['unknown'] is None;sample=reply['sample'];u=sample['usage'];assert sample['identity']==identity and sample['end_ns']>=sample['begin_ns'];assert before['rc']==after['rc']==0 and before['birth']==after['birth']==identity['start_abstime'];assert before['parent_pid']==after['parent_pid']==identity['parent_pid'];assert before['bsd_returned_bytes']==after['bsd_returned_bytes']==ctypes.sizeof(BSD)
 for key in ['user_time_raw','system_time_raw']:assert before[key]<=u[key]<=after[key],(n,key,before[key],u[key],after[key])
 rows.append(dict(before=before,native=reply,after=after));durations.append(sample['end_ns']-sample['begin_ns'])
wrong=request(p,241,p.pid,{**identity,'start_abstime':identity['start_abstime']+1});assert wrong['sample'] is None and wrong['unknown']=='expected_identity_mismatch'
invalid=request(p,242,0,None);assert invalid['sample'] is None and invalid['unknown']=='invalid_pid'
child=subprocess.Popen(['/bin/sleep','10']);child_before=independent(child.pid);child_identity=dict(pid=child.pid,start_abstime=child_before['birth'],parent_pid=os.getpid());alive=request(p,243,child.pid,child_identity);assert alive['sample'] and alive['sample']['identity']==child_identity
child.terminate();child.wait();dead_oracle=independent(child.pid);dead=request(p,244,child.pid,child_identity);assert dead['sample'] is None and dead['unknown']=='resource_unavailable'
p.stdin.close();p.wait(timeout=10);assert p.returncode==0;stderr=p.stderr.read();assert not stderr;(D/'crosscheck.jsonl').write_text(''.join(json.dumps(x,separators=(',',':'))+'\n' for x in rows));(D/'negative-results.json').write_text(json.dumps(dict(wrong_birth=wrong,invalid_pid=invalid,live_child=alive,child_before=child_before,dead_child=dead,dead_oracle=dead_oracle),indent=2)+'\n')
# Finite raw-counter calibration on this host, not a cross-platform convention.
usage_before=independent(os.getpid());cpu_before=resource.getrusage(resource.RUSAGE_SELF);start_cpu=time.process_time();v=1
while time.process_time()-start_cpu<.25:v=(v*1664525+1013904223)&0xffffffff
cpu_after=resource.getrusage(resource.RUSAGE_SELF);usage_after=independent(os.getpid());raw_delta=sum(usage_after[k]-usage_before[k] for k in ['user_time_raw','system_time_raw']);cpu_seconds=(cpu_after.ru_utime+cpu_after.ru_stime)-(cpu_before.ru_utime+cpu_before.ru_stime);raw_as_ns=raw_delta/1e9;mach_as_ns=raw_as_ns*old.base.numer/old.base.denom
ratio_raw=raw_as_ns/cpu_seconds;ratio_mach=mach_as_ns/cpu_seconds;interpretation='raw_counters_are_mach_ticks_on_this_host' if abs(ratio_mach-1)<.05 and abs(ratio_raw-1)>.1 else ('raw_counters_are_nanoseconds_on_this_host' if abs(ratio_raw-1)<.05 and abs(ratio_mach-1)>.1 else 'unknown')
(D/'cpu-unit-calibration.json').write_text(json.dumps(dict(raw_before=usage_before,raw_after=usage_after,raw_delta=raw_delta,getrusage_seconds=cpu_seconds,raw_as_nanoseconds_seconds=raw_as_ns,mach_scaled_seconds=mach_as_ns,mach_timebase=dict(numer=old.base.numer,denom=old.base.denom),raw_ratio=ratio_raw,mach_ratio=ratio_mach,interpretation=interpretation,scope='local empirical calibration; no arbitrary host unit claim; adapter retains raw counters'),indent=2)+'\n');assert interpretation!='unknown'
# Protocol limits are actual failed processes; every output is retained.
failures=[]
for name,payload in [('long',b'x'*256),('partial',b'{'),('sequence',b'{"sequence":2,"pid":0,"expected":null}\n'),('unknown-field',b'{"sequence":1,"pid":0,"expected":null,"extra":true}\n'),('limit',b''.join((json.dumps(dict(sequence=n,pid=0,expected=None))+'\n').encode() for n in range(1,8194)))]:
 c=subprocess.run([binary,'native-metrics-probe',str(W/name)],input=payload,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30);assert c.returncode!=0,name;(D/(name+'.stdout')).write_bytes(c.stdout);(D/(name+'.stderr')).write_bytes(c.stderr);failures.append(dict(name=name,exit_code=c.returncode,stdout_bytes=len(c.stdout),stderr=c.stderr.decode()))
report=json.loads((W/'live/report.json').read_text());assert report['samples']==244 and report['unknowns']==3;assert report['arena']['activation']['grant_bytes']==2*1024**2 and report['arena']['allocation_failures']==0
result=dict(schema='mycity-native-reader-independent-probe-v1',source_sha256=info['source_sha256'],binary_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest(),live_samples=240,total_requests=244,abi_matches_local_sdk=True,mach_birth_parent_and_cpu_brackets_passed=True,volatile_footprint_compared_as_raw_not_forced_equal=True,sample_duration_ns=dict(minimum=min(durations),maximum=max(durations)),same_process_footprint_before_range_bytes=[min(x['before']['footprint_bytes'] for x in rows),max(x['before']['footprint_bytes'] for x in rows)],actual_protocol_failure_cases=failures,probe_report=report,cpu_unit_interpretation=interpretation,pressure_copy_bytes=0,replacement_credit_bytes=0,formal_observer='not_integrated',formal_acceptance='not_accepted',real_16gb='not_run',eight_hour='not_run')
(D/'probe-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
