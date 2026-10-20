#!/usr/bin/env python3
"""Exact nearest-rank quantiles using bounded 8192-record sorting runs.
Raw input remains untouched. Scratch is a NEW directory, retired after each class.
"""
import array,hashlib,heapq,json,math,struct,sys
from pathlib import Path
CLASSES=['L0_pe0_model_transaction','L0_pe1_model_transaction','L2_claims_batch','L2_payroll_batch','L2_region_exchange_boot_batch','L2_background_capture_admit_cpu','air_source_page','air_cell_page','L1_native_record_index_page','L1_stub_record_index_page']
CHUNK=8192

def merge(paths,out):
    readers=[open(p,'rb',buffering=4096) for p in paths]
    heap=[]
    try:
        for i,f in enumerate(readers):
            b=f.read(8)
            if b:heap.append((int.from_bytes(b,'little'),i))
        heapq.heapify(heap)
        with open(out,'wb',buffering=65536) as target:
            while heap:
                value,i=heapq.heappop(heap);target.write(value.to_bytes(8,'little'));b=readers[i].read(8)
                if b:heapq.heappush(heap,(int.from_bytes(b,'little'),i))
    finally:
        for f in readers:f.close()

def main():
    if len(sys.argv)!=3:raise SystemExit('latency_audit.py RAW_DURATIONS NEW_OUTPUT_DIR')
    raw=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir();buffers=[array.array('Q') for _ in CLASSES];runs=[[] for _ in CLASSES];count=[0]*10;maximum=[0]*10;digest=hashlib.sha256();seen_bytes=0
    def flush(kind):
        if not buffers[kind]:return
        path=out/f'class-{kind}-run-{len(runs[kind])}.bin'
        with open(path,'wb') as f:
            for n in sorted(buffers[kind]):f.write(n.to_bytes(8,'little'))
        buffers[kind]=array.array('Q');runs[kind].append(path)
    with open(raw,'rb',buffering=65536) as f:
        while True:
            b=f.read(9)
            if not b:break
            if len(b)!=9 or b[0]>=10:raise RuntimeError('invalid raw duration record')
            seen_bytes+=9
            if seen_bytes>128*1024**2:raise RuntimeError('raw latency cap exceeded')
            digest.update(b);kind=b[0];value=int.from_bytes(b[1:],'little');buffers[kind].append(value);count[kind]+=1;maximum[kind]=max(maximum[kind],value)
            if len(buffers[kind])==CHUNK:flush(kind)
    report={}
    for kind,name in enumerate(CLASSES):
        flush(kind);paths=runs[kind];level=0
        while len(paths)>1:
            nxt=[]
            for i in range(0,len(paths),32):
                path=out/f'class-{kind}-merge-{level}-{i//32}.bin';merge(paths[i:i+32],path);nxt.append(path)
                for p in paths[i:i+32]:p.unlink()
            paths=nxt;level+=1
        p95=p99=None
        if count[kind]:
            with open(paths[0],'rb') as f:
                f.seek((((count[kind]*95+99)//100)-1)*8);p95=int.from_bytes(f.read(8),'little')
                f.seek((((count[kind]*99+99)//100)-1)*8);p99=int.from_bytes(f.read(8),'little')
            paths[0].unlink()
        report[name]={'samples':count[kind],'p95_ns':p95,'p99_ns':p99,'maximum_ns':maximum[kind],'gate':'not_passed_insufficient_samples' if count[kind]<200 else ('passed' if p95<=20000000 and p99<=40000000 else 'failed')}
    result={'schema':'mycity-exact-latency-audit-v1','audit_source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'raw_sha256':digest.hexdigest(),'raw_bytes':seen_bytes,'quantiles':'exact nearest rank ceil(n*p); original ns values','sort_chunk_records':CHUNK,'merge_fan_in':32,'scratch_is_not_a_gameplay_world':True,'classes':report}
    (out/'report.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
