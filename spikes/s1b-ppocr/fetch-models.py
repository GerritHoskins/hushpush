import json,urllib.request,pathlib,hashlib,concurrent.futures
import yaml
root=pathlib.Path(__file__).resolve().parent; records=[]
REVISIONS = {
 'mobile_det': 'e6f4fa85f00e168c862bc462aebca69eef9b3d3d',
 'mobile_rec': 'ed152b8b495f84de93cda5709d768548a9127622',
 'server_det': 'dcf248c4dcc064c6d030db241255f9e8fbc6733a',
 'server_rec': 'b70df217f4fd99d14f970bad092cebe7d74cc4d1',
}
(root/'models').mkdir(exist_ok=True)
previous=json.loads((root/'provenance.json').read_text()) if (root/'provenance.json').exists() else []
def get(kind):
 repo='PaddlePaddle/PP-OCRv5_'+kind+'_onnx'
 rev=REVISIONS[kind]
 result=[]
 for source in ['README.md','inference.yml','inference.onnx']:
  url=f'https://huggingface.co/{repo}/resolve/{rev}/{source}'
  path=root/'models'/(kind+'_'+source)
  urllib.request.urlretrieve(url,path)
  data=path.read_bytes()
  expected=next((r for r in previous if r['url']==url),None)
  if expected and hashlib.sha256(data).hexdigest()!=expected['sha256']:
   raise RuntimeError('Pinned model artifact checksum mismatch: '+kind+' '+source)
  result.append(dict(file=str(path.relative_to(root)),url=url,bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),license='Apache-2.0 declared by official model card',license_url=f'https://huggingface.co/{repo}/blob/{rev}/README.md'))
 return result
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
 for r in pool.map(get,['mobile_det','mobile_rec','server_det','server_rec']):records.extend(r)
for variant in ['mobile','server']:
 source=root/'models'/f'{variant}_rec_inference.yml'
 chars=yaml.safe_load(source.read_text(encoding='utf-8'))['PostProcess']['character_dict']
 target=root/'models'/f'{variant}_dict.txt'
 target.write_text('\n'.join(chars)+'\n',encoding='utf-8')
 data=target.read_bytes()
 parent=next(r for r in records if r['file'].endswith(f'{variant}_rec_inference.yml'))
 records.append(dict(file=str(target.relative_to(root)),url=parent['url']+'#PostProcess.character_dict',bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),license=parent['license'],license_url=parent['license_url'],derivation='UTF-8 one dictionary entry per line, in original YAML order; CTC blank and optional space handled by runner'))
(root/'provenance.json').write_text(json.dumps(records,indent=2)+'\n')
print('Downloaded and recorded',len(records),'artifacts;',sum(r['bytes'] for r in records),'bytes')
