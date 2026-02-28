from __future__ import annotations

import json
from pathlib import Path

from benchmarks.report import render_markdown
from benchmarks.run_benchmarks import CIRCUITS, run


def test_run_simulator_writes_expected_payload(tmp_path, monkeypatch):
    monkeypatch.setattr("benchmarks.run_benchmarks.RESULTS_DIR", tmp_path)

    result = run("sim")
    output_path = Path(result["output"])

    assert output_path.exists()
    payload = json.loads(output_path.read_text(encoding="utf-8"))
    assert payload["backend"] == "simulator"
    assert payload["shots"] == 1024
    assert len(payload["results"]) == len(CIRCUITS)

    for row in payload["results"]:
        assert row["backend"] == "simulator"
        assert row["fidelity"] >= 0.95
        assert row["gate_count"] >= 1
        assert row["depth"] >= 1
        assert row["shots"] == 1024


def test_render_markdown_formats_table(tmp_path):
    src = tmp_path / "simulator.json"
    src.write_text(
        json.dumps(
            {
                "backend": "simulator",
                "shots": 1024,
                "results": [
                    {
                        "circuit": "Bell state",
                        "backend": "simulator",
                        "fidelity": 1.0,
                        "gate_count": 3,
                        "depth": 2,
                        "shots": 1024,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    rendered = render_markdown(src)

    assert "| Circuit | Backend | Fidelity | Gate Count | Depth | Shots |" in rendered
    assert "| Bell state | simulator | 100.0% | 3 | 2 | 1024 |" in rendered
