"""Package built binaries, licenses, build metadata, and validation evidence."""
from pathlib import Path
import hashlib, json, shutil, subprocess, sys, zipfile, tomllib
root=Path(__file__).resolve().parents[1]
platform=sys.argv[1]
version=tomllib.loads((root/'plugins/spectral_compressor/Cargo.toml').read_text())['package']['version']
name=f'Spectral-Compressor-Turbo-v{version}-{platform}'
out=root/'dist'/name
out.mkdir(parents=True,exist_ok=True)
for extension in ('vst3','clap','app','exe'):
    path=root/'target/bundled'/f'Spectral Compressor Turbo.{extension}'
    if path.exists():
        if path.is_dir(): shutil.copytree(path,out/path.name,dirs_exist_ok=True)
        else: shutil.copy2(path,out/path.name)
for filename in ('README.md','COPYING','LICENSE','THIRD-PARTY.md'):
    if (root/filename).exists(): shutil.copy2(root/filename,out/filename)
licenses=out/'Licenses'
licenses.mkdir(exist_ok=True)
for src,license_name in [('plugins/spectral_compressor/src/editor/fonts/LICENSE','Noto-Sans-OFL.txt'),('vendor/vizia/LICENSE','Vizia-MIT.txt')]:
    shutil.copy2(root/src,licenses/license_name)
if (root/'VALIDATION.md').exists(): shutil.copy2(root/'VALIDATION.md',out/'VALIDATION.md')
validation=root/'validation'
if validation.exists(): shutil.copytree(validation,out/'Validation',dirs_exist_ok=True)
info={'name':'Spectral Compressor Turbo','version':version,'platform':platform,'authors':['Robbert van der Helm','ferropop'],'source':'https://github.com/ferropop/spectral-compressor-turbo','commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()}
(out/'BUILD-INFO.json').write_text(json.dumps(info,indent=2)+'\n')
hashes={p.relative_to(out).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.rglob('*')) if p.is_file()}
(out/'SHA256SUMS.txt').write_text(''.join(f'{sha}  {name}\n' for name,sha in hashes.items()))
archive=root/'dist'/f'{name}.zip'
with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED) as z:
    for p in sorted(out.rglob('*')):
        if p.is_file(): z.write(p,p.relative_to(out.parent))
print(archive)
