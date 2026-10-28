#!/usr/bin/env python3
"""Compare two php-rs-phpt logs; fail if any previously passing fixture regresses."""
import argparse
from collections import Counter
from pathlib import Path


def read_log(path):
    cases = {}
    for line in Path(path).read_text().splitlines():
        if '.phpt ' not in line:
            continue
        name, result = line.split('.phpt ', 1)
        name += '.phpt'
        markers = [name.index(marker) for marker in ['Zend/tests/', 'tests/', 'ext/'] if marker in name]
        if markers:
            name = name[min(markers):]
        status, _, detail = result.partition(': ')
        if status in ['PASS', 'FAIL', 'UNSUPPORTED']:
            if name in cases:
                raise ValueError('Duplicate fixture: ' + name)
            cases[name] = (status, detail)
    if not cases:
        raise ValueError('No fixture results in ' + path)
    return cases


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('before')
parser.add_argument('after')
args = parser.parse_args()
before = read_log(args.before)
after = read_log(args.after)
for label, cases in [('Before', before), ('After', after)]:
    counts = Counter(status for status, _ in cases.values())
    print(f'{label}: {len(cases)} cases; {counts["PASS"]} passed, {counts["FAIL"]} failed, {counts["UNSUPPORTED"]} unsupported')
improved = sorted(name for name in before.keys() & after.keys() if before[name][0] != 'PASS' and after[name][0] == 'PASS')
regressed = sorted(name for name in before if before[name][0] == 'PASS' and after.get(name, ('MISSING', ''))[0] != 'PASS')
print(f'New passes: {len(improved)}; regressions: {len(regressed)}')
for label, names in [('NEW PASS', improved), ('REGRESSION', regressed), ('ADDED', sorted(after.keys()-before.keys())), ('MISSING', sorted(before.keys()-after.keys()))]:
    for name in names:
        print(label + ': ' + name)
print('\nMost frequent remaining failure messages:')
failures = Counter(detail.rsplit(' on line ', 1)[0] for status, detail in after.values() if status == 'FAIL')
for detail, count in failures.most_common(15):
    print(f'{count}: {detail[:180]}')
print('\nUnsupported section combinations:')
unsupported = Counter(detail for status, detail in after.values() if status == 'UNSUPPORTED')
for detail, count in unsupported.most_common(15):
    print(f'{count}: {detail}')
raise SystemExit(bool(regressed))
