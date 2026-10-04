# Consulta por hash sem chave: YARAify (abuse.ch), CIRCL hashlookup e Team Cymru MHR (DNS).
import json,sys,time,urllib.request,subprocess
def post(url,data):
    t=time.time(); r=urllib.request.urlopen(urllib.request.Request(url,data=json.dumps(data).encode(),headers={'User-Agent':'Warden-R6-spike'}),timeout=30).read(); return json.loads(r),time.time()-t
def get(url):
    t=time.time()
    try: r=urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'Warden-R6-spike'}),timeout=30).read(); return json.loads(r),time.time()-t
    except urllib.error.HTTPError as e: return {'http':e.code},time.time()-t
def cymru(h):
    o=subprocess.run(['nslookup','-type=TXT',h+'.hash.cymru.com'],capture_output=True,text=True).stdout
    q=[l.split('"')[1] for l in o.splitlines() if '"' in l]; return q[0] if q else '-'
for line in open(sys.argv[1]):
    if not line.strip() or line.startswith('#'): continue
    h,desc=line.split(None,1)
    y,ty=post('https://yaraify-api.abuse.ch/api/v1/',{'query':'lookup_hash','search_term':h})
    ys=y.get('query_status')
    if ys=='ok':
        tasks=y['data'].get('tasks',[])
        cl=sorted({c for t in tasks for c in (t.get('clamav_results') or [])})
        ru=sorted({r['rule_name'] for t in tasks for r in (t.get('static_results') or [])})
        ys=f"ok vistos={y['data']['metadata'].get('sightings')} clamav={cl[:4]} yara={ru[:6]}"
    c,tc=get(f'https://hashlookup.circl.lu/lookup/sha256/{h}') if len(h)==64 else get(f'https://hashlookup.circl.lu/lookup/sha1/{h}')
    cs='404' if c.get('http')==404 or 'message' in c else ('KnownMalicious='+str(c.get('KnownMalicious')) if 'KnownMalicious' in c else 'conhecido (NSRL)')
    print(f"{h[:16]}… {desc.strip()[:45]:45s} | Cymru {cymru(h):16s} | CIRCL {cs} ({tc:.2f}s) | YARAify {ys} ({ty:.2f}s)")
