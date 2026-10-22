from pathlib import Path
import hashlib, json, shutil, subprocess

root = Path.cwd()
proof = Path(__file__).resolve().parent
source = root / 'docs/research/fixtures/user-settings-native-1015/legacy-user-records/generate-own-records.rs'
original = source.read_text()
# Повторно используем измеренную реконструкцию, включая существующий render.
# Меняем только собственный входной каталог, имена и два нужных размера.
text = original.replace('let root=Path::new("/tmp/open-bsl-user-settings-native-1015/private");', 'let argument=std::env::args().nth(1).unwrap(); let root=Path::new(&argument);')
text = text.replace('[("OpenBslUserEmpty", 0usize), ("OpenBslUserSeparated", 1usize)]', '[("LangRussian", 0usize), ("LangEnglish", 0usize), ("LangDefault", 0usize)]')
text = text.replace('[37usize,31,32]', '[37usize,31]')
text = text.replace('let length=usize::from(bytes[0]);', '''let baseline=root.parent().unwrap().join("baseline").join(format!("{user}.data"));
        if baseline.exists() {
            let actual=open_sdbl::metadata::decode_user_data(&fs::read(baseline)?)?;
            let supplied=open_sdbl::metadata::decode_user_data(&bytes)?;
            assert_eq!(actual.id,supplied.id);
            assert_eq!(actual.name,supplied.name);
            assert_eq!(actual.roles,supplied.roles);
        }
        let length=usize::from(bytes[0]);''')
assert text != original and '/tmp/open-bsl-user-settings-native-1015/private' not in text
generated = root / 'target/user-language-legacy-generator.rs'
generated.write_text(text)
inputs = proof / 'inputs'
inputs.mkdir(exist_ok=True)
native = root / 'docs/research/fixtures/approved-sdk-production-integration-1016/current-language-session-audit/native-user-records-1017'
for name in ['LangRussian', 'LangEnglish', 'LangDefault']:
    shutil.copy2(native / f'{name}.data', inputs / f'{name}.data')
library = json.loads((root / 'docs/research/fixtures/sdk24-user-language-review-1018/probe-build.json').read_text())['libraries']['open_sdbl']
assert hashlib.sha256(Path(library['path']).read_bytes()).hexdigest() == library['sha256']
binary = root / 'target/user-language-legacy-generator'
args = ['rustc', '--edition=2024', '--crate-name', 'user_language_legacy_generator', str(generated), '-L', 'dependency=' + str(root / 'target/verification-compact-1017/debug/deps'), '--extern', 'open_sdbl=' + library['path'], '-o', str(binary)]
with (proof / 'generator-build.log').open('w') as log:
    result = subprocess.run(args, stdout=log, stderr=subprocess.STDOUT)
assert result.returncode == 0
with (proof / 'generator.log').open('w') as log:
    result = subprocess.run([str(binary), str(inputs)], stdout=log, stderr=subprocess.STDOUT)
assert result.returncode == 0
(proof / 'generator.json').write_text(json.dumps(dict(status='prepared inputs, not a native oracle',source=str(source.relative_to(root)),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),generated_source_sha256=hashlib.sha256(generated.read_bytes()).hexdigest(),forms=[31,37],users=['LangRussian','LangEnglish','LangDefault'],production_records_used=False,scope='Existing reconstruction algorithm applied to own native empty-password users. SDK validates structure only; platform results still required.'),indent=2)+'\n')
