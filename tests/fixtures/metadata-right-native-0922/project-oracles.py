"""Check retained native hashes and reproduce the observation-only TSV projection."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parent
for item in json.loads((root / 'sources.json').read_text()):
    assert hashlib.sha256((root / item['target']).read_bytes()).hexdigest() == item['sha256'], item
index = json.loads((root / 'standard-right-results.json').read_text())
contexts = {item['oracle']: item for item in index['contexts']}
rows = []
for item in index['observations']:
    context = contexts[item['context']]
    key = item['context']
    assert hashlib.sha256((root / key).read_bytes()).hexdigest() == context['sha256']
    roles = [str(Path(key).parent / 'RootDeny.rights.deflate')]
    if key.startswith('standard-paths'):
        roles = ['RootAllow.rights.deflate'] if context['user'] == 'RootProbeAdmin' else ['standard-profiles/owner-no-code-no/RootDeny.rights.deflate']
    if context['privileged']:
        roles = []
    rows.append([key, item['method'], item['right'], item['standard'], str(context['privileged']).lower(), item['outcome'], ','.join(roles)])
index = json.loads((root / 'multiple-roles/results.json').read_text())
for item in index['rows']:
    method, right, standard = item['probe'].split('.', 2)
    profile = item['profile']
    key = f'multiple-roles/{item["oracle"]}'
    assert hashlib.sha256((root / key).read_bytes()).hexdigest() == item['oracle_sha256']
    roles = [f'multiple-roles/{profile}/RootDeny.rights.deflate', f'multiple-roles/{profile}/RootAllow.rights.deflate']
    rows.append([key, method, right, standard, 'false', item['category'], ','.join(roles)])
assert len(rows) == 492
assert (root / 'cases.tsv').read_text() == '\n'.join('\t'.join(row) for row in rows) + '\n'
print('492 native oracle projections and source hashes verified')
