import urllib.request,pathlib,zipfile,json,hashlib
r=pathlib.Path(__file__).resolve().parent;u='https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-win-x64-1.22.0.zip'
p=r/'models/onnxruntime-win-x64-1.22.0.zip';urllib.request.urlretrieve(u,p)
if hashlib.sha256(p.read_bytes()).hexdigest() != '174c616efc0271194488642a72f1a514e01487da4dfe84c49296d66e40ebe0da':
 raise SystemExit('Official runtime archive checksum mismatch')
out=r/'models/runtime';out.mkdir(exist_ok=True)
artifacts=[]
with zipfile.ZipFile(p) as z:
 for n in z.namelist():
  if n.endswith('.dll') or n.endswith('LICENSE') or n.endswith('ThirdPartyNotices.txt'):
   data=z.read(n);dst=out/pathlib.Path(n).name;dst.write_bytes(data)
   artifacts.append(dict(file=str(dst.relative_to(r)),bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
data=p.read_bytes();d=dict(url=u,bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),license='MIT',license_url='https://github.com/microsoft/onnxruntime/blob/v1.22.0/LICENSE',acquisition='Official Microsoft CPU release ZIP; ORT_DYLIB_PATH models/runtime/onnxruntime.dll; ort download-binaries archive also recorded separately',artifacts=artifacts)
(r/'runtime-provenance-official.json').write_text(json.dumps(d,indent=2)+'\n'); print(d)
