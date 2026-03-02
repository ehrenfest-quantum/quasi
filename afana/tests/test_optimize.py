"""Unit tests for ZX-IR identity reduction optimization."""

import pytest
import math
from afana.optimize import ZXGraph, ZXOptimizer, ZXNode, ZXEdge


class TestIdentityReduction:
    """Test identity reduction rules achieve 20%+ edge/node reduction."""

    def test_remove_identity_z_spiders(self):
        """Test removal of Z-spiders with phase 0 and single connection."""
        graph = ZXGraph()
        # Create a chain: input -> Z(0) -> output
        input_id = graph.add_node("input")
        z_id = graph.add_node("Z", phase=0.0)
        output_id = graph.add_node("output")
        graph.add_edge(input_id, z_id)
        graph.add_edge(z_id, output_id)

        initial_nodes = graph.node_count()
        initial_edges = graph.edge_count()

        optimizer = ZXOptimizer(graph)
        optimizer.optimize()

        # Z(0) spider should be removed, direct connection remains
        assert graph.node_count() == initial_nodes - 1
        assert graph.edge_count() == initial_edges - 1

    def test_merge_same_type_spiders(self):
        """Test merging of two Z-spiders connected by simple edge."""
        graph = ZXGraph()
        input_id = graph.add_node("input")
        z1_id = graph.add_node("Z", phase=math.pi/4)
        z2_id = graph.add_node("Z", phase=math.pi/2)
        output_id = graph.add_node("output")
        graph.add_edge(input_id, z1_id)
        graph.add_edge(z1_id, z2_id, "simple")
        graph.add_edge(z2_id, output_id)

        initial_nodes = graph.node_count()

        optimizer = ZXOptimizer(graph)
        optimizer.optimize()

        # Two Z-spiders should merge into one
        assert graph.node_count() == initial_nodes - 1
        # Check phase combination
        remaining_z = [n for n in graph.nodes.values() if n.node_type == "Z"]
        assert len(remaining_z) == 1
        assert math.isclose(remaining_z[0].phase, 3*math.pi/4, abs_tol=1e-10)

    def test_remove_parallel_edges(self):
        """Test removal of duplicate parallel edges."""
        graph = ZXGraph()
        n1 = graph.add_node("Z")
        n2 = graph.add_node("Z")
        # Add two parallel simple edges
        graph.add_edge(n1, n2, "simple")
        graph.add_edge(n1, n2, "simple")

        initial_edges = graph.edge_count()

        optimizer = ZXOptimizer(graph)
        optimizer.optimize()

        # One parallel edge should be removed
        assert graph.edge_count() == initial_edges - 1

    def test_synthetic_graph_20_percent_reduction(self):
        """Test that synthetic ZX-IR graph achieves 20%+ node/edge reduction."""
        graph = ZXGraph()

        # Create a synthetic graph with many reducible patterns
        # Pattern 1: Chain of identity Z-spiders
        prev = graph.add_node("input")
        for _ in range(5):
            z_id = graph.add_node("Z", phase=0.0)
            graph.add_edge(prev, z_id)
            prev = z_id
        graph.add_edge(prev, graph.add_node("output"))

        # Pattern 2: Parallel edges
        n1 = graph.add_node("Z", phase=math.pi)
        n2 = graph.add_node("Z", phase=math.pi)
        graph.add_edge(n1, n2, "simple")
        graph.add_edge(n1, n2, "simple")

        # Pattern 3: Mergeable Z-spiders
        input2 = graph.add_node("input")
        z1 = graph.add_node("Z", phase=math.pi/3)
        z2 = graph.add_node("Z", phase=math.pi/6)
        z3 = graph.add_node("Z", phase=math.pi/2)
        output2 = graph.add_node("output")
        graph.add_edge(input2, z1)
        graph.add_edge(z1, z2, "simple")
        graph.add_edge(z2, z3, "simple")
        graph.add_edge(z3, output2)

        initial_nodes = graph.node_count()
        initial_edges = graph.edge_count()

        optimizer = ZXOptimizer(graph)
        optimizer.optimize()

        final_nodes = graph.node_count()
        final_edges = graph.edge_count()

        node_reduction = (initial_nodes - final_nodes) / initial_nodes * 100
        edge_reduction = (initial_edges - final_edges) / initial_edges * 100

        # Assert 20%+ reduction in both nodes and edges
        assert node_reduction >= 20.0, f"Node reduction: {node_reduction:.1f}%"
        assert edge_reduction >= 20.0, f"Edge reduction: {edge_reduction:.1f}%"

    def test_complex_synthetic_graph(self):
        """Test a more complex synthetic graph with multiple reducible patterns."""
        graph = ZXGraph()

        # Create multiple qubit lines with reducible patterns
        for qubit in range(3):
            prev = graph.add_node("input", qubit=qubit)
            # Add identity spiders
            for _ in range(3):
                z_id = graph.add_node("Z", phase=0.0, qubit=qubit)
                graph.add_edge(prev, z_id)
                prev = z_id
            # Add mergeable spiders
            z1 = graph.add_node("Z", phase=math.pi/4, qubit=qubit)
            z2 = graph.add_node("Z", phase=math.pi/4, qubit=qubit)
            graph.add_edge(prev, z1)
            graph.add_edge(z1, z2, "simple")
            prev = z2
            graph.add_edge(prev, graph.add_node("output", qubit=qubit))

        # Add some parallel edges
        n1 = graph.add_node("Z")
        n2 = graph.add_node("Z")
        graph.add_edge(n1, n2, "simple")
        graph.add_edge(n1, n2, "simple")
        graph.add_edge(n1, n2, "simple")

        initial_nodes = graph.node_count()
        initial_edges = graph.edge_count()

        optimizer = ZXOptimizer(graph)
        optimizer.optimize()

        final_nodes = graph.node_count()
        final_edges = graph.edge_count()

        node_reduction = (initial_nodes - final_nodes) / initial_nodes * 100
        edge_reduction = (initial_edges - final_edges) / initial_edges * 100

        assert node_reduction >= 20.0, f"Node reduction: {node_reduction:.1f}%"
        assert edge_reduction >= 20.0, f"Edge reduction: {edge_reduction:.1f}%"

    def test_phase_normalization(self):
        """Test that phases are normalized to [0, 2π)."""
        graph = ZXGraph()
        z1 = graph.add_node("Z", phase=-math.pi)
        z2 = graph.add_node("Z", phase=3*math.pi)
        z3 = graph.add_node("Z", phase=5*math.pi/2)

        optimizer = ZXOptimizer(graph)
        optimizer.optimize()

        # All phases should be in [0, 2π)
        for node in graph.nodes.values():
            if node.node_type in ("Z", "X"):
                assert 0 <= node.phase < 2 * math.pi


class TestBellStateOptimization:
    """Integration test for bell_state.qasm optimization."""

    def test_bell_state_node_reduction(self):
        """Test that bell_state.qasm ZX-IR reduces from 12 to ≤8 nodes."""
        from afana.optimize import parse_qasm_to_zxir

        bell_qasm = """
OPENQASM 2.0;
include "qelib1.inc";
qreg q[2];
creg c[2];
h q[0];
cx q[0],q[1];
"""

        graph = parse_qasm_to_zxir(bell_qasm)
        initial_nodes = graph.node_count()

        # The bell state should parse to 12 nodes
        assert initial_nodes == 12, f"Expected 12 nodes, got {initial_nodes}"

        optimizer = ZXOptimizer(graph)
        optimized_graph = optimizer.optimize()

        final_nodes = optimized_graph.node_count()

        # After optimization, should have ≤8 nodes
        assert final_nodes <= 8, f"Expected ≤8 nodes after optimization, got {final_nodes}"

    def test_bell_state_edge_reduction(self):
        """Test that bell_state.qasm also shows edge reduction."""
        from afana.optimize import parse_qasm_to_zxir

        bell_qasm = """
OPENQASM 2.0;
include "qelib1.inc";
qreg q[2];
creg c[2];
h q[0];
cx q[0],q[1];
"""

        graph = parse_qasm_to_zxir(bell_qasm)
        initial_edges = graph.edge_count()

        optimizer = ZXOptimizer(graph)
        optimized_graph = optimizer.optimize()

        final_edges = optimized_graph.edge_count()

        # Should also reduce edges
        assert final_edges < initial_edges, f"Edges: {initial_edges} -> {final_edges}"
