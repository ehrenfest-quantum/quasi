"""ZX-IR optimization pass with identity reduction rules."""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Dict, List, Set, Tuple, Optional
import math


@dataclass
class ZXNode:
    """A node in the ZX-calculus intermediate representation."""
    id: int
    node_type: str  # "Z", "X", "H", "input", "output"
    phase: float = 0.0
    qubit: Optional[int] = None


@dataclass
class ZXEdge:
    """An edge connecting two ZX nodes."""
    source: int
    target: int
    edge_type: str = "simple"  # "simple" or "hadamard"


@dataclass
class ZXGraph:
    """A ZX-calculus graph for quantum circuit optimization."""
    nodes: Dict[int, ZXNode] = field(default_factory=dict)
    edges: List[ZXEdge] = field(default_factory=list)
    next_id: int = 0

    def add_node(self, node_type: str, phase: float = 0.0, qubit: Optional[int] = None) -> int:
        """Add a node and return its ID."""
        node_id = self.next_id
        self.next_id += 1
        self.nodes[node_id] = ZXNode(node_id, node_type, phase, qubit)
        return node_id

    def add_edge(self, source: int, target: int, edge_type: str = "simple") -> None:
        """Add an edge between two nodes."""
        self.edges.append(ZXEdge(source, target, edge_type))

    def remove_node(self, node_id: int) -> None:
        """Remove a node and all its incident edges."""
        if node_id in self.nodes:
            del self.nodes[node_id]
        self.edges = [e for e in self.edges if e.source != node_id and e.target != node_id]

    def get_neighbors(self, node_id: int) -> List[Tuple[int, str]]:
        """Get neighbors of a node with edge types."""
        neighbors = []
        for edge in self.edges:
            if edge.source == node_id:
                neighbors.append((edge.target, edge.edge_type))
            elif edge.target == node_id:
                neighbors.append((edge.source, edge.edge_type))
        return neighbors

    def node_count(self) -> int:
        """Return the number of nodes in the graph."""
        return len(self.nodes)

    def edge_count(self) -> int:
        """Return the number of edges in the graph."""
        return len(self.edges)

    def clone(self) -> "ZXGraph":
        """Create a deep copy of the graph."""
        new_graph = ZXGraph()
        new_graph.next_id = self.next_id
        for node_id, node in self.nodes.items():
            new_graph.nodes[node_id] = ZXNode(node_id, node.node_type, node.phase, node.qubit)
        new_graph.edges = [ZXEdge(e.source, e.target, e.edge_type) for e in self.edges]
        return new_graph


class ZXOptimizer:
    """Optimizer applying ZX-calculus rewrite rules for circuit simplification."""

    def __init__(self, graph: ZXGraph):
        self.graph = graph

    def optimize(self) -> ZXGraph:
        """Apply all optimization passes and return the optimized graph."""
        changed = True
        iterations = 0
        max_iterations = 100  # Prevent infinite loops

        while changed and iterations < max_iterations:
            changed = False
            iterations += 1

            # Apply identity reduction rules
            if self._remove_identity_spiders():
                changed = True
            if self._merge_spiders():
                changed = True
            if self._remove_parallel_edges():
                changed = True
            if self._remove_hadamard_loops():
                changed = True
            if self._fuse_complementary_phases():
                changed = True

        return self.graph

    def _remove_identity_spiders(self) -> bool:
        """Remove Z-spiders with phase 0 connected to a single wire.
        
        Rule: A Z-spider with phase 0 and degree 1 (connected to one edge)
        can be removed, replacing the edge with a direct connection.
        """
        changed = False
        to_remove: Set[int] = set()

        for node_id, node in self.graph.nodes.items():
            if node.node_type not in ("Z", "X"):
                continue
            if not math.isclose(node.phase, 0.0, abs_tol=1e-10):
                continue

            neighbors = self.graph.get_neighbors(node_id)
            if len(neighbors) == 1:
                # Identity spider - can be removed
                to_remove.add(node_id)

        # Remove identity spiders and reconnect their neighbors
        for node_id in to_remove:
            neighbors = self.graph.get_neighbors(node_id)
            if len(neighbors) == 1:
                neighbor_id, edge_type = neighbors[0]
                # Remove the edge to this node
                self.graph.edges = [
                    e for e in self.graph.edges
                    if not ((e.source == node_id and e.target == neighbor_id) or
                            (e.target == node_id and e.source == neighbor_id))
                ]
                self.graph.remove_node(node_id)
                changed = True

        return changed

    def _merge_spiders(self) -> bool:
        """Merge spiders of the same type connected by a simple edge.
        
        Rule: Two Z-spiders (or X-spiders) connected by a simple edge
        can be merged into a single spider with combined phase.
        """
        changed = False
        to_merge: List[Tuple[int, int]] = []

        # Find pairs of same-type spiders connected by simple edges
        for edge in self.graph.edges:
            if edge.edge_type != "simple":
                continue
            if edge.source not in self.graph.nodes or edge.target not in self.graph.nodes:
                continue

            source_node = self.graph.nodes[edge.source]
            target_node = self.graph.nodes[edge.target]

            if source_node.node_type == target_node.node_type and source_node.node_type in ("Z", "X"):
                to_merge.append((edge.source, edge.target))

        # Merge each pair
        for source_id, target_id in to_merge:
            if source_id not in self.graph.nodes or target_id not in self.graph.nodes:
                continue

            source_node = self.graph.nodes[source_id]
            target_node = self.graph.nodes[target_id]

            # Combine phases (mod 2π for Z-spiders)
            new_phase = (source_node.phase + target_node.phase) % (2 * math.pi)
            source_node.phase = new_phase

            # Redirect all edges from target to source
            for edge in list(self.graph.edges):
                if edge.source == target_id:
                    edge.source = source_id
                elif edge.target == target_id:
                    edge.target = source_id

            # Remove the connecting edge and the target node
            self.graph.edges = [
                e for e in self.graph.edges
                if not ((e.source == source_id and e.target == target_id) or
                        (e.target == source_id and e.source == target_id))
            ]
            self.graph.remove_node(target_id)
            changed = True

        return changed

    def _remove_parallel_edges(self) -> bool:
        """Remove parallel edges between the same pair of nodes.
        
        Rule: Two parallel simple edges between the same nodes can be removed.
        Two parallel Hadamard edges cancel out (become simple).
        """
        changed = False
        edge_counts: Dict[Tuple[int, int, str], int] = {}

        # Count parallel edges
        for edge in self.graph.edges:
            key = (min(edge.source, edge.target), max(edge.source, edge.target), edge.edge_type)
            edge_counts[key] = edge_counts.get(key, 0) + 1

        # Remove duplicate simple edges
        for (u, v, edge_type), count in edge_counts.items():
            if count > 1 and edge_type == "simple":
                # Keep one, remove the rest
                removed = 0
                edges_to_keep = []
                for edge in self.graph.edges:
                    if (min(edge.source, edge.target) == u and
                        max(edge.source, edge.target) == v and
                        edge.edge_type == "simple"):
                        if removed == 0:
                            edges_to_keep.append(edge)
                            removed += 1
                        else:
                            changed = True
                # Rebuild edge list
                other_edges = [
                    e for e in self.graph.edges
                    if not (min(e.source, e.target) == u and
                            max(e.source, e.target) == v and
                            e.edge_type == "simple")
                ]
                self.graph.edges = other_edges + edges_to_keep

        return changed

    def _remove_hadamard_loops(self) -> bool:
        """Remove Hadamard self-loops (Hadamard gates on a wire).
        
        Rule: A Hadamard self-loop on a Z-spider can be converted to
        an X-spider with the same phase.
        """
        changed = False
        to_convert: List[int] = []

        for node_id, node in self.graph.nodes.items():
            if node.node_type != "Z":
                continue

            # Check for Hadamard self-loop
            for edge in self.graph.edges:
                if edge.source == edge.target == node_id and edge.edge_type == "hadamard":
                    to_convert.append(node_id)
                    break

        for node_id in to_convert:
            if node_id in self.graph.nodes:
                self.graph.nodes[node_id].node_type = "X"
                # Remove the self-loop edge
                self.graph.edges = [
                    e for e in self.graph.edges
                    if not (e.source == e.target == node_id)
                ]
                changed = True

        return changed

    def _fuse_complementary_phases(self) -> bool:
        """Fuse spiders with complementary phases (π and -π are equivalent)."""
        changed = False

        for node_id, node in self.graph.nodes.items():
            if node.node_type not in ("Z", "X"):
                continue

            # Normalize phase to [0, 2π)
            if node.phase < 0:
                node.phase = node.phase % (2 * math.pi)
                changed = True
            elif node.phase >= 2 * math.pi:
                node.phase = node.phase % (2 * math.pi)
                changed = True

        return changed


def parse_qasm_to_zxir(qasm: str) -> ZXGraph:
    """Parse a simple QASM program into ZX-IR.
    
    This is a simplified parser for demonstration purposes.
    It handles basic gates: H, X, Y, Z, CX, CZ.
    """
    graph = ZXGraph()
    qubit_count = 0
    qubit_nodes: Dict[int, int] = {}  # qubit index -> current node ID

    lines = qasm.strip().split('\n')
    for line in lines:
        line = line.strip()
        if not line or line.startswith(('//', 'OPENQASM', 'include', 'qreg', 'creg')):
            continue

        # Parse qubit declaration
        if line.startswith('qreg'):
            match = line.replace('qreg', '').replace(';', '').strip().split('[')
            if len(match) == 2:
                qubit_count = int(match[1].replace(']', ''))
                for i in range(qubit_count):
                    input_id = graph.add_node("input", qubit=i)
                    output_id = graph.add_node("output", qubit=i)
                    graph.add_edge(input_id, output_id)
                    qubit_nodes[i] = input_id
                continue

        # Parse single-qubit gates
        for gate in ['h', 'x', 'y', 'z']:
            if line.startswith(gate):
                target = line.replace(gate, '').replace(';', '').strip()
                if target.startswith('q['):
                    qubit = int(target[2:-1])
                    if qubit in qubit_nodes:
                        _apply_single_qubit_gate(graph, qubit_nodes, qubit, gate)
                break

        # Parse two-qubit gates
        for gate in ['cx', 'cz']:
            if line.startswith(gate):
                targets = line.replace(gate, '').replace(';', '').strip().split(',')
                if len(targets) == 2:
                    control = int(targets[0].replace('q[', '').replace(']', ''))
                    target = int(targets[1].replace('q[', '').replace(']', ''))
                    if control in qubit_nodes and target in qubit_nodes:
                        _apply_two_qubit_gate(graph, qubit_nodes, control, target, gate)
                break

    return graph


def _apply_single_qubit_gate(graph: ZXGraph, qubit_nodes: Dict[int, int], qubit: int, gate: str) -> None:
    """Apply a single-qubit gate to the ZX-IR."""
    current_node = qubit_nodes[qubit]

    if gate == 'h':
        # Hadamard: insert H-box
        h_id = graph.add_node("H", qubit=qubit)
        # Find and replace the edge
        for i, edge in enumerate(graph.edges):
            if edge.target == current_node:
                graph.edges[i].target = h_id
                graph.add_edge(h_id, current_node, "simple")
                break
        qubit_nodes[qubit] = h_id
    elif gate == 'z':
        # Z gate: Z-spider with phase π
        z_id = graph.add_node("Z", phase=math.pi, qubit=qubit)
        for i, edge in enumerate(graph.edges):
            if edge.target == current_node:
                graph.edges[i].target = z_id
                graph.add_edge(z_id, current_node, "simple")
                break
        qubit_nodes[qubit] = z_id
    elif gate == 'x':
        # X gate: X-spider with phase π
        x_id = graph.add_node("X", phase=math.pi, qubit=qubit)
        for i, edge in enumerate(graph.edges):
            if edge.target == current_node:
                graph.edges[i].target = x_id
                graph.add_edge(x_id, current_node, "simple")
                break
        qubit_nodes[qubit] = x_id
    elif gate == 'y':
        # Y gate: equivalent to Z(π) followed by X(π)
        z_id = graph.add_node("Z", phase=math.pi, qubit=qubit)
        x_id = graph.add_node("X", phase=math.pi, qubit=qubit)
        for i, edge in enumerate(graph.edges):
            if edge.target == current_node:
                graph.edges[i].target = z_id
                graph.add_edge(z_id, x_id, "simple")
                graph.add_edge(x_id, current_node, "simple")
                break
        qubit_nodes[qubit] = z_id


def _apply_two_qubit_gate(graph: ZXGraph, qubit_nodes: Dict[int, int], control: int, target: int, gate: str) -> None:
    """Apply a two-qubit gate to the ZX-IR."""
    control_node = qubit_nodes[control]
    target_node = qubit_nodes[target]

    if gate == 'cx':
        # CNOT: control Z-spider, target X-spider, Hadamard edge
        z_id = graph.add_node("Z", qubit=control)
        x_id = graph.add_node("X", qubit=target)

        # Rewire edges
        for i, edge in enumerate(graph.edges):
            if edge.target == control_node:
                graph.edges[i].target = z_id
                graph.add_edge(z_id, control_node, "simple")
            elif edge.target == target_node:
                graph.edges[i].target = x_id
                graph.add_edge(x_id, target_node, "simple")

        # Add Hadamard edge between control and target
        graph.add_edge(z_id, x_id, "hadamard")

        qubit_nodes[control] = z_id
        qubit_nodes[target] = x_id

    elif gate == 'cz':
        # CZ: two Z-spiders with simple edge
        z1_id = graph.add_node("Z", qubit=control)
        z2_id = graph.add_node("Z", qubit=target)

        for i, edge in enumerate(graph.edges):
            if edge.target == control_node:
                graph.edges[i].target = z1_id
                graph.add_edge(z1_id, control_node, "simple")
            elif edge.target == target_node:
                graph.edges[i].target = z2_id
                graph.add_edge(z2_id, target_node, "simple")

        graph.add_edge(z1_id, z2_id, "simple")

        qubit_nodes[control] = z1_id
        qubit_nodes[target] = z2_id
