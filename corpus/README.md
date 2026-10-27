# Independent construction corpus

Dev seed: `20261027`; sealed test seed: `20261028`. Fixtures and labels are authored from the T11 specification,
the public README, CLI help, and `docs/report.schema.json`. The author did not
read detectors, `clipboard.rs`, or S1/S4 generators during initial T11 authoring.
For T11-fix, the field-name declaration alone was checked to resolve the absent
README key list: `clipboard`, `pasteboard`, `clip_text`, `primaryClip`, `copiedText`.
These names are now documented in the README evaluation section. No matching
logic was used to choose core boundaries. T11-fix follows the coordinator?s
explicit contract corrections and missing-form requirements. No fixture was
selected or edited to improve observed scores. An initial screenshot-layout review,
before scores were inspected, replaced shared page framing with four distinct
UI structures: split IDE editor/terminal, emulator beside logcat, chat sidebar
and message bubble, and browser console with a drawer.

`labels.json` enumerates evaluation fixtures under `text/`, `images/`, and
`edge/`. Generator source, documentation, patterns, baseline and reports are
evaluation metadata, not scan inputs. The evaluator stages only the enumerated
fixtures in an isolated temporary `corpus/` and runs the release CLI there.
This prevents metadata self-matches and repository configuration/allowlists
from affecting scores, and permits the fixed `LEAKLENS_PATTERNS` file under
the public outside-active-repository restriction. No user configuration or
real evidence is loaded.

## Label contract

Cores come from the public [README](../README.md), never detector output:

- ?username segment only in Unix, Windows and JSON-escaped home paths? (line 30): home-path core is only the username.
- ?private-key blocks, including truncated pastes? (line 31): PEM core is the complete planted block, including literal JSON-escaped `\n` sequences and any truncated ending.
- ?ClipboardService quoted ClipData/text and timestamped multiline history? (line 32): core is the quoted text content or all history body lines, excluding the timestamp/header.
- ?quoted fields and line-leading (or recognized log-prefix) unquoted prose fields? (line 33): field core is its value.
- ?13?19 digits, known IIN prefixes, Luhn check?, ?Email?, ?international/NANP formats?, and ?external literals ? and/or Rust regex? (lines 27?34): card/email/phone/user-pattern cores are the inserted values.

Each label has `core` and `extent`. Text regions are half-open UTF-8 byte
ranges recorded during insertion and mapped through JSON escaping and source
wrappers. `extent` includes the complete enclosing diagnostic record; `core`
contains only the contract bytes above. Screenshot regions contain per-character
boxes for the planted value (core and extent coincide). No fixture or core is
inferred from predictions. Source fixtures contain inert diagnostic strings,
not applications or compiler test inputs.

Text TPs require exact category/severity and coverage of at least 90% of core
bytes. The prediction may be larger. Spill counts predicted text bytes outside
the union of same-category extents, across all predictions, and is diagnostic
only. Screenshot matching retains the separate 98% union glyph-area threshold.
One-to-one matching prevents duplicate credit.

Existing `phone/national-no-country-code` and `clipboard/undocumented-key`
labels are retained as `off_spec` groups. Their TP/FP/FN counts are reported
separately; their labels and overlapping predictions never enter quality gates
or baseline regression metrics. Added E.164 values retain the country code and
national grouping for DE, GB, FR, NL, ES, IT, JP and AU. Added documented
fields are paired with history examples to preserve the specification?s minimum
40% physically multiline clipboard mix after expanding field coverage.

Screenshot truth consists of per-character `Range.getClientRects()` rectangles
in device pixels (browser advance boxes, including spaces, not ink masks).
The four UI structures are crossed with scales 1, 1.25, 1.5, 2 and font sizes
11, 13, 16 CSS px. Product findings already contain product padding; the scorer
does not add extra padding. PNGs and truth are static evaluation inputs, so
CI needs neither fonts nor a browser. Fixtures have SHA-256 hashes in the
manifest; `.gitattributes` disables checkout text conversion.

All personal-looking data is synthetic and must not be contacted or used for
transactions. Emails use reserved domains. US telephone examples use the
555-01xx range; national-format and explicitly requested NANP-local edge forms
are deliberately measured even when unsupported. Other international strings
are invented test fixtures, not asserted globally allocated test numbers.
PEM bodies are seeded random bytes encoded as base64, never actual generated
cryptographic keys. Names and prose are invented. Card fixtures are published
sandbox numbers from these primary sources (checked 2026-10-27):

- [Stripe testing](https://docs.stripe.com/testing?numbers-or-method-or-token=tokens): Visa, Mastercard 51/2-series.
- [Braintree testing](https://developer.paypal.com/braintree/docs/reference/general/testing/node): Visa, Amex, Discover, Mastercard 55.
- [Adyen test cards](https://docs.adyen.com/development-resources/testing/test-card-numbers/): Mastercard 54/2-series, Amex, Discover.

Negative lines include UUIDs/hashes, timestamps, IPs/versions, order/tracking
IDs, invalid Luhn values, public PEM/certificates, non-01xx short 555 forms,
clipboard UI/code/scalars, placeholder paths, and non-address at-sign shapes.
There are at least as many negative lines as positive lines in every text
fixture. This is constructed adversarial coverage, not a population sample.

## Reproduce

Evaluate the frozen corpus (no regeneration needed):

```sh
cargo build --release --locked
cargo xtask eval --split dev
cargo xtask eval --split dev --write-baseline  # deliberate reviewed baseline update only
cargo xtask eval --split dev --ci
node corpus/verify.mjs
node corpus/verify.mjs --split test
cargo xtask eval --split test  # milestone check only; never tuning
```

Set `LEAKLENS_MODELS` to the verified model directory. Reports are generated
at `RESULTS.md` and `../target/eval.json`. Absolute gate failures are evidence;
`--ci` compares quality metrics to `baseline.json` with a one-percentage-point
tolerance. Runtime/config/corpus identity failures are errors. Host performance
is reported separately, not normalized across Windows/Linux or CPUs.

To recreate inputs deliberately, use Node and `playwright-core` 1.56.1 with
installed Edge (`node corpus/generate.mjs`). The default reuses the existing
repository installation at `spikes/s1-ocr/node_modules/playwright-core` without
reading spike generators. Alternatively set `PLAYWRIGHT_CORE` to an installed
package path and `CORPUS_BROWSER=chrome` for installed Chrome. No browser is
downloaded. Rendering may differ with browser/OS/font versions; regeneration
changes corpus identity and requires fresh review, not automatic acceptance.
Symlinks are constructed when permitted; a committed symlink is recreated
without following its target during evaluation.

Do not tune this corpus to pass gates. Detector changes belong to separate
tasks; preserve failed category, context, localization, coverage and redaction
results in the generated report.

## Sealed test split

`corpus/test/` is generated from the same specification with seed `20261028`,
including its own manifest, patterns and screenshots. Its labels manifest SHA-256
is pinned in the dev baseline; every input hash is also recorded in that manifest
and checked before scoring. Generate with `node corpus/generate.mjs --split test`
only when deliberately establishing a new sealed corpus. Test scores go to
`test/RESULTS.md` and `../target/eval-test.json`, preserving dev results.
CI and baseline writing reject the test split. Test evaluation prints a reminder
that milestone checks must not become tuning feedback. Do not regenerate this
split after observing results.
