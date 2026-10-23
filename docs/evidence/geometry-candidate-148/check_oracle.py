"""Independent dense-Bezier reference for the pinned flat CLI fixture only.

Usage: python3 check_oracle.py REPO CURVE_OUTPUT DOOR_OUTPUT
Does not invoke Rust, change Store, or assert general geometry correctness.
"""
import hashlib
import json
import math
import sys
from pathlib import Path


def length(road):
    a, b, c = (road[k] for k in ("start", "control", "end"))
    previous = (a["x"], a["y"])
    total = 0.0
    for i in range(1, 100001):
        t = i / 100000
        current = tuple((1-t)**2*a[k] + 2*(1-t)*t*b[k] + t*t*c[k]
                        for k in ("x", "y"))
        total += math.dist(previous, current)
        previous = current
    return total


repo = Path(sys.argv[1])
fixture = json.loads((repo / "fixtures/m0-spatial.json").read_bytes())["scene"]
results = []
for directory, kind in zip(map(Path, sys.argv[2:]), ("curve", "door-width")):
    report = json.loads((directory / "candidate-report.json").read_bytes())
    scene = json.loads((directory / "candidate-scene-input.json").read_bytes())
    raw = (directory / "candidate-profile.json").read_bytes()
    profile = json.loads(raw)
    revisions = report["revisions"]
    movement = report["movement"]
    edges = profile["opening"]["edges"]
    checks = {
        "candidate_only": report["result"] == "candidate_only_not_activated"
            and report["persistence"] == "not_written_to_store",
        "exact_profile_hash": hashlib.sha256(raw).hexdigest()
            == report["source_sha256"]["candidate_profile"],
        "separate_revision_increments": all(revisions["candidate_"+k] ==
            revisions["parent_"+k]+1 for k in ("geometry_revision", "transport_revision")),
        "serialized_revisions": scene["geometry_revision"] == revisions["candidate_geometry_revision"]
            and profile["opening"]["revision"] == revisions["candidate_transport_revision"]
            and profile["assignment"]["network_revision"] == revisions["candidate_transport_revision"],
        "endpoints_unchanged": all(scene["road"][k] == fixture["road"][k]
            for k in ("start", "end")),
        "explicit_capacity_unchanged": all(e["capacity_people"] == 4 for e in edges),
        "edge_ids": [e["id"] for e in edges] == [21, 22, 23, 24],
        "served_four": movement["candidate_served_people_per_period"] == 4
            and profile["assignment"]["served_people"] == 4,
        "road_time_reference": movement["candidate_edge_free_flow_seconds"][2:]
            == [math.ceil(length(scene["road"])/100)]*2,
        "parent_road_time_reference": movement["prior_edge_free_flow_seconds"][2:]
            == [math.ceil(length(fixture["road"])/100)]*2,
        "full_outputs": all((directory/n).is_file() for n in
            ("candidate-report.json", "candidate-scene-input.json", "candidate-profile.json")),
    }
    if kind == "door-width":
        checks["no_invented_time_gain"] = movement["candidate_edge_free_flow_seconds"] == movement["prior_edge_free_flow_seconds"]
    assert all(checks.values()), checks
    results.append({"directory": str(directory), "kind": kind, "checks": checks,
                    "dense_length_cm": length(scene["road"])})
assert len(results) == 2
print(json.dumps({"passed": True, "scope": "pinned_flat_fixture_only", "results": results}, indent=2))
