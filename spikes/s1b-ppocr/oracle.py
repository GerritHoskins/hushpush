"""Bounded RapidOCR correctness oracle; never used for the Rust timing gate."""

import argparse
import hashlib
import json
from importlib.metadata import version
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CORPUS = ROOT.parent / "s1-ocr" / "out"


class Sensitive:
    def __init__(self, value):
        self.__value = value

    def expose(self):
        return self.__value

    def __repr__(self):
        return "[redacted]"

    __str__ = __repr__


def edit_distance(left, right):
    previous = list(range(len(right) + 1))
    for i, a in enumerate(left, 1):
        current = [i]
        for j, b in enumerate(right, 1):
            current.append(min(current[-1] + 1, previous[j] + 1, previous[j - 1] + (a != b)))
        previous = current
    return previous[-1]


def bounds(vertices):
    return min(p[0] for p in vertices), min(p[1] for p in vertices), max(p[0] for p in vertices), max(p[1] for p in vertices)


def iou(left, right):
    a, b = bounds(left), bounds(right)
    intersection = max(0, min(a[2], b[2]) - max(a[0], b[0])) * max(0, min(a[3], b[3]) - max(a[1], b[1]))
    union = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - intersection
    return intersection / union if union else 0


def compare(reference, actual):
    candidates = sorted(((iou(a["vertices"], b["vertices"]), i, j) for i, a in enumerate(reference) for j, b in enumerate(actual)), reverse=True)
    used_reference, used_actual, pairs = set(), set(), []
    for overlap, i, j in candidates:
        if overlap < 0.2 or i in used_reference or j in used_actual:
            continue
        used_reference.add(i)
        used_actual.add(j)
        left, right = Sensitive(reference[i]["text"]), Sensitive(actual[j]["text"])
        distance = edit_distance(left.expose(), right.expose())
        pairs.append({"oracle_line": i, "rust_line": j, "iou": overlap, "oracle_characters": len(left.expose()), "rust_characters": len(right.expose()), "edit_distance": distance, "normalized_distance": distance / max(len(left.expose()), len(right.expose()), 1), "exact": distance == 0})
    return {"oracle_lines": len(reference), "rust_lines": len(actual), "matched_lines": len(pairs), "unmatched_oracle_lines": len(reference) - len(pairs), "unmatched_rust_lines": len(actual) - len(pairs), "exact_lines": sum(p["exact"] for p in pairs), "total_edit_distance": sum(p["edit_distance"] for p in pairs), "pairs": pairs}


def verify_corpus(manifest):
    for item in manifest["images"]:
        if hashlib.sha256((CORPUS / f'{item["id"]}.png').read_bytes()).hexdigest() != item["sha256"]:
            raise RuntimeError("Synthetic image hash mismatch")
    # Truth hashes use the generator's compact JSON serialization. PNG checks
    # here complement the Node scorer tests, which check both artifact types.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", default="mobile", choices=["mobile", "server"])
    parser.add_argument("--ids", nargs="+", default=["terminal-dark-s1-f14", "chat-s1-f14", "terminal-light-s2-f14"])
    args = parser.parse_args()
    manifest = json.loads((CORPUS / "manifest.json").read_text())
    allowed = {item["id"] for item in manifest["images"]}
    if not set(args.ids) <= allowed:
        raise RuntimeError("Oracle accepts only existing synthetic manifest image IDs")
    verify_corpus(manifest)
    model_dir = ROOT / "models"
    paths = {"det": model_dir / f"{args.variant}_det_inference.onnx", "rec": model_dir / f"{args.variant}_rec_inference.onnx", "dict": model_dir / f"{args.variant}_dict.txt"}
    if not all(path.is_file() for path in paths.values()):
        raise RuntimeError("Download/extract the pinned local models and dictionary first")
    for image_id in args.ids:
        if not (ROOT / "out" / args.variant / f"{image_id}.ocr.json").is_file():
            raise RuntimeError("Run the Rust runner before the correctness oracle")
    import onnxruntime
    from rapidocr import RapidOCR
    from rapidocr.utils.typings import ModelType, OCRVersion

    config = {
        "Global.use_cls": False, "Global.use_preprocess_img": False,
        "Global.use_vertical_padding": False, "Global.text_score": 0.0,
        "Global.log_level": "critical", "Global.model_root_dir": str(model_dir),
        "Det.model_path": str(paths["det"]), "Det.ocr_version": OCRVersion.PPOCRV5,
        "Det.model_type": ModelType(args.variant), "Det.limit_side_len": 960,
        "Det.limit_type": "max", "Det.mean": [0.485, 0.456, 0.406],
        "Det.std": [0.229, 0.224, 0.225], "Det.thresh": 0.3,
        "Det.box_thresh": 0.6, "Det.unclip_ratio": 1.5,
        "Det.use_dilation": False, "Det.score_mode": "fast",
        "Rec.model_path": str(paths["rec"]), "Rec.rec_keys_path": str(paths["dict"]),
        "Rec.ocr_version": OCRVersion.PPOCRV5, "Rec.model_type": ModelType(args.variant),
        "Rec.rec_img_shape": [3, 48, 320], "Rec.rec_batch_num": 1,
        "EngineConfig.onnxruntime.intra_op_num_threads": 4,
        "EngineConfig.onnxruntime.inter_op_num_threads": 1,
        "EngineConfig.onnxruntime.use_cuda": False,
        "EngineConfig.onnxruntime.use_dml": False,
    }
    engine = RapidOCR(params=config)
    output = ROOT / "out" / "oracle" / args.variant
    output.mkdir(parents=True, exist_ok=True)
    summaries = []
    for image_id in args.ids:
        result = engine(str(CORPUS / f"{image_id}.png"), use_cls=False)
        lines = []
        if result.boxes is not None and result.txts is not None:
            for vertices, text in zip(result.boxes, result.txts):
                value = Sensitive(text)
                lines.append({"text": value.expose(), "vertices": vertices.tolist()})
        # Raw recognized synthetic text is isolated in the ignored oracle dir.
        (output / f"{image_id}.ocr.json").write_text(json.dumps({"paragraphs": [{"lines": lines}]}, ensure_ascii=False), encoding="utf-8")
        actual = json.loads((ROOT / "out" / args.variant / f"{image_id}.ocr.json").read_text(encoding="utf-8"))
        summary = {"id": image_id, **compare(lines, [line for paragraph in actual["paragraphs"] for line in paragraph["lines"]])}
        summaries.append(summary)
        print(json.dumps({key: value for key, value in summary.items() if key != "pairs"}))
    report = {"rapidocr_version": version("rapidocr"), "onnxruntime_version": onnxruntime.__version__, "config": config, "comparison": summaries}
    (output / "comparison.json").write_text(json.dumps(report, indent=2, default=lambda value: value.value), encoding="utf-8")


if __name__ == "__main__":
    main()
