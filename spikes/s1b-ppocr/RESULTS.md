# S1b: PP-OCRv5 via Rust ort

Measured 2026-10-27. Bounded synthetic-corpus evidence only. No real personal files were scanned.

**mobile: FAIL** (recall FAIL, derived-character localization PASS, desktop speed PASS).
**server: FAIL** (recall FAIL, derived-character localization FAIL, desktop speed FAIL).

The four-core laptop gate and network-isolated execution remain unverified. This spike does not by itself clear release or redistribution review.

## Method

The unchanged S1 corpus and its original labels are scored with the same category regex/Luhn detection, canonical matching and one-to-one occurrence assignment. PNG and truth hashes are verified before each run. No fixture regeneration, spelling repair or ground-truth-assisted OCR occurs. Recall counts exact normalized planted values; unmatched findings and those overlapping benign look-alikes are counted separately. Glyph coverage means union of browser character rectangles, not an ink mask; misses contribute zero to end-to-end coverage. Padding is in original device pixels.

The JSON retains line text, quadrilaterals and a whole-line `words` entry for S1 shape compatibility. `chars` contains decoded CTC tokens and timestep indices; the inverse crop projection maps their horizontal ranges into original raster coordinates. This is approximate sub-line geometry. Both coverage modes use identical text, so localization cannot alter recall. S1b scoring lives entirely here and imports S1 primitives read-only; no S1 file was changed.

## Measured gates

| Variant | Criterion | S1b | S1 base |
| --- | --- | --- | --- |
| mobile | recall at 1x (each category) | FAIL | FAIL |
| mobile | recall at 2x (each category) | FAIL | FAIL |
| mobile | every found value ≥98% glyph-range coverage, char boxes +8 px | PASS | FAIL |
| mobile | every found value ≥98% glyph-range coverage, line boxes +8 px | PASS | FAIL |
| mobile | every 1x image warm ≤1.5 s on measured desktop | PASS | PASS |
| mobile | warm ≤1.5 s on a 4-core laptop | NOT RUN | NOT RUN |
| server | recall at 1x (each category) | FAIL | FAIL |
| server | recall at 2x (each category) | FAIL | FAIL |
| server | every found value ≥98% glyph-range coverage, char boxes +8 px | FAIL | FAIL |
| server | every found value ≥98% glyph-range coverage, line boxes +8 px | PASS | FAIL |
| server | every 1x image warm ≤1.5 s on measured desktop | FAIL | PASS |
| server | warm ≤1.5 s on a 4-core laptop | NOT RUN | NOT RUN |

S1 base char comparisons use its word boxes; line comparisons use its line boxes. Four-core laptop remains unmeasured.

## Recall and false findings

| Variant | Scale | Category | Planted | Found | Recall | S1 base | Delta | False on look-alikes | All false findings |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mobile | 1 | email | 60 | 41 | 68.33% | 23.33% | 45.00 pp | 0 | 8 |
| mobile | 1 | phone | 40 | 31 | 77.50% | 62.50% | 15.00 pp | 0 | 0 |
| mobile | 1 | card | 60 | 47 | 78.33% | 71.67% | 6.67 pp | 0 | 0 |
| mobile | 1.5 | email | 60 | 56 | 93.33% | 16.67% | 76.67 pp | 0 | 2 |
| mobile | 1.5 | phone | 40 | 36 | 90.00% | 70.00% | 20.00 pp | 0 | 0 |
| mobile | 1.5 | card | 60 | 52 | 86.67% | 73.33% | 13.33 pp | 0 | 0 |
| mobile | 2 | email | 60 | 58 | 96.67% | 13.33% | 83.33 pp | 0 | 2 |
| mobile | 2 | phone | 40 | 37 | 92.50% | 62.50% | 30.00 pp | 0 | 0 |
| mobile | 2 | card | 60 | 50 | 83.33% | 63.33% | 20.00 pp | 0 | 0 |
| server | 1 | email | 60 | 50 | 83.33% | 23.33% | 60.00 pp | 0 | 10 |
| server | 1 | phone | 40 | 35 | 87.50% | 62.50% | 25.00 pp | 0 | 0 |
| server | 1 | card | 60 | 51 | 85.00% | 71.67% | 13.33 pp | 0 | 0 |
| server | 1.5 | email | 60 | 56 | 93.33% | 16.67% | 76.67 pp | 0 | 3 |
| server | 1.5 | phone | 40 | 35 | 87.50% | 70.00% | 17.50 pp | 0 | 0 |
| server | 1.5 | card | 60 | 48 | 80.00% | 73.33% | 6.67 pp | 0 | 0 |
| server | 2 | email | 60 | 55 | 91.67% | 13.33% | 78.33 pp | 0 | 4 |
| server | 2 | phone | 40 | 31 | 77.50% | 62.50% | 15.00 pp | 0 | 0 |
| server | 2 | card | 60 | 48 | 80.00% | 63.33% | 16.67 pp | 0 | 0 |

## Localization

| Variant | Scale | Boxes | Found | Mean +0 px | Mean +4 px | Mean +8 px | Worst +8 px | ≥98% at +8 | End-to-end mean +8 (misses=0) | S1 mean +0 | S1 mean +4 | S1 mean +8 | S1 worst +8 | S1 ≥98% at +8 | S1 end-to-end +8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mobile | 1 | char | 119 | 99.42% | 100.00% | 100.00% | 100.00% | 119/119 | 74.38% | 77.16% | 99.51% | 100.00% | 100.00% | 82/82 | 51.25% |
| mobile | 1 | line | 119 | 99.31% | 100.00% | 100.00% | 100.00% | 119/119 | 74.38% | 92.30% | 100.00% | 100.00% | 100.00% | 82/82 | 51.25% |
| mobile | 1.5 | char | 144 | 98.89% | 99.99% | 100.00% | 100.00% | 144/144 | 90.00% | 67.08% | 94.39% | 99.98% | 99.48% | 82/82 | 51.24% |
| mobile | 1.5 | line | 144 | 98.74% | 100.00% | 100.00% | 100.00% | 144/144 | 90.00% | 81.19% | 98.29% | 100.00% | 100.00% | 82/82 | 51.25% |
| mobile | 2 | char | 145 | 99.02% | 99.97% | 100.00% | 100.00% | 145/145 | 90.63% | 59.43% | 85.09% | 98.30% | 90.13% | 47/71 | 43.62% |
| mobile | 2 | line | 145 | 98.96% | 99.96% | 100.00% | 100.00% | 145/145 | 90.63% | 71.76% | 92.82% | 99.85% | 97.37% | 67/71 | 44.31% |
| server | 1 | char | 136 | 98.39% | 99.94% | 100.00% | 100.00% | 136/136 | 85.00% | 77.16% | 99.51% | 100.00% | 100.00% | 82/82 | 51.25% |
| server | 1 | line | 136 | 97.60% | 100.00% | 100.00% | 100.00% | 136/136 | 85.00% | 92.30% | 100.00% | 100.00% | 100.00% | 82/82 | 51.25% |
| server | 1.5 | char | 139 | 96.61% | 99.78% | 99.99% | 99.10% | 139/139 | 86.87% | 67.08% | 94.39% | 99.98% | 99.48% | 82/82 | 51.24% |
| server | 1.5 | line | 139 | 96.43% | 99.97% | 100.00% | 100.00% | 139/139 | 86.88% | 81.19% | 98.29% | 100.00% | 100.00% | 82/82 | 51.25% |
| server | 2 | char | 134 | 95.47% | 99.43% | 99.93% | 97.45% | 133/134 | 83.69% | 59.43% | 85.09% | 98.30% | 90.13% | 47/71 | 43.62% |
| server | 2 | line | 134 | 95.68% | 99.59% | 100.00% | 100.00% | 134/134 | 83.75% | 71.76% | 92.82% | 99.85% | 97.37% | 67/71 | 44.31% |

| Variant | Scale | Boxes | Category | Found | Mean +0 px | Mean +4 px | Mean +8 px | Worst +8 px | S1 mean +0 | S1 mean +4 | S1 mean +8 | S1 worst +8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mobile | 1 | char | email | 41 | 98.79% | 100.00% | 100.00% | 100.00% | 90.98% | 99.86% | 100.00% | 100.00% |
| mobile | 1 | char | phone | 31 | 99.64% | 100.00% | 100.00% | 100.00% | 78.16% | 99.94% | 100.00% | 100.00% |
| mobile | 1 | char | card | 47 | 99.83% | 100.00% | 100.00% | 100.00% | 72.08% | 99.14% | 100.00% | 100.00% |
| mobile | 1 | line | email | 41 | 98.44% | 100.00% | 100.00% | 100.00% | 96.05% | 100.00% | 100.00% | 100.00% |
| mobile | 1 | line | phone | 31 | 99.46% | 100.00% | 100.00% | 100.00% | 90.78% | 100.00% | 100.00% | 100.00% |
| mobile | 1 | line | card | 47 | 99.98% | 100.00% | 100.00% | 100.00% | 91.96% | 100.00% | 100.00% | 100.00% |
| mobile | 1.5 | char | email | 56 | 98.21% | 99.98% | 100.00% | 100.00% | 80.41% | 98.64% | 100.00% | 100.00% |
| mobile | 1.5 | char | phone | 36 | 99.01% | 100.00% | 100.00% | 100.00% | 68.27% | 94.79% | 99.94% | 99.48% |
| mobile | 1.5 | char | card | 52 | 99.55% | 100.00% | 100.00% | 100.00% | 63.29% | 93.16% | 100.00% | 100.00% |
| mobile | 1.5 | line | email | 56 | 98.00% | 100.00% | 100.00% | 100.00% | 89.47% | 99.59% | 100.00% | 100.00% |
| mobile | 1.5 | line | phone | 36 | 98.89% | 100.00% | 100.00% | 100.00% | 80.12% | 98.29% | 100.00% | 100.00% |
| mobile | 1.5 | line | card | 52 | 99.43% | 100.00% | 100.00% | 100.00% | 79.98% | 97.99% | 100.00% | 100.00% |
| mobile | 2 | char | email | 58 | 97.96% | 99.96% | 100.00% | 100.00% | 75.44% | 95.05% | 99.70% | 98.87% |
| mobile | 2 | char | phone | 37 | 99.63% | 99.98% | 100.00% | 100.00% | 59.60% | 86.56% | 99.41% | 96.76% |
| mobile | 2 | char | card | 50 | 99.80% | 99.98% | 100.00% | 100.00% | 55.95% | 82.03% | 97.28% | 90.13% |
| mobile | 2 | line | email | 58 | 97.69% | 99.91% | 100.00% | 100.00% | 82.66% | 97.10% | 100.00% | 100.00% |
| mobile | 2 | line | phone | 37 | 99.68% | 100.00% | 100.00% | 100.00% | 67.46% | 91.99% | 99.89% | 97.37% |
| mobile | 2 | line | card | 50 | 99.91% | 100.00% | 100.00% | 100.00% | 72.30% | 92.46% | 99.79% | 97.37% |
| server | 1 | char | email | 50 | 98.26% | 99.95% | 100.00% | 100.00% | 90.98% | 99.86% | 100.00% | 100.00% |
| server | 1 | char | phone | 35 | 98.10% | 99.90% | 100.00% | 100.00% | 78.16% | 99.94% | 100.00% | 100.00% |
| server | 1 | char | card | 51 | 98.71% | 99.94% | 100.00% | 100.00% | 72.08% | 99.14% | 100.00% | 100.00% |
| server | 1 | line | email | 50 | 97.53% | 100.00% | 100.00% | 100.00% | 96.05% | 100.00% | 100.00% | 100.00% |
| server | 1 | line | phone | 35 | 97.59% | 100.00% | 100.00% | 100.00% | 90.78% | 100.00% | 100.00% | 100.00% |
| server | 1 | line | card | 51 | 97.68% | 100.00% | 100.00% | 100.00% | 91.96% | 100.00% | 100.00% | 100.00% |
| server | 1.5 | char | email | 56 | 96.31% | 99.73% | 100.00% | 99.83% | 80.41% | 98.64% | 100.00% | 100.00% |
| server | 1.5 | char | phone | 35 | 96.55% | 99.67% | 99.99% | 99.56% | 68.27% | 94.79% | 99.94% | 99.48% |
| server | 1.5 | char | card | 48 | 97.00% | 99.91% | 99.98% | 99.10% | 63.29% | 93.16% | 100.00% | 100.00% |
| server | 1.5 | line | email | 56 | 96.54% | 100.00% | 100.00% | 100.00% | 89.47% | 99.59% | 100.00% | 100.00% |
| server | 1.5 | line | phone | 35 | 96.69% | 99.90% | 100.00% | 100.00% | 80.12% | 98.29% | 100.00% | 100.00% |
| server | 1.5 | line | card | 48 | 96.11% | 100.00% | 100.00% | 100.00% | 79.98% | 97.99% | 100.00% | 100.00% |
| server | 2 | char | email | 55 | 94.79% | 99.41% | 99.96% | 98.86% | 75.44% | 95.05% | 99.70% | 98.87% |
| server | 2 | char | phone | 31 | 96.33% | 99.15% | 99.83% | 97.45% | 59.60% | 86.56% | 99.41% | 96.76% |
| server | 2 | char | card | 48 | 95.69% | 99.63% | 99.96% | 98.92% | 55.95% | 82.03% | 97.28% | 90.13% |
| server | 2 | line | email | 55 | 95.46% | 99.65% | 100.00% | 100.00% | 82.66% | 97.10% | 100.00% | 100.00% |
| server | 2 | line | phone | 31 | 96.67% | 99.46% | 100.00% | 100.00% | 67.46% | 91.99% | 99.89% | 97.37% |
| server | 2 | line | card | 48 | 95.29% | 99.59% | 100.00% | 100.00% | 72.30% | 92.46% | 99.79% | 97.37% |

## Warm timing

| Variant | Scale | Images | Warm mean s | Warm max s | S1 mean s | S1 max s |
| --- | --- | --- | --- | --- | --- | --- |
| mobile | 1 | 20 | 0.703 | 1.152 | 0.402 | 0.494 |
| mobile | 1.5 | 20 | 0.741 | 1.106 | 0.469 | 0.529 |
| mobile | 2 | 20 | 0.699 | 1.157 | 0.536 | 0.583 |
| server | 1 | 20 | 2.832 | 3.496 | 0.402 | 0.494 |
| server | 1.5 | 20 | 2.881 | 3.320 | 0.469 | 0.529 |
| server | 2 | 20 | 2.918 | 3.664 | 0.536 | 0.583 |

## Observed failure modes

| Variant | Observed symptom | Missed values | Example image IDs |
| --- | --- | --- | --- |
| mobile | Canonical characters altered in overlapping OCR lines | 50 | terminal-dark-s1-f11, terminal-dark-s1-f14, terminal-dark-s2-f11, terminal-light-s1-f11 |
| mobile | Email @ absent from overlapping OCR lines | 11 | terminal-dark-s1-f12, terminal-light-s1-f11, devtools-s1-f11, devtools-s1-f12 |
| mobile | Canonical value survives; detector separator or boundary mismatch | 8 | terminal-dark-s1-f16, terminal-dark-s2-f16, chat-s1.5-f12, chat-s2-f14 |
| mobile | No OCR line overlaps planted glyph ranges | 3 | terminal-dark-s1.5-f11, terminal-light-s1.5-f12, devtools-s1.5-f11 |
| server | Canonical characters altered in overlapping OCR lines | 62 | terminal-dark-s1-f14, terminal-dark-s1.5-f11, terminal-dark-s2-f11, terminal-light-s1-f11 |
| server | Canonical value survives; detector separator or boundary mismatch | 7 | terminal-dark-s1.5-f16, terminal-dark-s2-f16, chat-s1-f14, chat-s1-f16 |
| server | Email @ absent from overlapping OCR lines | 1 | devtools-s1.5-f11 |
| server | Email dot absent from overlapping OCR lines | 1 | log-viewer-s2-f12 |

Symptoms examine overlapping full lines and can include neighboring tokens; they do not establish a model root cause.

## Failure breakdown

| Variant | Template | CSS font px | Planted | Found | Recall |
| --- | --- | --- | --- | --- | --- |
| mobile | terminal-dark | 11 | 24 | 17 | 70.83% |
| mobile | terminal-dark | 12 | 24 | 23 | 95.83% |
| mobile | terminal-dark | 14 | 24 | 20 | 83.33% |
| mobile | terminal-dark | 16 | 24 | 22 | 91.67% |
| mobile | terminal-light | 11 | 24 | 18 | 75.00% |
| mobile | terminal-light | 12 | 24 | 23 | 95.83% |
| mobile | terminal-light | 14 | 24 | 23 | 95.83% |
| mobile | terminal-light | 16 | 24 | 24 | 100.00% |
| mobile | chat | 11 | 24 | 19 | 79.17% |
| mobile | chat | 12 | 24 | 18 | 75.00% |
| mobile | chat | 14 | 24 | 19 | 79.17% |
| mobile | chat | 16 | 24 | 23 | 95.83% |
| mobile | devtools | 11 | 24 | 17 | 70.83% |
| mobile | devtools | 12 | 24 | 21 | 87.50% |
| mobile | devtools | 14 | 24 | 23 | 95.83% |
| mobile | devtools | 16 | 24 | 22 | 91.67% |
| mobile | log-viewer | 11 | 24 | 13 | 54.17% |
| mobile | log-viewer | 12 | 24 | 20 | 83.33% |
| mobile | log-viewer | 14 | 24 | 20 | 83.33% |
| mobile | log-viewer | 16 | 24 | 23 | 95.83% |
| server | terminal-dark | 11 | 24 | 21 | 87.50% |
| server | terminal-dark | 12 | 24 | 24 | 100.00% |
| server | terminal-dark | 14 | 24 | 23 | 95.83% |
| server | terminal-dark | 16 | 24 | 22 | 91.67% |
| server | terminal-light | 11 | 24 | 19 | 79.17% |
| server | terminal-light | 12 | 24 | 20 | 83.33% |
| server | terminal-light | 14 | 24 | 23 | 95.83% |
| server | terminal-light | 16 | 24 | 22 | 91.67% |
| server | chat | 11 | 24 | 13 | 54.17% |
| server | chat | 12 | 24 | 16 | 66.67% |
| server | chat | 14 | 24 | 18 | 75.00% |
| server | chat | 16 | 24 | 18 | 75.00% |
| server | devtools | 11 | 24 | 19 | 79.17% |
| server | devtools | 12 | 24 | 24 | 100.00% |
| server | devtools | 14 | 24 | 23 | 95.83% |
| server | devtools | 16 | 24 | 22 | 91.67% |
| server | log-viewer | 11 | 24 | 19 | 79.17% |
| server | log-viewer | 12 | 24 | 18 | 75.00% |
| server | log-viewer | 14 | 24 | 22 | 91.67% |
| server | log-viewer | 16 | 24 | 23 | 95.83% |

## Per-image exceptions

| Variant | Image ID | Missed IDs (category) | Char <98% at +8 | Line <98% at +8 | False findings |
| --- | --- | --- | --- | --- | --- |
| mobile | terminal-dark-s1-f11 | p0(email), p1(email), p2(email), p4(phone), p7(card) | none | none | 3 |
| mobile | terminal-dark-s1-f12 | p1(email) | none | none | 0 |
| mobile | terminal-dark-s1-f14 | p0(email), p1(email), p2(email), p3(phone) | none | none | 3 |
| mobile | terminal-dark-s1-f16 | p4(phone) | none | none | 0 |
| mobile | terminal-dark-s1.5-f11 | p2(email) | none | none | 0 |
| mobile | terminal-dark-s2-f11 | p0(email) | none | none | 1 |
| mobile | terminal-dark-s2-f16 | p4(phone) | none | none | 0 |
| mobile | terminal-light-s1-f11 | p0(email), p1(email), p4(phone) | none | none | 0 |
| mobile | terminal-light-s1-f14 | p7(card) | none | none | 0 |
| mobile | terminal-light-s1.5-f11 | p0(email), p4(phone) | none | none | 1 |
| mobile | terminal-light-s1.5-f12 | p7(card) | none | none | 0 |
| mobile | terminal-light-s2-f11 | p6(card) | none | none | 0 |
| mobile | chat-s1-f11 | p2(email), p6(card), p7(card) | none | none | 1 |
| mobile | chat-s1-f12 | p7(card) | none | none | 0 |
| mobile | chat-s1-f14 | p6(card) | none | none | 0 |
| mobile | chat-s1-f16 | p6(card) | none | none | 0 |
| mobile | chat-s1.5-f11 | p1(email) | none | none | 1 |
| mobile | chat-s1.5-f12 | p4(phone), p6(card), p7(card) | none | none | 0 |
| mobile | chat-s1.5-f14 | p7(card) | none | none | 0 |
| mobile | chat-s2-f11 | p7(card) | none | none | 0 |
| mobile | chat-s2-f12 | p6(card), p7(card) | none | none | 0 |
| mobile | chat-s2-f14 | p4(phone), p6(card), p7(card) | none | none | 0 |
| mobile | devtools-s1-f11 | p0(email), p1(email), p2(email), p4(phone), p7(card) | none | none | 0 |
| mobile | devtools-s1-f12 | p0(email), p1(email), p2(email) | none | none | 1 |
| mobile | devtools-s1-f16 | p4(phone) | none | none | 0 |
| mobile | devtools-s1.5-f11 | p2(email) | none | none | 0 |
| mobile | devtools-s1.5-f14 | p4(phone) | none | none | 0 |
| mobile | devtools-s2-f11 | p0(email) | none | none | 1 |
| mobile | devtools-s2-f16 | p4(phone) | none | none | 0 |
| mobile | log-viewer-s1-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p6(card), p7(card) | none | none | 0 |
| mobile | log-viewer-s1-f12 | p7(card) | none | none | 0 |
| mobile | log-viewer-s1-f14 | p3(phone), p6(card), p7(card) | none | none | 0 |
| mobile | log-viewer-s1.5-f11 | p6(card), p7(card) | none | none | 0 |
| mobile | log-viewer-s1.5-f12 | p6(card), p7(card) | none | none | 0 |
| mobile | log-viewer-s1.5-f16 | p4(phone) | none | none | 0 |
| mobile | log-viewer-s2-f11 | p6(card), p7(card) | none | none | 0 |
| mobile | log-viewer-s2-f12 | p7(card) | none | none | 0 |
| mobile | log-viewer-s2-f14 | p6(card) | none | none | 0 |
| server | terminal-dark-s1-f14 | p6(card) | none | none | 0 |
| server | terminal-dark-s1.5-f11 | p0(email) | none | none | 1 |
| server | terminal-dark-s1.5-f16 | p4(phone) | none | none | 0 |
| server | terminal-dark-s2-f11 | p0(email), p5(card) | none | none | 1 |
| server | terminal-dark-s2-f16 | p4(phone) | none | none | 0 |
| server | terminal-light-s1-f11 | p1(email) | none | none | 1 |
| server | terminal-light-s1-f12 | p7(card) | none | none | 0 |
| server | terminal-light-s1-f14 | p5(card) | none | none | 0 |
| server | terminal-light-s1.5-f11 | p7(card) | none | none | 0 |
| server | terminal-light-s1.5-f12 | p4(phone) | none | none | 0 |
| server | terminal-light-s1.5-f16 | p5(card) | none | none | 0 |
| server | terminal-light-s2-f11 | p1(email), p4(phone), p7(card) | none | none | 1 |
| server | terminal-light-s2-f12 | p6(card), p7(card) | none | none | 0 |
| server | terminal-light-s2-f16 | p4(phone) | none | none | 0 |
| server | chat-s1-f11 | p1(email), p3(phone), p4(phone), p6(card), p7(card) | none | none | 1 |
| server | chat-s1-f12 | p6(card) | none | none | 0 |
| server | chat-s1-f14 | p2(email), p4(phone) | none | none | 1 |
| server | chat-s1-f16 | p1(email), p2(email), p4(phone), p7(card) | none | none | 2 |
| server | chat-s1.5-f11 | p5(card), p6(card), p7(card) | none | none | 0 |
| server | chat-s1.5-f12 | p2(email), p4(phone), p5(card), p6(card) | none | none | 1 |
| server | chat-s1.5-f14 | p6(card) | none | none | 0 |
| server | chat-s1.5-f16 | p3(phone) | none | none | 0 |
| server | chat-s2-f11 | p4(phone), p6(card), p7(card) | none | none | 0 |
| server | chat-s2-f12 | p2(email), p5(card), p6(card) | none | none | 1 |
| server | chat-s2-f14 | p4(phone), p6(card), p7(card) | none | none | 0 |
| server | chat-s2-f16 | p4(phone) | p3: 97.45% | none | 0 |
| server | devtools-s1-f11 | p1(email) | none | none | 1 |
| server | devtools-s1-f14 | p6(card) | none | none | 0 |
| server | devtools-s1.5-f11 | p0(email) | none | none | 0 |
| server | devtools-s1.5-f16 | p4(phone) | none | none | 0 |
| server | devtools-s2-f11 | p0(email), p4(phone), p5(card) | none | none | 1 |
| server | devtools-s2-f16 | p4(phone) | none | none | 0 |
| server | log-viewer-s1-f11 | p0(email), p1(email), p2(email), p3(phone) | none | none | 3 |
| server | log-viewer-s1-f12 | p1(email) | none | none | 1 |
| server | log-viewer-s1-f16 | p6(card) | none | none | 0 |
| server | log-viewer-s1.5-f12 | p1(email), p6(card), p7(card) | none | none | 1 |
| server | log-viewer-s1.5-f14 | p5(card), p6(card) | none | none | 0 |
| server | log-viewer-s2-f11 | p4(phone) | none | none | 0 |
| server | log-viewer-s2-f12 | p1(email), p6(card) | none | none | 0 |

## Pipeline and reference comparison

Detection uses BGR input, maximum side 960, nearest multiple of 32, ImageNet mean/std, DB probability threshold 0.3, mean box score threshold 0.6 and unclip ratio 1.5. Outer contours produce minimum-area rectangles; unclip expands the rectangle by area/perimeter distance. Crop rectification uses a perspective transform, then height-48 recognition with aspect preservation, minimum width 320, zero normalized padding, BGR mean/std 0.5 and greedy CTC collapse. Blank is dictionary index zero; space follows the supplied dictionary. No angle classifier is used. Rectangular unclip, crop interpolation and contour rounding are implementation differences from RapidOCR; the oracle comparison is not a claim of bit-equivalent preprocessing.

| Variant | Image ID | Reference lines | Rust lines | IoU-matched | Exact text matches | Character edits | Unmatched ref/Rust |
| --- | --- | --- | --- | --- | --- | --- | --- |
| mobile | terminal-dark-s1-f14 | 26 | 29 | 26 | 17 | 10 | 0/3 |
| mobile | chat-s1-f14 | 27 | 27 | 27 | 22 | 8 | 0/0 |
| mobile | terminal-light-s2-f14 | 29 | 29 | 29 | 28 | 1 | 0/0 |
| server | terminal-dark-s1-f14 | 19 | 29 | 19 | 14 | 6 | 0/10 |
| server | chat-s1-f14 | 28 | 28 | 28 | 19 | 34 | 0/0 |
| server | terminal-light-s2-f14 | 21 | 30 | 21 | 17 | 5 | 0/9 |

RapidOCR 3.10.0 / Python ONNX Runtime 1.23.2 is a correctness oracle only. CPU, classifier disabled, same official model files/dictionaries and matching thresholds. Greedy geometric line pairing uses IoU >=0.2; edit counts include substitutions, insertions and deletions on matched lines. Unmatched lines are reported separately. Full masked pair data/configuration: `out/oracle/<variant>/comparison.json`. Python results are never substituted for Rust scores or timings.

## Hardware and timing interpretation

CPU: AMD Ryzen 7 7800X3D 8-Core Processor           ; 8 physical cores / 16 logical processors, Windows AMD64. Rust uses four intra-op threads, one inter-op thread and sequential CPU execution. Sessions load once; first image warmup excluded. Decode and final JSON serialization are outside timing; all det/rec preprocessing, inference, DB geometry and CTC output construction are inside. One sample per image; maximum observed 1x sample decides the speed gate. No CPU affinity restriction, laptop simulation or statistical confidence claim. S1's default runtime threads differ, so speed comparison is descriptive. Final timing run must be performed without concurrent spike builds/oracle inference.

## Model provenance and licensing

All exports are published by the official PaddlePaddle account, pinned to immutable Hugging Face revisions. Each official model card explicitly declares `license: apache-2.0`; this is a weight-specific statement, rather than inference from a code repository license. The matching dictionary is extracted from each recognizer configuration, not borrowed from another recognizer. Full model cards/configurations remain in ignored `models/`.

| File | Measured bytes | SHA-256 | Source | License statement |
| --- | --- | --- | --- | --- |
| models/mobile_det_README.md | 1828 | 951c155673ac6c15feb85857c9024a62266e5956d15319ce578a8f508f399d66 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/e6f4fa85f00e168c862bc462aebca69eef9b3d3d/README.md) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/blob/e6f4fa85f00e168c862bc462aebca69eef9b3d3d/README.md) |
| models/mobile_det_inference.yml | 903 | 98069072e1b6b37d727fd9d9f11725faa46d6ea0de012f2ed26caea011c37699 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/e6f4fa85f00e168c862bc462aebca69eef9b3d3d/inference.yml) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/blob/e6f4fa85f00e168c862bc462aebca69eef9b3d3d/README.md) |
| models/mobile_det_inference.onnx | 4826518 | a431985659dc921974177a95adcfbb90fd9e51989a5e04d70d0b75f597b6e61d | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/e6f4fa85f00e168c862bc462aebca69eef9b3d3d/inference.onnx) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/blob/e6f4fa85f00e168c862bc462aebca69eef9b3d3d/README.md) |
| models/mobile_rec_README.md | 1768 | 7331c129fd5024b6a0abd1f510cfdfb2d7714fdcdbb00df21a0cb8b48e6683ee | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/ed152b8b495f84de93cda5709d768548a9127622/README.md) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/blob/ed152b8b495f84de93cda5709d768548a9127622/README.md) |
| models/mobile_rec_inference.yml | 148345 | 5dfeb2777f6d0db8177d8128a8acfcf6e6276dc4ac73ea3bf0dc06d6a5e85d8e | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/ed152b8b495f84de93cda5709d768548a9127622/inference.yml) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/blob/ed152b8b495f84de93cda5709d768548a9127622/README.md) |
| models/mobile_rec_inference.onnx | 16534782 | da72dc72ca4dc220df0dfde68c1dedc31c58d3e76a25871122e5056227d50092 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/ed152b8b495f84de93cda5709d768548a9127622/inference.onnx) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/blob/ed152b8b495f84de93cda5709d768548a9127622/README.md) |
| models/server_det_README.md | 1835 | 5863d8d7f86162eba740b2cae147b49330b724cf691b35f5a45a18fb7b121af8 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_det_onnx/resolve/dcf248c4dcc064c6d030db241255f9e8fbc6733a/README.md) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_det_onnx/blob/dcf248c4dcc064c6d030db241255f9e8fbc6733a/README.md) |
| models/server_det_inference.yml | 903 | 28fb721efc3634fc8aa677e474b9602cb815a91cf569ef357a7a553d7b3ce685 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_det_onnx/resolve/dcf248c4dcc064c6d030db241255f9e8fbc6733a/inference.yml) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_det_onnx/blob/dcf248c4dcc064c6d030db241255f9e8fbc6733a/README.md) |
| models/server_det_inference.onnx | 88116791 | 10803475a591f7dc623e24670fb5752ec94d39a1f8cf069aac1b6f0ce19cfc85 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_det_onnx/resolve/dcf248c4dcc064c6d030db241255f9e8fbc6733a/inference.onnx) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_det_onnx/blob/dcf248c4dcc064c6d030db241255f9e8fbc6733a/README.md) |
| models/server_rec_README.md | 1770 | 806a481f4c15a38feb7103a64ec0e14f83f6a211abb4b0a345bdc3b13145c821 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/resolve/b70df217f4fd99d14f970bad092cebe7d74cc4d1/README.md) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/blob/b70df217f4fd99d14f970bad092cebe7d74cc4d1/README.md) |
| models/server_rec_inference.yml | 148345 | 2c719dba044c4e2228aef8ff92f5f575394d75d24c16de096a33b7cfd902f66d | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/resolve/b70df217f4fd99d14f970bad092cebe7d74cc4d1/inference.yml) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/blob/b70df217f4fd99d14f970bad092cebe7d74cc4d1/README.md) |
| models/server_rec_inference.onnx | 84503027 | d9dc333c9c7b042c6dffb8e33d72b6f65c9c1d463d0a3c2f78174fea55e94752 | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/resolve/b70df217f4fd99d14f970bad092cebe7d74cc4d1/inference.onnx) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/blob/b70df217f4fd99d14f970bad092cebe7d74cc4d1/README.md) |
| models/mobile_dict.txt | 92395 | 1ea29636956177e400af712d9782e7693f3fb25f98617bed10479d2965a836fd | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/ed152b8b495f84de93cda5709d768548a9127622/inference.yml#PostProcess.character_dict) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/blob/ed152b8b495f84de93cda5709d768548a9127622/README.md) |
| models/server_dict.txt | 92395 | 1ea29636956177e400af712d9782e7693f3fb25f98617bed10479d2965a836fd | [download](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/resolve/b70df217f4fd99d14f970bad092cebe7d74cc4d1/inference.yml#PostProcess.character_dict) | [Apache-2.0 model card](https://huggingface.co/PaddlePaddle/PP-OCRv5_server_rec_onnx/blob/b70df217f4fd99d14f970bad092cebe7d74cc4d1/README.md) |

## Runtime and packaging

`ort =2.0.0-rc.10`, independent edition-2024 Cargo workspace. The initial `download-binaries` attempt selected a static pyke ORT distribution that could not link against this host MSVC runtime; it is not the measured runtime. The final runner uses `std` + `load-dynamic`, no automatic binary download, and the official Microsoft CPU runtime selected through `ORT_DYLIB_PATH`. No CUDA/DirectML provider is registered. The unused downloaded pyke archive is recorded separately in `runtime-provenance.json`; it is not a proposed redistributable dependency.

| Runtime artifact | Bytes | SHA-256 |
| --- | --- | --- |
| https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-win-x64-1.22.0.zip | 72368545 | 174c616efc0271194488642a72f1a514e01487da4dfe84c49296d66e40ebe0da |
| models/runtime/onnxruntime.dll | 12418080 | 579b636403983254346a5c1d80bd28f1519cd1e284cd204f8d4ff41f8d711559 |
| models/runtime/onnxruntime_providers_shared.dll | 22064 | ba00ea1ef846c9b909c7854bc56c51051a20f9773b3e1153dda118d4b85d0b93 |
| models/runtime/LICENSE | 1094 | c250d6278f0b47a6439fb7592b08b58a55eb9f535aa49a1db63211c3f982b674 |
| models/runtime/ThirdPartyNotices.txt | 333022 | e00f828e0a33de591a355ae6606d2625f5758da7d2c844db7821c9dd3e3647b6 |


[Microsoft ONNX Runtime 1.22.0 license](https://github.com/microsoft/onnxruntime/blob/v1.22.0/LICENSE): MIT. The extracted `ThirdPartyNotices.txt` accompanies the runtime and must remain part of packaging review. ZIP size includes build/debugging artifacts, not only inference DLLs.

| Variant | Det+rec bytes | Dictionary bytes |
| --- | --- | --- |
| mobile | 21361300 | 92395 |
| server | 172619818 | 92395 |

Models and the runtime are read exclusively from local paths after explicit acquisition; Rust has no fetch feature. **Network-disabled execution: NOT TESTED.** A Windows token membership check returned Administrator=false; the running Windows Firewall service requires elevation to install blocking rules. Adapter/firewall settings were not changed. Structural absence of downloads is not a substitute for network isolation evidence.

## Reproduction and validation

See [README.md](README.md). `provenance.json` and `runtime-provenance-official.json` carry exact URLs/hashes; ignored `out/` holds configuration, timings, raw synthetic OCR, masked scores and oracle comparison. No changes were made to `crates/`, root Cargo files or `.github/`; no commit/push/publication was performed.

| Command | Result |
| --- | --- |
| node --test test.mjs | PASS |
| cargo fmt --check | PASS |
| cargo clippy --locked --all-targets -- -D warnings | PASS |
| cargo test --locked | PASS |
| cargo deny check licenses sources | PASS |
| cargo deny check | FAIL |
| Python py_compile | PASS |

Full dependency advisory audit fails: `anyhow 1.0.100` (RUSTSEC-2026-0190), `imageproc 0.25.0` (RUSTSEC-2026-0115, RUSTSEC-2026-0116, RUSTSEC-2026-0117), and unmaintained transitive `paste` (RUSTSEC-2024-0436) / `ttf-parser` (RUSTSEC-2026-0192). License, source and bans checks pass; no advisory suppressions were added. Dependency updates were not performed under the supplied no-update boundary. Exact audit evidence is in `out/validation-5.log`. This is a synthetic-only spike, not a shipping dependency approval.

Reference line text and detection counts still differ from Rust, especially with server detection; the comparison tables expose that remaining uncertainty. These results characterize this Rust pipeline and configuration, rather than establishing the maximum accuracy of every PP-OCR deployment. Compressed release bundle size was not measured. Model-card Apache-2.0 declarations are recorded; no standalone dictionary license or training-data license review was established in this spike.

## Coordinator follow-up: detector limit and NFKC (2026-10-27)

Measured after the run above, with the same runner, corpus and scorer (unchanged). The runner's `MAX_SIDE` argument was set to 1920 and 2880 (`out/mobile-<N>/`). A second copy of the 1920 output had each recognized character NFKC-normalized independently, keeping the original wherever NFKC would change the character count (`out/mobile-1920-nfkc/`). 41 characters changed, for example full-width `（` → `(`. The original `out/scores.json` and `out/metrics-report.md` were backed up and restored; the sweep reports are `out/sweep-metrics-report.md` and `out/nfkc-metrics-report.md`.

| Variant | Scale | Email | Phone | Card |
| --- | --- | --- | --- | --- |
| mobile (960) | 1 / 1.5 / 2 | 68.3 / 93.3 / 96.7% | 77.5 / 90.0 / 92.5% | 78.3 / 86.7 / 83.3% |
| mobile-1920 | 1 / 1.5 / 2 | 81.7 / 100 / 100% | 75.0 / 80.0 / 80.0% | 93.3 / 96.7 / 100% |
| mobile-2880 | 1 / 1.5 / 2 | 81.7 / 100 / 100% | 75.0 / 85.0 / 72.5% | 93.3 / 95.0 / 95.0% |
| mobile-1920 + NFKC | 1 / 1.5 / 2 | 81.7 / 100 / 100% | 82.5 / 97.5 / 100% | 93.3 / 96.7 / 100% |

- 1920 + NFKC: the 2x recall gate passes and the 1x gate fails (email 81.7%, phone 82.5%). There are no false findings on look-alikes, and every found value is ≥98% covered at +8 px with char and line boxes alike.
- Warm time at 1920, 1x images, 4 threads, Ryzen 7 7800X3D: mean 1.215 s, max 1.769 s. The 1.5 s gate fails; laptop timing has not been measured.
- Decision for T4: PP-OCRv5 mobile, detector limit 1920, per-character NFKC, +8 px char-box padding, and a measured 2x pre-upscale for small images. The per-image budget becomes 2.5 s (a target).
