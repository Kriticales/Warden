import re, collections
txt = open(r'C:\wt\r6\rep\final.txt', 'rb').read().decode('utf-8', 'replace')
ev = collections.OrderedDict()
PREF = 'C:' + chr(92) + 'wt' + chr(92) + 'r6' + chr(92)
for l in txt.splitlines():
    m = re.match(r'Hoje, 04/10/2026 (1[89]:\d\d:\d\d)\t(.*)', l)
    if not m or PREF not in l or 'grupo' in l:
        continue
    c = m.group(2).split('\t')
    if c[1] in ('Detectado', 'Excluído(s)', 'Não processado', 'Cópia de backup criada'):
        obj, evento, ver, comp = c[0], c[2], c[3], 'verificação (avp.com)'
    else:
        obj, evento, ver, comp = c[0], c[5], c[7], 'proteção em tempo real'
    ev[(m.group(1), obj.replace(PREF, ''), evento, ver, comp)] = 1
cnt = collections.Counter((k[4], k[2]) for k in ev)
for (comp, e), n in sorted(cnt.items()):
    print(f'{n:3d}  {comp:28s} {e}')
print('total', len(ev))
for k in ev:
    if k[0] >= '18:48':
        print(k)
