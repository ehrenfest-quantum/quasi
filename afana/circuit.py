from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List


@dataclass(frozen=True)
class Operation:
    gate: str
    qubits: List[int]
    meta: Dict[str, Any] = field(default_factory=dict)


@dataclass
class Circuit:
    n_qubits: int
    operations: List[Operation] = field(default_factory=list)

    def add(self, gate: str, qubits: List[int], **meta: Any) -> None:
        self.operations.append(Operation(gate=gate, qubits=list(qubits), meta=dict(meta)))

    def gate_count(self) -> int:
        return len(self.operations)
