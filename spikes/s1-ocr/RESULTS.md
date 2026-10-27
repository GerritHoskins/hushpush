# S1: ocrs screenshot PII experiment

Measured locally on 2026-10-27. This is a bounded synthetic-corpus result, not a real-scene accuracy claim.

## Verdict

| Variant | Criterion | Result |
| --- | --- | --- |
| base | recall at 1x (each category) | FAIL |
| base | recall at 2x (each category) | FAIL |
| base | every found value ≥98% glyph-range coverage, word boxes +8 px | FAIL |
| base | every 1x image warm ≤1.5 s on measured desktop | PASS |
| base | warm ≤1.5 s on a 4-core laptop | NOT RUN |
| fallback | recall at 1x (each category) | FAIL |
| fallback | recall at 2x (each category) | FAIL |
| fallback | every found value ≥98% glyph-range coverage, word boxes +8 px | FAIL |
| fallback | every 1x image warm ≤1.5 s on measured desktop | FAIL |
| fallback | warm ≤1.5 s on a 4-core laptop | NOT RUN |

**S1 verdict: FAIL.** Baseline and first fallback miss recall and localization targets, so S1 does not clear the T4 gate. The four-core laptop performance criterion remains unverified.

## Method and corpus

60 images: five templates × three device scales × four fonts, viewport 1920×1080 CSS pixels. Physical dimensions are 1920×1080, 2880×1620 and 3840×2160. Each image has three emails, two phones, three published test cards and five benign look-alikes. Reserved example-domain emails deliberately remain detector-positive for this spike, bypassing the product's reserved-domain suppression. Phone rules support the exact synthetic formats requested; they do not establish international phone coverage.

Seeded xorshift32; Edge 156.0.4314.42; Node v26.3.0; pinned npm dependencies. Card fixtures are published by [Stripe](https://docs.stripe.com/testing#cards) and [Braintree](https://developer.paypal.com/braintree/docs/reference/general/testing/node/). Ground truth stores span ranges and individual non-whitespace character ranges from Range.getClientRects(), scaled by devicePixelRatio. “Glyph area” here means the union of these browser character rectangles, including font ascent/descent and advance width, not a bitmap ink mask. Fixtures are horizontally arranged, unwrapped, high contrast, with a modest text density; rotated text and realistic full-screen dense logs remain outside this result.

OCR text is reconstructed from word text with normalized whitespace and an offset-to-word map. Word-internal whitespace and word separators become spaces; line boundaries remain newlines to avoid merging unrelated numeric records. Email separators allow OCR whitespace; phones accept spaces around separators; cards allow spaces/hyphens and require Luhn, checking shorter legal endpoints if a following numeric token extends a candidate. Cards broken across OCR lines are a detector limitation. Matching canonicalizes whitespace for email and digits for phones/cards, with one-to-one occurrence assignment. No OCR spelling correction or ground-truth-guided text repair. False findings are unmatched detector outputs; the look-alike subset intersects a benign ground-truth range. Outputs contain category/plant IDs and metrics, never recognized values; only the explicitly requested synthetic truth and raw CLI JSON retain synthetic strings under ignored out/.

## Recall and false findings

| Variant | Scale | Category | Planted | Found | Recall | False on look-alikes | All false findings |
| --- | --- | --- | --- | --- | --- | --- | --- |
| base | 1 | email | 60 | 14 | 23.33% | 1 | 11 |
| base | 1 | phone | 40 | 25 | 62.50% | 0 | 1 |
| base | 1 | card | 60 | 43 | 71.67% | 3 | 4 |
| base | 1.5 | email | 60 | 10 | 16.67% | 0 | 13 |
| base | 1.5 | phone | 40 | 28 | 70.00% | 0 | 1 |
| base | 1.5 | card | 60 | 44 | 73.33% | 7 | 7 |
| base | 2 | email | 60 | 8 | 13.33% | 0 | 12 |
| base | 2 | phone | 40 | 25 | 62.50% | 0 | 2 |
| base | 2 | card | 60 | 38 | 63.33% | 7 | 12 |
| fallback | 1 | email | 60 | 13 | 21.67% | 0 | 14 |
| fallback | 1 | phone | 40 | 32 | 80.00% | 0 | 2 |
| fallback | 1 | card | 60 | 51 | 85.00% | 4 | 7 |
| fallback | 1.5 | email | 60 | 6 | 10.00% | 0 | 12 |
| fallback | 1.5 | phone | 40 | 22 | 55.00% | 0 | 1 |
| fallback | 1.5 | card | 60 | 46 | 76.67% | 9 | 12 |
| fallback | 2 | email | 60 | 2 | 3.33% | 0 | 13 |
| fallback | 2 | phone | 40 | 19 | 47.50% | 0 | 2 |
| fallback | 2 | card | 60 | 33 | 55.00% | 11 | 16 |

“Precision on look-alikes” is reported as the requested false-finding count, not a precision ratio.

## Localization

OCR quadrilaterals become axis-aligned redaction rectangles. Coverage is exact rectangle-union intersection area divided by union ground-truth character-range area; overlapping OCR boxes are counted once. Fallback coordinates map back to original device pixels before padding. Padding is in original device pixels. Both word and line boxes are evaluated; only words producing the detector match contribute in word mode, and their parent lines in line mode. Means are per-value means.

| Variant | Scale | Boxes | Found | Mean +0 px | Mean +4 px | Mean +8 px | Worst +8 px | ≥98% at +8 | End-to-end mean +8 (misses=0) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| base | 1 | word | 82 | 77.16% | 99.51% | 100.00% | 100.00% | 82/82 | 51.25% |
| base | 1 | line | 82 | 92.30% | 100.00% | 100.00% | 100.00% | 82/82 | 51.25% |
| base | 1.5 | word | 82 | 67.08% | 94.39% | 99.98% | 99.48% | 82/82 | 51.24% |
| base | 1.5 | line | 82 | 81.19% | 98.29% | 100.00% | 100.00% | 82/82 | 51.25% |
| base | 2 | word | 71 | 59.43% | 85.09% | 98.30% | 90.13% | 47/71 | 43.62% |
| base | 2 | line | 71 | 71.76% | 92.82% | 99.85% | 97.37% | 67/71 | 44.31% |
| fallback | 1 | word | 96 | 61.56% | 98.53% | 100.00% | 100.00% | 96/96 | 60.00% |
| fallback | 1 | line | 96 | 73.79% | 99.72% | 100.00% | 100.00% | 96/96 | 60.00% |
| fallback | 1.5 | word | 74 | 54.53% | 87.39% | 99.70% | 96.93% | 71/74 | 46.11% |
| fallback | 1.5 | line | 74 | 68.75% | 94.10% | 100.00% | 100.00% | 74/74 | 46.25% |
| fallback | 2 | word | 54 | 51.56% | 78.23% | 95.83% | 83.88% | 32/54 | 32.34% |
| fallback | 2 | line | 54 | 63.88% | 86.61% | 98.01% | 90.79% | 37/54 | 33.08% |

Word-box localization by category:

| Variant | Scale | Category | Found | Mean +0 px | Mean +4 px | Mean +8 px | Worst +8 px |
| --- | --- | --- | --- | --- | --- | --- | --- |
| base | 1 | email | 14 | 90.98% | 99.86% | 100.00% | 100.00% |
| base | 1 | phone | 25 | 78.16% | 99.94% | 100.00% | 100.00% |
| base | 1 | card | 43 | 72.08% | 99.14% | 100.00% | 100.00% |
| base | 1.5 | email | 10 | 80.41% | 98.64% | 100.00% | 100.00% |
| base | 1.5 | phone | 28 | 68.27% | 94.79% | 99.94% | 99.48% |
| base | 1.5 | card | 44 | 63.29% | 93.16% | 100.00% | 100.00% |
| base | 2 | email | 8 | 75.44% | 95.05% | 99.70% | 98.87% |
| base | 2 | phone | 25 | 59.60% | 86.56% | 99.41% | 96.76% |
| base | 2 | card | 38 | 55.95% | 82.03% | 97.28% | 90.13% |
| fallback | 1 | email | 13 | 78.83% | 99.64% | 100.00% | 100.00% |
| fallback | 1 | phone | 32 | 61.69% | 99.44% | 100.00% | 100.00% |
| fallback | 1 | card | 51 | 57.08% | 97.67% | 100.00% | 100.00% |
| fallback | 1.5 | email | 6 | 79.06% | 97.82% | 99.97% | 99.87% |
| fallback | 1.5 | phone | 22 | 54.45% | 87.71% | 99.78% | 98.25% |
| fallback | 1.5 | card | 46 | 51.36% | 85.88% | 99.62% | 96.93% |
| fallback | 2 | email | 2 | 78.84% | 93.96% | 99.45% | 99.02% |
| fallback | 2 | phone | 19 | 52.85% | 80.29% | 97.48% | 90.96% |
| fallback | 2 | card | 33 | 49.16% | 76.09% | 94.67% | 83.88% |

The strict localization verdict requires every found value to reach 98%; detection misses are separately reflected in recall and end-to-end coverage. Line boxes are a separate S1 fallback observation, not evidence that word localization passed.

## Timing and hardware

CPU: AMD Ryzen 7 7800X3D 8-Core Processor; 8 physical cores / 16 logical processors (Win32_Processor). Windows x64. Default runtime threading, no CPU affinity limit. Release build; ocrs-cli/ocrs 0.13.1 and rten 0.26.0. One sequential measurement per image per mode, no concurrent OCR processes.

| Variant | Original scale | Images | Cold mean s | Cold max s | Warm mean s | Warm max s |
| --- | --- | --- | --- | --- | --- | --- |
| base | 1 | 20 | 0.379 | 0.443 | 0.402 | 0.494 |
| base | 1.5 | 20 | 0.444 | 0.510 | 0.469 | 0.529 |
| base | 2 | 20 | 0.523 | 0.650 | 0.536 | 0.583 |
| fallback | 1 | 20 | 0.501 | 0.553 | 0.804 | 2.064 |
| fallback | 1.5 | 20 | 0.628 | 0.774 | 0.811 | 1.418 |
| fallback | 2 | 20 | 0.742 | 0.872 | 0.881 | 1.514 |

Cold means a fresh CLI process with cached models: process startup, model load, image decode, OCR and JSON write; it does not mean an evicted filesystem cache and excludes initial model downloads. Warm uses the Rust API, loads both models once from bytes, decodes each PNG outside timing, warms up once on the first image, then times prepare_input → detect_words → find_text_lines → recognize_text. No JSON/text serialization in warm timing. Fallback preprocessing and PNG decode are excluded from warm timing; fallback OCR inputs have twice the original width and height, reaching 7680×4320 at original 2x. The final timing refresh ran without this spike's builds or screenshot generation in parallel; other host background activity was not controlled. These samples are descriptive, not benchmark confidence intervals. The verdict uses the maximum observed sample, so a fast mean cannot hide a slow image.

## First fallback and delta

All original screenshots were bilinearly upscaled 2×; terminal-dark and devtools additionally had RGB inverted. This combined first fallback preserves font rasterization artifacts from the original screenshots. It does not isolate inversion from upscaling.

| Original scale | Category | Baseline recall | Fallback recall | Delta |
| --- | --- | --- | --- | --- |
| 1 | email | 23.33% | 21.67% | -1.67 pp |
| 1 | phone | 62.50% | 80.00% | 17.50 pp |
| 1 | card | 71.67% | 85.00% | 13.33 pp |
| 1.5 | email | 16.67% | 10.00% | -6.67 pp |
| 1.5 | phone | 70.00% | 55.00% | -15.00 pp |
| 1.5 | card | 73.33% | 76.67% | 3.33 pp |
| 2 | email | 13.33% | 3.33% | -10.00 pp |
| 2 | phone | 62.50% | 47.50% | -15.00 pp |
| 2 | card | 63.33% | 55.00% | -8.33 pp |

## Observed failure modes

| Variant | Observed text/geometry symptom | Missed values | Example image IDs |
| --- | --- | --- | --- |
| base | Email @ absent from overlapping OCR words | 108 | terminal-dark-s1-f11, terminal-dark-s1-f12, terminal-dark-s1-f14 |
| base | Canonical characters altered in overlapping OCR words | 128 | terminal-dark-s1-f11, terminal-dark-s1-f12, terminal-dark-s1-f14 |
| base | Characters survive normalization; separator or boundary mismatch | 5 | terminal-dark-s1-f14, terminal-light-s1-f14, terminal-light-s2-f12 |
| base | Email dot absent from overlapping OCR words | 4 | terminal-dark-s1.5-f11, terminal-dark-s2-f11, chat-s1.5-f11 |
| fallback | Email @ absent from overlapping OCR words | 117 | terminal-dark-s1-f11, terminal-dark-s1-f12, terminal-dark-s1-f14 |
| fallback | Canonical characters altered in overlapping OCR words | 135 | terminal-dark-s1-f11, terminal-dark-s1-f14, terminal-dark-s1.5-f11 |
| fallback | Characters survive normalization; separator or boundary mismatch | 3 | terminal-light-s1.5-f12, devtools-s1-f12, log-viewer-s2-f16 |
| fallback | Email dot absent from overlapping OCR words | 1 | log-viewer-s1.5-f11 |

Symptoms come from words geometrically overlapping each planted glyph range. They distinguish absent email punctuation and altered canonical characters from detector separator/boundary failures; they do not infer an OCR model root cause. The font/template table below shows where these occur. Scaling does not monotonically improve this corpus; the combined fallback improves 1x phone/card recall but does not reach the full gate.

## Failure breakdown

| Variant | Template | CSS font px | Planted (all scales) | Found | Recall |
| --- | --- | --- | --- | --- | --- |
| base | terminal-dark | 11 | 24 | 3 | 12.50% |
| base | terminal-dark | 12 | 24 | 8 | 33.33% |
| base | terminal-dark | 14 | 24 | 12 | 50.00% |
| base | terminal-dark | 16 | 24 | 15 | 62.50% |
| base | terminal-light | 11 | 24 | 9 | 37.50% |
| base | terminal-light | 12 | 24 | 11 | 45.83% |
| base | terminal-light | 14 | 24 | 13 | 54.17% |
| base | terminal-light | 16 | 24 | 15 | 62.50% |
| base | chat | 11 | 24 | 15 | 62.50% |
| base | chat | 12 | 24 | 13 | 54.17% |
| base | chat | 14 | 24 | 20 | 83.33% |
| base | chat | 16 | 24 | 20 | 83.33% |
| base | devtools | 11 | 24 | 5 | 20.83% |
| base | devtools | 12 | 24 | 9 | 37.50% |
| base | devtools | 14 | 24 | 12 | 50.00% |
| base | devtools | 16 | 24 | 13 | 54.17% |
| base | log-viewer | 11 | 24 | 2 | 8.33% |
| base | log-viewer | 12 | 24 | 12 | 50.00% |
| base | log-viewer | 14 | 24 | 13 | 54.17% |
| base | log-viewer | 16 | 24 | 15 | 62.50% |
| fallback | terminal-dark | 11 | 24 | 6 | 25.00% |
| fallback | terminal-dark | 12 | 24 | 10 | 41.67% |
| fallback | terminal-dark | 14 | 24 | 12 | 50.00% |
| fallback | terminal-dark | 16 | 24 | 14 | 58.33% |
| fallback | terminal-light | 11 | 24 | 3 | 12.50% |
| fallback | terminal-light | 12 | 24 | 10 | 41.67% |
| fallback | terminal-light | 14 | 24 | 13 | 54.17% |
| fallback | terminal-light | 16 | 24 | 13 | 54.17% |
| fallback | chat | 11 | 24 | 14 | 58.33% |
| fallback | chat | 12 | 24 | 14 | 58.33% |
| fallback | chat | 14 | 24 | 17 | 70.83% |
| fallback | chat | 16 | 24 | 19 | 79.17% |
| fallback | devtools | 11 | 24 | 4 | 16.67% |
| fallback | devtools | 12 | 24 | 10 | 41.67% |
| fallback | devtools | 14 | 24 | 12 | 50.00% |
| fallback | devtools | 16 | 24 | 14 | 58.33% |
| fallback | log-viewer | 11 | 24 | 3 | 12.50% |
| fallback | log-viewer | 12 | 24 | 12 | 50.00% |
| fallback | log-viewer | 14 | 24 | 12 | 50.00% |
| fallback | log-viewer | 16 | 24 | 12 | 50.00% |

Per-image exceptions below list missed plant IDs and poorly covered matched values. Use ignored out/<ID>.json to locate a plant without exposing its value in this report. No exceptions means no observed miss, under-coverage or false finding in that image.

| Variant | Image ID | Missed IDs (category) | Found but <98% coverage at +8 | False findings |
| --- | --- | --- | --- | --- |
| base | terminal-dark-s1-f11 | p0(email), p1(email), p2(email), p4(phone), p5(card) | none | 0 |
| base | terminal-dark-s1-f12 | p0(email), p1(email), p2(email), p3(phone), p5(card), p7(card) | none | 1 |
| base | terminal-dark-s1-f14 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card) | none | 1 |
| base | terminal-dark-s1-f16 | p1(email), p2(email), p3(phone) | none | 0 |
| base | terminal-dark-s1.5-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card), p7(card) | none | 1 |
| base | terminal-dark-s1.5-f12 | p0(email), p1(email), p2(email), p4(phone), p7(card) | none | 1 |
| base | terminal-dark-s1.5-f14 | p0(email), p1(email) | none | 0 |
| base | terminal-dark-s1.5-f16 | p0(email), p1(email), p2(email) | none | 1 |
| base | terminal-dark-s2-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card), p7(card) | none | 2 |
| base | terminal-dark-s2-f12 | p0(email), p1(email), p2(email), p6(card), p7(card) | none | 1 |
| base | terminal-dark-s2-f14 | p0(email), p1(email), p2(email), p3(phone) | p4: 97.06%, p5: 94.12%, p6: 97.06%, p7: 96.32% | 1 |
| base | terminal-dark-s2-f16 | p0(email), p1(email), p2(email) | p3: 97.54%, p4: 96.96%, p5: 96.05%, p6: 92.11%, p7: 90.13% | 0 |
| base | terminal-light-s1-f11 | p0(email), p1(email), p2(email), p6(card), p7(card) | none | 1 |
| base | terminal-light-s1-f12 | p0(email), p1(email), p3(phone), p4(phone), p7(card) | none | 0 |
| base | terminal-light-s1-f14 | p0(email), p1(email), p2(email), p3(phone), p7(card) | none | 1 |
| base | terminal-light-s1-f16 | p0(email), p1(email), p2(email), p3(phone) | none | 1 |
| base | terminal-light-s1.5-f11 | p0(email), p1(email), p2(email), p4(phone), p5(card), p7(card) | none | 1 |
| base | terminal-light-s1.5-f12 | p0(email), p1(email), p7(card) | none | 1 |
| base | terminal-light-s1.5-f14 | p0(email), p1(email) | none | 1 |
| base | terminal-light-s1.5-f16 | p0(email), p2(email) | none | 0 |
| base | terminal-light-s2-f11 | p0(email), p2(email), p5(card), p6(card) | none | 1 |
| base | terminal-light-s2-f12 | p0(email), p1(email), p2(email), p6(card), p7(card) | none | 1 |
| base | terminal-light-s2-f14 | p0(email), p1(email), p2(email), p7(card) | p6: 97.06% | 1 |
| base | terminal-light-s2-f16 | p1(email), p2(email), p3(phone) | p4: 96.76%, p5: 93.42%, p6: 97.37%, p7: 95.39% | 1 |
| base | chat-s1-f11 | p1(email), p6(card) | none | 2 |
| base | chat-s1-f12 | p0(email), p1(email), p2(email) | none | 4 |
| base | chat-s1-f14 | p7(card) | none | 0 |
| base | chat-s1-f16 | none | none | 1 |
| base | chat-s1.5-f11 | p0(email), p1(email), p2(email), p6(card) | none | 3 |
| base | chat-s1.5-f12 | p0(email), p1(email), p2(email) | none | 4 |
| base | chat-s1.5-f14 | p1(email), p4(phone) | none | 2 |
| base | chat-s1.5-f16 | p1(email) | none | 2 |
| base | chat-s2-f11 | p1(email), p2(email), p6(card) | none | 3 |
| base | chat-s2-f12 | p0(email), p1(email), p2(email), p3(phone), p4(phone) | none | 4 |
| base | chat-s2-f14 | p1(email) | none | 2 |
| base | chat-s2-f16 | p0(email), p1(email), p6(card) | none | 3 |
| base | devtools-s1-f11 | p0(email), p1(email), p2(email), p5(card) | none | 0 |
| base | devtools-s1-f12 | p0(email), p1(email), p2(email), p3(phone), p7(card) | none | 0 |
| base | devtools-s1-f14 | p0(email), p1(email), p2(email), p3(phone), p5(card) | none | 0 |
| base | devtools-s1-f16 | p1(email), p3(phone), p4(phone) | none | 0 |
| base | devtools-s1.5-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card), p7(card) | none | 0 |
| base | devtools-s1.5-f12 | p0(email), p1(email), p3(phone), p4(phone), p7(card) | none | 1 |
| base | devtools-s1.5-f14 | p0(email), p2(email) | none | 1 |
| base | devtools-s1.5-f16 | p0(email), p1(email), p2(email), p3(phone) | none | 1 |
| base | devtools-s2-f11 | p0(email), p1(email), p2(email), p4(phone), p5(card), p6(card), p7(card) | none | 2 |
| base | devtools-s2-f12 | p0(email), p1(email), p2(email), p6(card), p7(card) | none | 1 |
| base | devtools-s2-f14 | p0(email), p1(email), p2(email), p3(phone), p4(phone) | p5: 94.12%, p6: 97.06%, p7: 95.59% | 0 |
| base | devtools-s2-f16 | p0(email), p1(email), p2(email), p3(phone) | p4: 96.96%, p5: 96.05%, p6: 92.11%, p7: 90.13% | 0 |
| base | log-viewer-s1-f11 | p0(email), p1(email), p2(email), p3(phone), p6(card), p7(card) | none | 3 |
| base | log-viewer-s1-f12 | p0(email), p1(email), p3(phone), p5(card) | none | 1 |
| base | log-viewer-s1-f14 | p0(email), p1(email), p2(email), p7(card) | none | 0 |
| base | log-viewer-s1-f16 | p0(email), p1(email) | none | 0 |
| base | log-viewer-s1.5-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card), p7(card) | none | 0 |
| base | log-viewer-s1.5-f12 | p0(email), p1(email), p2(email), p7(card) | none | 0 |
| base | log-viewer-s1.5-f14 | p0(email), p1(email), p2(email) | none | 1 |
| base | log-viewer-s1.5-f16 | p0(email), p1(email), p2(email) | none | 0 |
| base | log-viewer-s2-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card), p7(card) | none | 0 |
| base | log-viewer-s2-f12 | p0(email), p1(email), p2(email), p7(card) | none | 2 |
| base | log-viewer-s2-f14 | p0(email), p1(email), p4(phone), p7(card) | none | 1 |
| base | log-viewer-s2-f16 | p1(email), p2(email), p3(phone), p4(phone) | p5: 96.05%, p6: 94.74%, p7: 94.74% | 0 |
| fallback | terminal-dark-s1-f11 | p0(email), p2(email), p4(phone), p5(card), p7(card) | none | 1 |
| fallback | terminal-dark-s1-f12 | p0(email), p1(email) | none | 0 |
| fallback | terminal-dark-s1-f14 | p0(email), p1(email), p2(email), p4(phone) | none | 1 |
| fallback | terminal-dark-s1-f16 | p0(email), p1(email), p2(email) | none | 0 |
| fallback | terminal-dark-s1.5-f11 | p0(email), p1(email), p2(email), p3(phone), p5(card), p7(card) | none | 1 |
| fallback | terminal-dark-s1.5-f12 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p7(card) | none | 1 |
| fallback | terminal-dark-s1.5-f14 | p0(email), p1(email), p2(email) | none | 1 |
| fallback | terminal-dark-s1.5-f16 | p1(email), p2(email), p3(phone) | p7: 96.93% | 0 |
| fallback | terminal-dark-s2-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card) | none | 1 |
| fallback | terminal-dark-s2-f12 | p0(email), p1(email), p2(email), p3(phone), p6(card), p7(card) | none | 1 |
| fallback | terminal-dark-s2-f14 | p0(email), p1(email), p2(email), p5(card), p7(card) | p3: 94.12%, p6: 89.71% | 1 |
| fallback | terminal-dark-s2-f16 | p0(email), p1(email), p2(email), p6(card) | p3: 90.96%, p4: 93.72%, p5: 87.17%, p7: 83.88% | 0 |
| fallback | terminal-light-s1-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p7(card) | none | 1 |
| fallback | terminal-light-s1-f12 | p1(email), p2(email) | none | 2 |
| fallback | terminal-light-s1-f14 | p0(email), p1(email), p2(email), p3(phone) | none | 0 |
| fallback | terminal-light-s1-f16 | p0(email), p1(email), p2(email) | none | 1 |
| fallback | terminal-light-s1.5-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p7(card) | none | 1 |
| fallback | terminal-light-s1.5-f12 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p7(card) | none | 1 |
| fallback | terminal-light-s1.5-f14 | p0(email), p1(email), p2(email) | none | 0 |
| fallback | terminal-light-s1.5-f16 | p0(email), p1(email), p2(email), p3(phone) | none | 1 |
| fallback | terminal-light-s2-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card) | none | 1 |
| fallback | terminal-light-s2-f12 | p0(email), p1(email), p2(email), p3(phone), p6(card), p7(card) | none | 1 |
| fallback | terminal-light-s2-f14 | p0(email), p1(email), p2(email), p5(card) | p3: 94.12%, p4: 95.25%, p6: 89.71%, p7: 94.85% | 1 |
| fallback | terminal-light-s2-f16 | p0(email), p1(email), p2(email), p3(phone) | p4: 91.60%, p5: 87.83%, p6: 86.84%, p7: 83.88% | 2 |
| fallback | chat-s1-f11 | p0(email), p1(email) | none | 3 |
| fallback | chat-s1-f12 | p0(email), p2(email) | none | 3 |
| fallback | chat-s1-f14 | p1(email), p2(email) | none | 3 |
| fallback | chat-s1-f16 | p1(email) | none | 2 |
| fallback | chat-s1.5-f11 | p0(email), p1(email), p2(email), p6(card) | none | 3 |
| fallback | chat-s1.5-f12 | p0(email), p1(email), p2(email) | none | 4 |
| fallback | chat-s1.5-f14 | p1(email), p4(phone) | none | 2 |
| fallback | chat-s1.5-f16 | none | none | 1 |
| fallback | chat-s2-f11 | p0(email), p1(email), p2(email), p6(card) | none | 4 |
| fallback | chat-s2-f12 | p0(email), p1(email), p2(email), p3(phone), p4(phone) | none | 3 |
| fallback | chat-s2-f14 | p1(email), p2(email), p3(phone) | none | 3 |
| fallback | chat-s2-f16 | p0(email), p1(email), p2(email), p6(card) | none | 4 |
| fallback | devtools-s1-f11 | p0(email), p1(email), p2(email), p3(phone), p5(card), p7(card) | none | 1 |
| fallback | devtools-s1-f12 | p0(email), p1(email), p2(email) | none | 2 |
| fallback | devtools-s1-f14 | p0(email), p1(email), p2(email) | none | 0 |
| fallback | devtools-s1-f16 | p1(email), p2(email) | none | 0 |
| fallback | devtools-s1.5-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p7(card) | none | 2 |
| fallback | devtools-s1.5-f12 | p0(email), p1(email), p2(email), p4(phone), p7(card) | none | 1 |
| fallback | devtools-s1.5-f14 | p0(email), p1(email), p2(email) | none | 2 |
| fallback | devtools-s1.5-f16 | p0(email), p1(email), p2(email), p3(phone) | p7: 96.93% | 1 |
| fallback | devtools-s2-f11 | p0(email), p1(email), p2(email), p3(phone), p5(card), p6(card), p7(card) | none | 1 |
| fallback | devtools-s2-f12 | p0(email), p1(email), p2(email), p4(phone), p6(card), p7(card) | none | 1 |
| fallback | devtools-s2-f14 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card) | p6: 89.71%, p7: 90.07% | 1 |
| fallback | devtools-s2-f16 | p1(email), p2(email), p3(phone), p4(phone) | p5: 87.50%, p6: 90.79%, p7: 83.88% | 1 |
| fallback | log-viewer-s1-f11 | p0(email), p2(email), p4(phone), p6(card), p7(card) | none | 1 |
| fallback | log-viewer-s1-f12 | p1(email), p4(phone), p7(card) | none | 2 |
| fallback | log-viewer-s1-f14 | p0(email), p1(email), p2(email) | none | 0 |
| fallback | log-viewer-s1-f16 | p0(email), p1(email) | none | 0 |
| fallback | log-viewer-s1.5-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card), p7(card) | none | 1 |
| fallback | log-viewer-s1.5-f12 | p0(email), p1(email), p2(email), p4(phone), p7(card) | none | 2 |
| fallback | log-viewer-s1.5-f14 | p0(email), p1(email), p2(email) | none | 0 |
| fallback | log-viewer-s1.5-f16 | p0(email), p1(email), p2(email), p3(phone) | p5: 97.37% | 0 |
| fallback | log-viewer-s2-f11 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p5(card), p6(card), p7(card) | none | 2 |
| fallback | log-viewer-s2-f12 | p0(email), p1(email), p2(email), p7(card) | none | 1 |
| fallback | log-viewer-s2-f14 | p0(email), p1(email), p2(email), p3(phone), p5(card), p7(card) | p4: 94.68% | 2 |
| fallback | log-viewer-s2-f16 | p0(email), p1(email), p2(email), p3(phone), p4(phone), p7(card) | p5: 92.43%, p6: 88.16% | 0 |

## Observed CLI JSON contract

Actual keys: image_height, image_width, paragraphs, url. paragraphs[].lines[] has text, vertices, words; words[] has text and vertices. Both word and line polygons contain four [x,y] integer vertices. Coordinates align with original raster/device pixels (verified across scales against the planted ranges); they are not CSS coordinates. No confidence scores observed. The CLI groups all lines into one paragraph.

**Metadata bug in installed 0.13.1:** a 1920×1080 PNG reports image_width=3, image_height=1920. The scorer ignores those fields and uses PNG/ground-truth geometry. Word coordinates themselves remain usable; consumers must validate dimensions against their decoder. Raw CLI JSON is in out/{base,fallback}/<ID>.ocr.json.

## S3 notes

| Cached model filename | Measured bytes | SHA-256 |
| --- | --- | --- |
| text-detection.onnx | 2499479 | a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe |
| text-recognition.onnx | 9713177 | 86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5 |

The CLI fetched these ONNX files from https://ocrs-models.s3-accelerate.amazonaws.com/ into ~/.cache/ocrs on first invocation. Model acquisition was completed before timed runs. No model files are committed here. Combined uncompressed size: 12212656 bytes. Compressed release archive size and network-disabled execution: **not measured**.

The bench actually loads both files via Model::load(Vec<u8>) and passes them into OcrEngineParams, proving byte-loading on these exact versions. [RTen Model API](https://docs.rs/rten/0.26.0/rten/struct.Model.html#method.load) documents byte loading and load_static_slice; [ocrs OcrEngineParams](https://docs.rs/ocrs/0.13.1/ocrs/struct.OcrEngineParams.html) accepts optional rten::Model instances for detection and recognition. Thus embedded bytes are feasible; a shipping binary still needs independent packaging and offline validation.

The [upstream model card](https://huggingface.co/robertknight/ocrs) explicitly labels pretrained weights **CC-BY-SA-4.0**, and says training uses HierText and synthetic data. The [training repository](https://github.com/robertknight/ocrs-models#datasets) identifies HierText as CC-BY-SA-4.0. These terms differ from the library's MIT/Apache-2.0 source licenses. The S3-download endpoint itself provided no accompanying model license during this run; equivalence of the exact S3 weights to the model-card files was not established. Redistribution under the repository's permissive-only policy is therefore **unresolved**, not approved by this spike.

## Evidence and reproducibility

See README.md for the three-command run. out/manifest.json holds PNG and truth hashes; environment.json records CPU/models/browser; {base,fallback}/{cold,warm}.json hold per-image timings; scores.json holds masked scoring evidence. The generator reproducibility check, tests, and Rust quality gates are recorded in VALIDATION.md.
