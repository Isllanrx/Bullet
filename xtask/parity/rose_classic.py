import json
import sys
from pathlib import Path


def main() -> int:
    rose_root, manifest_path, result_path = (Path(a) for a in sys.argv[1:4])
    sys.path.insert(0, str(rose_root))
    from injection.classic.classic_skin_builder import (  # noqa: E402  (path set just above)
        ClassicSkinError,
        build_classic_mod,
        load_jade_characters,
    )

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    game_dir = Path(manifest["game_dir"])
    out_dir = Path(manifest["out_dir"])
    out_dir.mkdir(parents=True, exist_ok=True)
    known = load_jade_characters(Path(manifest["hashes"]), Path(manifest["cache"]))

    results = []
    for case in manifest["cases"]:
        entry = {"alias": case["alias"], "skin": case["skin"]}
        try:
            entry["folder"] = build_classic_mod(
                game_dir, case["alias"], case["skin"], out_dir, known, case["slots"]
            )
        except ClassicSkinError as e:
            entry["error"] = str(e)
        except Exception as e:  # a crash in Rose is a result too, never a harness failure
            entry["error"] = f"{type(e).__name__}: {e}"
        results.append(entry)

    result_path.write_text(json.dumps({"known": sorted(known), "results": results}), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
