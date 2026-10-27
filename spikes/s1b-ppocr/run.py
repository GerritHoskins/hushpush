import pathlib,json,hashlib,subprocess,platform,os,argparse
root=pathlib.Path(__file__).resolve().parent
parser=argparse.ArgumentParser();parser.add_argument('variants',nargs='*',default=['mobile','server']);args=parser.parse_args()
if any(v not in ['mobile','server'] for v in args.variants):parser.error('mobile/server only')
corpus=root.parent/'s1-ocr/out';manifest=json.loads((corpus/'manifest.json').read_text())
subprocess.run(['node','--test',str(root/'test.mjs')],check=True)
print('Verified PNG and truth hashes for every original manifest image',flush=True)
exe=root/'runner/target/release/s1b-ppocr-runner.exe'
os.environ['ORT_DYLIB_PATH']=str(root/'models/runtime/onnxruntime.dll')
for variant in args.variants:
 subprocess.run([str(exe),str(root/'models'),str(corpus),str(root/'out'/variant),variant,'4','960'],check=True)
subprocess.run(['node',str(root/'score.mjs'),*args.variants],check=True)
