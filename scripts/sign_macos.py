from pathlib import Path
import plistlib, subprocess, tomllib
root=Path(__file__).resolve().parents[1]
version=tomllib.loads((root/'plugins/spectral_compressor/Cargo.toml').read_text())['package']['version']
for ext in ('vst3','clap','app'):
    bundle=root/'target/bundled'/f'Spectral Compressor Turbo.{ext}'
    plist=bundle/'Contents/Info.plist'
    metadata=plistlib.loads(plist.read_bytes())
    metadata.update(CFBundleIdentifier=f'com.ferropop.spectral-compressor-turbo.{ext}',
                    CFBundleShortVersionString=version,CFBundleVersion=version,
                    NSHumanReadableCopyright='Original: Robbert van der Helm. Turbo: ferropop. GPL-3.0-or-later.')
    plist.write_bytes(plistlib.dumps(metadata))
    subprocess.run(['codesign','--force','--sign','-',str(bundle)],check=True)
    subprocess.run(['codesign','--verify','--deep','--strict',str(bundle)],check=True)
