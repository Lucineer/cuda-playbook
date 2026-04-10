/*!
# cuda-playbook

Reusable action patterns and strategies.

A playbook is to an agent what a cookbook is to a chef — proven recipes
for common situations. Instead of deliberating from scratch every time,
agents execute pre-validated patterns.

- Play patterns (named reusable sequences)
- Tactics (conditional sub-patterns)
- Strategies (meta-patterns for choosing tactics)
- Pattern composition (build complex from simple)
- Execution tracking (success rate per pattern)
- Pattern sharing between agents
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A step in a play
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub action: String,
    pub params: HashMap<String, f64>,
    pub condition: Option<String>,   // execute only if condition met
    pub timeout_ms: u64,
    pub fallback: Option<String>,    // step name to jump to on failure
    pub is_optional: bool,
}

/// A play — a reusable action pattern
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Play {
    pub id: String,
    pub name: String,
    pub category: String,
    pub steps: Vec<Step>,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
    pub estimated_effort: f64,
    pub success_count: u32,
    pub failure_count: u32,
    pub tags: Vec<String>,
}

impl Play {
    pub fn new(id: &str, name: &str, category: &str) -> Self {
        Play { id: id.to_string(), name: name.to_string(), category: category.to_string(), steps: vec![], preconditions: vec![], postconditions: vec![], estimated_effort: 0.0, success_count: 0, failure_count: 0, tags: vec![] }
    }

    pub fn add_step(&mut self, step: Step) { self.estimated_effort += 0.05; self.steps.push(step); }

    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 { return 0.5; }
        self.success_count as f64 / total as f64
    }

    pub fn reliability(&self) -> f64 {
        // Weighted by execution count — more executions = more reliable estimate
        let total = (self.success_count + self.failure_count) as f64;
        let base_rate = self.success_rate();
        let weight = (total / 20.0).min(1.0); // 20 executions = full weight
        0.5 * (1.0 - weight) + base_rate * weight
    }
}

/// A tactic — conditional pattern selection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tactic {
    pub id: String,
    pub name: String,
    pub trigger_conditions: Vec<String>,
    pub play_id: String,
    pub priority: f64,
    pub last_used: u64,
}

impl Tactic {
    pub fn matches(&self, state_tags: &[&str]) -> f64 {
        let matches = self.trigger_conditions.iter().filter(|c| state_tags.iter().any(|t| t.contains(c) || c.contains(t))).count();
        if self.trigger_conditions.is_empty() { return 0.0; }
        matches as f64 / self.trigger_conditions.len() as f64
    }
}

/// A strategy — meta-level pattern for selecting tactics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Strategy {
    pub id: String,
    pub name: String,
    pub tactic_order: Vec<String>,  // preferred tactic order
    pub mode: StrategyMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyMode {
    Aggressive,    // prefer high-priority, high-risk plays
    Cautious,      // prefer reliable, low-risk plays
    Balanced,      // mix of both
    Adaptive,      // choose based on recent outcomes
}

/// Play execution state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState { Ready, Running, Paused, Completed, Failed, Cancelled }

/// Running play instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayExecution {
    pub play_id: String,
    pub instance_id: String,
    pub current_step: usize,
    pub state: ExecutionState,
    pub started_at: u64,
    pub vars: HashMap<String, f64>,
    pub steps_completed: u32,
    pub steps_failed: u32,
}

impl PlayExecution {
    pub fn progress(&self) -> f64 {
        if self.steps_completed + self.steps_failed == 0 { return 0.0; }
        self.steps_completed as f64 / (self.steps_completed + self.steps_failed) as f64
    }
}

/// The playbook
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Playbook {
    pub plays: HashMap<String, Play>,
    pub tactics: Vec<Tactic>,
    pub strategies: HashMap<String, Strategy>,
    pub executions: Vec<PlayExecution>,
    pub shared_patterns: HashMap<String, Vec<String>>, // agent_id -> list of play_ids they know
    pub max_executions: usize,
}

impl Playbook {
    pub fn new() -> Self { Playbook { plays: HashMap::new(), tactics: vec![], strategies: HashMap::new(), executions: vec![], shared_patterns: HashMap::new(), max_executions: 50 } }

    /// Add a play
    pub fn add_play(&mut self, play: Play) { self.plays.insert(play.id.clone(), play); }

    /// Add a tactic
    pub fn add_tactic(&mut self, tactic: Tactic) { self.tactics.push(tactic); }

    /// Add a strategy
    pub fn add_strategy(&mut self, strategy: Strategy) { self.strategies.insert(strategy.id.clone(), strategy); }

    /// Find best play for a situation
    pub fn find_play(&self, state_tags: &[&str], mode: StrategyMode) -> Option<(&Play, f64)> {
        let matching: Vec<(&Tactic, f64)> = self.tactics.iter()
            .filter_map(|t| { let score = t.matches(state_tags); if score > 0.3 { Some((t, score * t.priority)) } else { None } })
            .collect();

        if matching.is_empty() { return None; }

        let best_tactic = match mode {
            StrategyMode::Aggressive => matching.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()),
            StrategyMode::Cautious => matching.iter().max_by(|a, b| {
                let ra = self.plays.get(&a.0.play_id).map(|p| p.reliability()).unwrap_or(0.5);
                let rb = self.plays.get(&b.0.play_id).map(|p| p.reliability()).unwrap_or(0.5);
                ra.partial_cmp(&rb).unwrap()
            }),
            StrategyMode::Balanced | StrategyMode::Adaptive => matching.iter().max_by(|a, b| {
                let sa = a.1 * 0.5 + self.plays.get(&a.0.play_id).map(|p| p.reliability()).unwrap_or(0.5) * 0.5;
                let sb = b.1 * 0.5 + self.plays.get(&b.0.play_id).map(|p| p.reliability()).unwrap_or(0.5) * 0.5;
                sa.partial_cmp(&sb).unwrap()
            }),
        };

        best_tactic.and_then(|(t, score)| self.plays.get(&t.play_id).map(|p| (p, *score)))
    }

    /// Execute a play
    pub fn execute(&mut self, play_id: &str) -> Option<String> {
        if !self.plays.contains_key(play_id) { return None; }
        let instance_id = format!("exec_{}", self.executions.len());
        self.executions.push(PlayExecution { play_id: play_id.to_string(), instance_id: instance_id.clone(), current_step: 0, state: ExecutionState::Running, started_at: now(), vars: HashMap::new(), steps_completed: 0, steps_failed: 0 });
        Some(instance_id)
    }

    /// Record play outcome
    pub fn record_outcome(&mut self, play_id: &str, success: bool) {
        if let Some(play) = self.plays.get_mut(play_id) {
            if success { play.success_count += 1; } else { play.failure_count += 1; }
        }
    }

    /// Share a pattern with an agent
    pub fn share(&mut self, agent_id: &str, play_id: &str) {
        self.shared_patterns.entry(agent_id.to_string()).or_default().push(play_id.to_string());
    }

    /// Get patterns known to an agent
    pub fn known_patterns(&self, agent_id: &str) -> Vec<&str> {
        self.shared_patterns.get(agent_id).map(|v| v.iter().map(|s| s.as_str()).collect()).unwrap_or_default()
    }

    /// Most reliable play
    pub fn most_reliable(&self) -> Option<(&Play, f64)> {
        self.plays.iter()
            .filter(|(_, p)| p.success_count + p.failure_count > 0)
            .map(|(id, p)| (p, p.reliability()))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(p, r)| (p, r))
    }

    /// Find plays by tag
    pub fn plays_by_tag(&self, tag: &str) -> Vec<&Play> {
        self.plays.values().filter(|p| p.tags.iter().any(|t| t == tag)).collect()
    }

    /// Find plays by category
    pub fn plays_by_category(&self, category: &str) -> Vec<&Play> {
        self.plays.values().filter(|p| p.category == category).collect()
    }

    /// Summary
    pub fn summary(&self) -> String {
        let total_execs: u32 = self.plays.values().map(|p| p.success_count + p.failure_count).sum();
        let total_successes: u32 = self.plays.values().map(|p| p.success_count).sum();
        format!("Playbook: {} plays, {} tactics, {} strategies, total_execs={}, success_rate={:.0%}",
            self.plays.len(), self.tactics.len(), self.strategies.len(), total_execs, if total_execs > 0 { total_successes as f64 / total_execs as f64 } else { 0.0 })
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_play() -> Play {
        let mut play = Play::new("patrol", "Perimeter Patrol", "security");
        play.tags.push("patrol".into()); play.tags.push("security".into());
        play.add_step(Step { action: "move_to".into(), params: HashMap::new(), condition: None, timeout_ms: 5000, fallback: None, is_optional: false });
        play.add_step(Step { action: "scan".into(), params: HashMap::new(), condition: None, timeout_ms: 2000, fallback: None, is_optional: false });
        play
    }

    #[test]
    fn test_add_play() {
        let mut pb = Playbook::new();
        pb.add_play(make_test_play());
        assert!(pb.plays.contains_key("patrol"));
    }

    #[test]
    fn test_success_rate() {
        let mut play = make_test_play();
        play.success_count = 8; play.failure_count = 2;
        assert!((play.success_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_reliability_weights() {
        let mut play = make_test_play();
        play.success_count = 1; play.failure_count = 0; // 1 exec, unreliable estimate
        let r1 = play.reliability();
        play.success_count = 20; play.failure_count = 0;
        let r2 = play.reliability();
        assert!(r2 > r1); // more data = higher reliability
    }

    #[test]
    fn test_find_play() {
        let mut pb = Playbook::new();
        pb.add_play(make_test_play());
        pb.add_tactic(Tactic { id: "t1", name: "patrol_tactic", trigger_conditions: vec!["patrol_needed".into()], play_id: "patrol".into(), priority: 0.8, last_used: 0 });
        let result = pb.find_play(&["patrol_needed"], StrategyMode::Balanced);
        assert!(result.is_some());
    }

    #[test]
    fn test_execute_play() {
        let mut pb = Playbook::new();
        pb.add_play(make_test_play());
        let exec_id = pb.execute("patrol");
        assert!(exec_id.is_some());
        assert_eq!(pb.executions.len(), 1);
    }

    #[test]
    fn test_record_outcome() {
        let mut pb = Playbook::new();
        pb.add_play(make_test_play());
        pb.record_outcome("patrol", true);
        pb.record_outcome("patrol", true);
        pb.record_outcome("patrol", false);
        assert!((pb.plays["patrol"].success_rate() - 0.667).abs() < 0.01);
    }

    #[test]
    fn test_share_pattern() {
        let mut pb = Playbook::new();
        pb.add_play(make_test_play());
        pb.share("agent1", "patrol");
        assert_eq!(pb.known_patterns("agent1").len(), 1);
    }

    #[test]
    fn test_most_reliable() {
        let mut pb = Playbook::new();
        let mut p1 = make_test_play(); p1.id = "p1".into(); p1.success_count = 10; p1.failure_count = 2;
        let mut p2 = make_test_play(); p2.id = "p2".into(); p2.success_count = 3; p2.failure_count = 7;
        pb.add_play(p1); pb.add_play(p2);
        let best = pb.most_reliable();
        assert_eq!(best.unwrap().0.id, "p1");
    }

    #[test]
    fn test_plays_by_tag() {
        let mut pb = Playbook::new();
        pb.add_play(make_test_play());
        assert_eq!(pb.plays_by_tag("patrol").len(), 1);
    }

    #[test]
    fn test_tactic_matching() {
        let tactic = Tactic { id: "t1".into(), name: "x".into(), trigger_conditions: vec!["danger".into(), "alert".into()], play_id: "retreat".into(), priority: 0.8, last_used: 0 };
        let score = tactic.matches(&["danger", "enemy_nearby"]);
        assert!(score > 0.3);
        let score2 = tactic.matches(&["calm", "safe"]);
        assert!(score2 < 0.3);
    }

    #[test]
    fn test_play_execution_progress() {
        let mut pe = PlayExecution { play_id: "p".into(), instance_id: "e1".into(), current_step: 0, state: ExecutionState::Running, started_at: 0, vars: HashMap::new(), steps_completed: 3, steps_failed: 1 };
        assert!((pe.progress() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_summary() {
        let pb = Playbook::new();
        let s = pb.summary();
        assert!(s.contains("0 plays"));
    }
}
