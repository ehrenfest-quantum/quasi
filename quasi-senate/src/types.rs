// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Shared types for the Senate Loop pipeline.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ── Senate roles ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    A1Council,
    A2Drafter,
    A3Gate,
    B1Solver,
    B2Reviewer,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::A1Council => write!(f, "A.1 Council"),
            Role::A2Drafter => write!(f, "A.2 Drafter"),
            Role::A3Gate => write!(f, "A.3 Gate"),
            Role::B1Solver => write!(f, "B.1 Solver"),
            Role::B2Reviewer => write!(f, "B.2 Reviewer"),
        }
    }
}

// ── Rotation entry ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEntry {
    /// Short name used as identifier: "deepseek-v3"
    pub id: String,
    /// API model string: "deepseek/deepseek-chat-v3-0324"
    pub model: String,
    /// Key into PROVIDERS map
    pub provider: String,
    /// SPDX or brief license name
    pub license: String,
    /// Country/org for coverage tracking
    pub origin: String,
    /// Which Senate roles this model can fill
    pub roles: Vec<Role>,
    /// Quarantined models are excluded from selection (managed by quasi-roster)
    #[serde(default)]
    pub quarantined: bool,
    /// Override for models with small output windows
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Override for models with small context windows
    #[serde(default)]
    pub max_context: Option<u32>,
}

// ── Phase Charter ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Charter {
    pub phase_id: String,
    pub date: String,
    pub frontier_level: u8,
    pub goal: String,
    pub priorities: Vec<Priority>,
    pub blocked_topics: Vec<String>,
    pub quota: Quota,
    pub notes_to_reviewers: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Priority {
    pub rank: u8,
    pub area: String,
    pub description: String,
    pub max_issues: u8,
    pub level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    pub total_issues_this_phase: u16,
    pub max_per_priority: u8,
    pub max_l0_issues: u8,
}

// ── Issue Draft ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDraft {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub label: String,
    pub drafter_model: String,
    pub phase_id: String,
}

// ── Gate Verdict ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateVerdict {
    pub verdict: Verdict,
    pub reasoning: String,
    pub suggestions: Option<String>,
    pub reviewer_model: String,
}

// ── Solve Result ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResult {
    pub reasoning: String,
    pub edits: Vec<FileEdit>,
    #[serde(default)]
    pub new_files: HashMap<String, String>,
    pub solver_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    pub file: String,
    pub find: String,
    pub replace: String,
}

// ── Review Verdict ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub verdict: Verdict,
    pub reasoning: String,
    #[serde(default)]
    pub issues: Vec<String>,
    pub suggested_fix: Option<String>,
    pub reviewer_model: String,
}

// ── Verdict ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approve,
    Reject,
    RequestChanges,
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Verdict::Approve => write!(f, "approve"),
            Verdict::Reject => write!(f, "reject"),
            Verdict::RequestChanges => write!(f, "request_changes"),
        }
    }
}

// ── Persistent State ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SenateState {
    pub current_charter: Option<String>,
    pub charter_path: Option<String>,
    pub last_generate_provider: Option<String>,
    pub last_solve_provider: Option<String>,
    #[serde(default)]
    pub draft_retries: HashMap<String, u8>,
    #[serde(default)]
    pub solve_retries: HashMap<String, u8>,
    #[serde(default)]
    pub phase_issue_count: HashMap<String, u16>,
}

// ── GitHub API types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub labels: Vec<IssueLabel>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueLabel {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub content: String,
    pub sha: String,
    pub encoding: String,
}
