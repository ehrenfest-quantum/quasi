// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Entanglement tracking (Phase E).
//!
//! Tracks which qubits are entangled across programs and enforces scheduling
//! constraints. Entanglement is the quantum analog of shared memory between
//! processes — two programs sharing entangled qubits cannot be measured
//! independently without destroying the shared state.

use std::collections::{HashSet, VecDeque};

/// An entanglement link between two qubits.
#[derive(Debug, Clone)]
pub struct EntanglementLink {
    pub qubit_a: u32,
    pub qubit_b: u32,
    /// Which program created this entanglement.
    pub created_by: String,
    /// Gate that created it (e.g., "cx", "cz").
    pub gate: String,
    /// Whether this link has been consumed by measurement.
    pub consumed: bool,
}

/// Tracks all active entanglement in the system.
#[derive(Debug, Clone, Default)]
pub struct EntanglementMap {
    links: Vec<EntanglementLink>,
}

impl EntanglementMap {
    /// Create an empty entanglement map.
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    /// Record an entangling gate between two qubits.
    pub fn add_link(&mut self, qubit_a: u32, qubit_b: u32, program_id: &str, gate: &str) {
        self.links.push(EntanglementLink {
            qubit_a,
            qubit_b,
            created_by: program_id.to_string(),
            gate: gate.to_string(),
            consumed: false,
        });
    }

    /// Mark all links involving this qubit as consumed (measurement happened).
    pub fn consume(&mut self, qubit: u32) {
        for link in &mut self.links {
            if (link.qubit_a == qubit || link.qubit_b == qubit) && !link.consumed {
                link.consumed = true;
            }
        }
    }

    /// Iterate over links that have not been consumed.
    pub fn active_links(&self) -> impl Iterator<Item = &EntanglementLink> {
        self.links.iter().filter(|l| !l.consumed)
    }

    /// Number of active (unconsumed) entanglement links.
    pub fn active_link_count(&self) -> usize {
        self.active_links().count()
    }

    /// All qubits entangled with the given qubit (transitive closure).
    ///
    /// If A-B and B-C are active links, then `entangled_with(A)` returns `[B, C]`.
    pub fn entangled_with(&self, qubit: u32) -> Vec<u32> {
        let mut visited: HashSet<u32> = HashSet::new();
        let mut queue: VecDeque<u32> = VecDeque::new();

        visited.insert(qubit);
        queue.push_back(qubit);

        while let Some(current) = queue.pop_front() {
            for link in self.active_links() {
                let neighbor = if link.qubit_a == current {
                    Some(link.qubit_b)
                } else if link.qubit_b == current {
                    Some(link.qubit_a)
                } else {
                    None
                };
                if let Some(n) = neighbor {
                    if visited.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
        }

        visited.remove(&qubit);
        let mut result: Vec<u32> = visited.into_iter().collect();
        result.sort_unstable();
        result
    }

    /// Whether two qubits are entangled (directly or transitively) via active links.
    pub fn is_entangled(&self, qubit_a: u32, qubit_b: u32) -> bool {
        self.entangled_with(qubit_a).contains(&qubit_b)
    }

    /// Pairs of programs that share entangled qubits (scheduling dependency).
    ///
    /// Two programs share entanglement if program A created entanglement on
    /// qubit X and program B also created entanglement on qubit X (or a qubit
    /// transitively entangled with X).
    pub fn programs_sharing_entanglement(&self) -> Vec<(String, String)> {
        // Collect which programs touch which qubits via active links.
        let mut program_qubits: std::collections::HashMap<String, HashSet<u32>> =
            std::collections::HashMap::new();

        for link in self.active_links() {
            let entry = program_qubits
                .entry(link.created_by.clone())
                .or_default();
            entry.insert(link.qubit_a);
            entry.insert(link.qubit_b);
        }

        let programs: Vec<String> = program_qubits.keys().cloned().collect();
        let mut pairs: Vec<(String, String)> = Vec::new();

        for i in 0..programs.len() {
            for j in (i + 1)..programs.len() {
                let qubits_a = &program_qubits[&programs[i]];
                let qubits_b = &program_qubits[&programs[j]];
                let shares = qubits_a.iter().any(|q| qubits_b.contains(q));
                if shares {
                    let mut pair = [programs[i].clone(), programs[j].clone()];
                    pair.sort();
                    pairs.push((pair[0].clone(), pair[1].clone()));
                }
            }
        }

        pairs.sort();
        pairs.dedup();
        pairs
    }
}

/// Build an entanglement map from a circuit's two-qubit gates.
///
/// This is what the kernel calls after Afana compiles a program:
/// extract all CX/CZ/etc. gates and record them as entanglement links.
///
/// `gates` is a slice of `(gate_name, qubit_a, qubit_b)`.
pub fn from_gates(program_id: &str, gates: &[(String, u32, u32)]) -> EntanglementMap {
    let mut map = EntanglementMap::new();
    for (gate, qa, qb) in gates {
        map.add_link(*qa, *qb, program_id, gate);
    }
    map
}

/// Check if two programs can be scheduled independently.
///
/// They CAN if they share no entangled qubits.
/// They CANNOT if program A created entanglement that program B needs to measure.
pub fn can_schedule_independently(
    map: &EntanglementMap,
    program_a: &str,
    program_b: &str,
) -> bool {
    let sharing = map.programs_sharing_entanglement();
    !sharing.iter().any(|(a, b)| {
        (a == program_a && b == program_b) || (a == program_b && b == program_a)
    })
}

/// Compute scheduling dependencies: which programs must run before which.
///
/// If program A creates entanglement consumed by program B's measurements,
/// A must complete before B measures. Returns `(must_run_first, must_run_second)`
/// pairs based on creation order in the entanglement map.
pub fn scheduling_dependencies(
    map: &EntanglementMap,
    programs: &[String],
) -> Vec<(String, String)> {
    let program_set: HashSet<&str> = programs.iter().map(|s| s.as_str()).collect();
    let mut deps: Vec<(String, String)> = Vec::new();

    // For each active link, the creating program must run first.
    // If another program has an active link on the same qubit, it depends
    // on the creator finishing first.
    for link in map.active_links() {
        if !program_set.contains(link.created_by.as_str()) {
            continue;
        }
        // Find other programs that touch either qubit in this link.
        for other_link in map.active_links() {
            if other_link.created_by == link.created_by {
                continue;
            }
            if !program_set.contains(other_link.created_by.as_str()) {
                continue;
            }
            // Check if the other program touches the same qubits.
            let shares = other_link.qubit_a == link.qubit_a
                || other_link.qubit_a == link.qubit_b
                || other_link.qubit_b == link.qubit_a
                || other_link.qubit_b == link.qubit_b;
            if shares {
                let mut pair = [link.created_by.clone(), other_link.created_by.clone()];
                pair.sort();
                let dep = (pair[0].clone(), pair[1].clone());
                if !deps.contains(&dep) {
                    deps.push(dep);
                }
            }
        }
    }

    deps.sort();
    deps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_link_appears_and_active_count_is_one() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog1", "cx");
        assert_eq!(map.active_link_count(), 1);

        let link = map.active_links().next().unwrap();
        assert_eq!(link.qubit_a, 0);
        assert_eq!(link.qubit_b, 1);
        assert_eq!(link.created_by, "prog1");
        assert_eq!(link.gate, "cx");
        assert!(!link.consumed);
    }

    #[test]
    fn consume_marks_link_consumed() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog1", "cx");
        assert_eq!(map.active_link_count(), 1);

        map.consume(0);
        assert_eq!(map.active_link_count(), 0);
    }

    #[test]
    fn transitive_entanglement() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog1", "cx"); // A-B
        map.add_link(1, 2, "prog1", "cx"); // B-C

        let entangled = map.entangled_with(0);
        assert!(entangled.contains(&1));
        assert!(entangled.contains(&2));
        assert_eq!(entangled.len(), 2);
    }

    #[test]
    fn two_independent_programs_can_schedule_independently() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog_a", "cx");
        map.add_link(2, 3, "prog_b", "cx");

        assert!(can_schedule_independently(&map, "prog_a", "prog_b"));
    }

    #[test]
    fn two_programs_sharing_qubit_cannot_schedule_independently() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog_a", "cx");
        map.add_link(1, 2, "prog_b", "cz"); // qubit 1 shared

        assert!(!can_schedule_independently(&map, "prog_a", "prog_b"));
    }

    #[test]
    fn scheduling_dependencies_with_dependent_programs() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog_a", "cx");
        map.add_link(1, 2, "prog_b", "cz");

        let programs = vec!["prog_a".to_string(), "prog_b".to_string()];
        let deps = scheduling_dependencies(&map, &programs);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], ("prog_a".to_string(), "prog_b".to_string()));
    }

    #[test]
    fn from_gates_builds_map_correctly() {
        let gates = vec![
            ("cx".to_string(), 0u32, 1u32),
            ("cz".to_string(), 2, 3),
        ];
        let map = from_gates("prog1", &gates);
        assert_eq!(map.active_link_count(), 2);
        assert!(map.is_entangled(0, 1));
        assert!(map.is_entangled(2, 3));
        assert!(!map.is_entangled(0, 2));
    }

    #[test]
    fn empty_map_everything_independent() {
        let map = EntanglementMap::new();
        assert_eq!(map.active_link_count(), 0);
        assert!(can_schedule_independently(&map, "a", "b"));
        assert!(map.entangled_with(0).is_empty());
        assert!(!map.is_entangled(0, 1));
    }

    #[test]
    fn multiple_gates_between_same_qubits_tracked() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog1", "cx");
        map.add_link(0, 1, "prog1", "cz");
        assert_eq!(map.active_link_count(), 2);

        // Consuming qubit 0 marks both links as consumed.
        map.consume(0);
        assert_eq!(map.active_link_count(), 0);
    }

    #[test]
    fn is_entangled_direct() {
        let mut map = EntanglementMap::new();
        map.add_link(5, 7, "prog1", "cx");
        assert!(map.is_entangled(5, 7));
        assert!(map.is_entangled(7, 5)); // symmetric
        assert!(!map.is_entangled(5, 6));
    }

    #[test]
    fn consume_only_affects_matching_qubit_links() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog1", "cx");
        map.add_link(2, 3, "prog1", "cx");
        assert_eq!(map.active_link_count(), 2);

        map.consume(0);
        assert_eq!(map.active_link_count(), 1);

        // The 2-3 link is still active.
        let remaining = map.active_links().next().unwrap();
        assert_eq!(remaining.qubit_a, 2);
        assert_eq!(remaining.qubit_b, 3);
    }

    #[test]
    fn programs_sharing_entanglement_returns_sorted_pairs() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "zebra", "cx");
        map.add_link(1, 2, "alpha", "cz");

        let pairs = map.programs_sharing_entanglement();
        assert_eq!(pairs.len(), 1);
        // Should be sorted: alpha before zebra.
        assert_eq!(pairs[0], ("alpha".to_string(), "zebra".to_string()));
    }

    #[test]
    fn consumed_links_do_not_affect_entanglement() {
        let mut map = EntanglementMap::new();
        map.add_link(0, 1, "prog1", "cx");
        map.consume(0);

        // After consumption, qubits are no longer entangled.
        assert!(!map.is_entangled(0, 1));
        assert!(map.entangled_with(0).is_empty());
    }
}
