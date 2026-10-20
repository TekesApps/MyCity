"""Independent stdlib f64 stencil and time-integral check of frozen task artifacts."""
from pathlib import Path
import hashlib
import json
import struct
import time
import zipfile

E=Path(__file__).resolve().parent
started=time.monotonic()
report=json.loads((E/'air.json').read_text())
results=[]
with zipfile.ZipFile(E/'tasks.zip') as z:
    for case in report['cases']:
        name=case['name']
        i=json.loads(z.read(name+'/input.json'))
        raw=z.read(name+'/complete.air')
        payload=json.loads(raw[len(b'MYCITY-AIR-TASK-1\n')+128:])['payload']
        n=i['side'];pitch=n+2;cells=[y*pitch+x for y in range(1,n+1) for x in range(1,n+1)]
        field=[i['initial_micro']/1e6]*(pitch*pitch)
        ghost=[v/1e6 for v in i['boundary_micro']]
        for x in range(1,n+1):field[x]=ghost[0];field[(n+1)*pitch+x]=ghost[1]
        for y in range(1,n+1):field[y*pitch+n+1]=ghost[2];field[y*pitch]=ghost[3]
        events=sorted({0,i['target'],*range(60,i['target'],60),*[s['start'] for s in i['sources']],*[s['end'] for s in i['sources']]})
        integrals=[0.0]*len(cells);at=0
        for end in events[1:]:
            seconds=end-at;r=0.02*seconds/60;d=0.01*seconds/60
            keep=(1-d)*(1-4*r);neighbor=(1-d)*r;regen=d*2
            nxt=field[:]
            # Algebraically equivalent f64 reference, independently ordered, no paging.
            for p in cells:nxt[p]=keep*field[p]+neighbor*(field[p-pitch]+field[p+pitch]+field[p+1]+field[p-1])+regen
            for s in i['sources']:
                overlap=max(0,min(end,s['end'])-max(at,s['start']))
                c=s['cell'];nxt[(c//n+1)*pitch+c%n+1]+=s['rate_micro']*overlap/1e6
            for c,p in enumerate(cells):integrals[c]+=(field[p]+nxt[p])*seconds/2
            field=nxt;at=end
        actual=[struct.unpack('<f',struct.pack('<I',v))[0] for v in payload['field']]
        actual_i=[struct.unpack('<d',struct.pack('<Q',v))[0] for v in payload['integral']]
        abs_error=[abs(v-field[p]) for v,p in zip(actual,cells)]
        mean_error=[abs(v-ref)/i['target'] for v,ref in zip(actual_i,integrals)]
        ratio=max(e/(0.001+0.0001*abs(field[p])) for e,p in zip(abs_error,cells))
        mean_ratio=max(e/(0.001+0.0001*abs(ref/i['target'])) for e,ref in zip(mean_error,integrals))
        assert ratio<=1 and mean_ratio<=1,(name,ratio,mean_ratio)
        results.append({'name':name,'side':n,'game_seconds':i['target'],'steps':len(events)-1,'max_absolute_field_error':max(abs_error),'max_absolute_time_mean_error':max(mean_error),'max_field_normalized_error':ratio,'max_time_mean_normalized_error':mean_ratio,'passed':True})
result={'scope':'independent_python_f64_numerical_reference_only','criterion':'abs error of field or time mean <= 0.001 + 0.0001 * abs(reference)','tasks_zip_sha256':hashlib.sha256((E/'tasks.zip').read_bytes()).hexdigest(),'reference_script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'cases':results,'wall_seconds':time.monotonic()-started,'gpu_accuracy':'not_established','real_physical_accuracy':'not_verified'}
(E/'reference-checks.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
